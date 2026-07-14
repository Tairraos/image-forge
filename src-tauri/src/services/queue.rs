use std::{collections::HashMap, path::Path, time::Duration};

use tauri::{AppHandle, Manager};

use crate::{
    models::{ApiProvider, GenerateRequest, QueueSnapshot, TaskRecord},
    services::images::{execute_generation, save_outputs},
    services::references::prune_unreferenced_files,
    state::RuntimeState,
    store::{
        clear_running_task, enqueue_task, ensure_data_dir, fallback_failed_record, history_record,
        output_dir_for, pop_next_runnable, read_history, read_json, read_queue, read_settings,
        request_path, upsert_history, write_history, write_queue,
    },
    utils::{http_client_with_proxy, utc_now, REQUEST_TIMEOUT_SECONDS},
};

/// 确保全局只有一个后台队列 worker 在运行。
pub(crate) fn ensure_queue_worker(app: &AppHandle) {
    let state = app.state::<RuntimeState>();
    let Ok(mut active) = state.worker_active.lock() else {
        return;
    };
    if *active {
        return;
    }
    *active = true;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        worker_loop(app.clone()).await;
        if let Ok(mut active) = app.state::<RuntimeState>().worker_active.lock() {
            *active = false;
        }
        if let Ok(data_dir) = ensure_data_dir(&app) {
            if read_queue(&data_dir)
                .map(|queue| !queue.waiting.is_empty())
                .unwrap_or(false)
            {
                ensure_queue_worker(&app);
            }
        }
    });
}

/// 根据队列 JSON 和历史记录组装前端展示所需的队列快照。
pub(crate) fn build_queue_snapshot(
    app: &AppHandle,
    data_dir: &Path,
    history: Vec<TaskRecord>,
) -> Result<QueueSnapshot, String> {
    let queue = read_queue(data_dir)?;
    let by_id: HashMap<String, TaskRecord> = history
        .iter()
        .map(|record| (record.id.clone(), record.clone()))
        .collect();
    let waiting = queue
        .waiting
        .iter()
        .filter_map(|id| by_id.get(id).cloned())
        .collect();
    let running = queue
        .running
        .iter()
        .filter_map(|run| by_id.get(&run.task_id).cloned())
        .collect();
    let recent = history.into_iter().take(80).collect();
    let worker_active = app
        .state::<RuntimeState>()
        .worker_active
        .lock()
        .map(|active| *active)
        .unwrap_or(false);
    Ok(QueueSnapshot {
        waiting,
        running,
        recent,
        worker_active,
        updated_at: queue.updated_at,
    })
}

/// 应用重启后把遗留的 running 任务恢复到 waiting，避免队列卡死。
pub(crate) fn recover_stale_running(app: &AppHandle, data_dir: &Path) -> Result<(), String> {
    let worker_active = app
        .state::<RuntimeState>()
        .worker_active
        .lock()
        .map(|active| *active)
        .unwrap_or(false);
    if worker_active {
        return Ok(());
    }
    let mut queue = read_queue(data_dir)?;
    if queue.running.is_empty() {
        return Ok(());
    }
    let stale: Vec<String> = queue
        .running
        .iter()
        .map(|run| run.task_id.clone())
        .collect();
    queue.running.clear();
    for task_id in stale {
        if !queue.waiting.contains(&task_id) {
            queue.waiting.insert(0, task_id.clone());
        }
        if let Some(mut record) = history_record(data_dir, &task_id)? {
            if record.status == "running" || record.status == "cancelling" {
                record.status = "queued".into();
                record.updated_at = utc_now();
                upsert_task_history(app, data_dir, record)?;
            }
        }
    }
    write_queue(data_dir, &queue)
}

/// 循环启动可运行任务，直到等待和运行队列都清空。
async fn worker_loop(app: AppHandle) {
    loop {
        let mut started = false;
        while start_next_runnable_task(&app).unwrap_or(false) {
            started = true;
        }

        let done = ensure_data_dir(&app)
            .and_then(|data_dir| read_queue(&data_dir))
            .map(|queue| queue.waiting.is_empty() && queue.running.is_empty())
            .unwrap_or(true);
        if done {
            break;
        }

        let delay = if started { 200 } else { 700 };
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
}

/// 从队列中取出一条可运行任务，并在独立异步任务中执行。
fn start_next_runnable_task(app: &AppHandle) -> Result<bool, String> {
    let data_dir = ensure_data_dir(app)?;
    recover_stale_running(app, &data_dir)?;
    let settings = read_settings(&data_dir)?;
    let Some((task_id, provider)) = pop_next_runnable(&data_dir, &settings)? else {
        return Ok(false);
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_task(app.clone(), task_id.clone(), provider).await {
            if let Ok(data_dir) = ensure_data_dir(&app) {
                if is_deleted(&app, &task_id) {
                    let _ = finish_deleted_task(&app, &data_dir, &task_id);
                    return;
                }
                let mut record = read_history(&data_dir)
                    .ok()
                    .and_then(|history| history.into_iter().find(|item| item.id == task_id))
                    .unwrap_or_else(|| fallback_failed_record(&task_id, &error));
                record.status = "failed".into();
                record.error = Some(error);
                record.updated_at = utc_now();
                record.completed_at = Some(utc_now());
                let _ = upsert_task_history(&app, &data_dir, record);
                let _ = clear_running_task(&data_dir, &task_id);
            }
        }
    });
    Ok(true)
}

/// 执行单个生图任务：标记运行、调用 API、保存输出并更新历史。
async fn run_task(app: AppHandle, task_id: String, provider: ApiProvider) -> Result<(), String> {
    let data_dir = ensure_data_dir(&app)?;
    if is_deleted(&app, &task_id) {
        finish_deleted_task(&app, &data_dir, &task_id)?;
        return Ok(());
    }
    let request: GenerateRequest = read_json(&request_path(&data_dir, &task_id))?;
    let settings = read_settings(&data_dir)?;
    let mut record = history_record(&data_dir, &task_id)?.ok_or("找不到任务记录")?;
    let now = utc_now();
    record.status = "running".into();
    record.started_at.get_or_insert_with(|| now.clone());
    record.updated_at = now;
    record.attempts = record.attempts.saturating_add(1);
    record.error = None;
    if !upsert_task_history(&app, &data_dir, record.clone())? {
        finish_deleted_task(&app, &data_dir, &task_id)?;
        return Ok(());
    }

    if is_deleted(&app, &task_id) {
        finish_deleted_task(&app, &data_dir, &task_id)?;
        return Ok(());
    }

    if is_cancel_requested(&app, &task_id) {
        mark_cancelled(&app, &data_dir, &task_id)?;
        clear_running_task(&data_dir, &task_id)?;
        return Ok(());
    }

    let client = http_client_with_proxy(&provider.proxy_url, REQUEST_TIMEOUT_SECONDS, false)?;
    let result = match tokio::time::timeout(
        Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
        execute_generation(&client, &provider, &request),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err("生成超时：超过 5 分钟未返回结果".into()),
    };

    if is_deleted(&app, &task_id) {
        finish_deleted_task(&app, &data_dir, &task_id)?;
        return Ok(());
    }

    if is_cancel_requested(&app, &task_id) {
        mark_cancelled(&app, &data_dir, &task_id)?;
        clear_running_task(&data_dir, &task_id)?;
        return Ok(());
    }

    match result {
        Ok(images) => {
            let output_dir = output_dir_for(&data_dir, &settings)?;
            let outputs = save_outputs(&output_dir, &task_id, &request, images)?;
            record.outputs = outputs;
            record.status = "completed".into();
            record.error = None;
            record.completed_at = Some(utc_now());
            record.updated_at = utc_now();
            if !upsert_task_history(&app, &data_dir, record)? {
                finish_deleted_task(&app, &data_dir, &task_id)?;
                return Ok(());
            }
            clear_running_task(&data_dir, &task_id)?;
            Ok(())
        }
        Err(error) => {
            clear_running_task(&data_dir, &task_id)?;
            if settings.auto_retry && record.attempts < 2 {
                record.status = "queued".into();
                record.error = Some(error);
                record.updated_at = utc_now();
                if !upsert_task_history(&app, &data_dir, record)? {
                    finish_deleted_task(&app, &data_dir, &task_id)?;
                    return Ok(());
                }
                enqueue_task(&data_dir, &task_id)?;
                ensure_queue_worker(&app);
                Ok(())
            } else {
                record.status = "failed".into();
                record.error = Some(error);
                record.completed_at = Some(utc_now());
                record.updated_at = utc_now();
                if !upsert_task_history(&app, &data_dir, record)? {
                    finish_deleted_task(&app, &data_dir, &task_id)?;
                }
                Ok(())
            }
        }
    }
}

fn is_cancel_requested(app: &AppHandle, task_id: &str) -> bool {
    app.state::<RuntimeState>()
        .cancel_requests
        .lock()
        .map(|requests| requests.contains(task_id))
        .unwrap_or(false)
}

fn is_deleted(app: &AppHandle, task_id: &str) -> bool {
    app.state::<RuntimeState>()
        .deleted_tasks
        .lock()
        .map(|tasks| tasks.contains(task_id))
        .unwrap_or(false)
}

/// 写历史前检查删除标记，避免用户删除后后台任务又把记录写回来。
fn upsert_task_history(
    app: &AppHandle,
    data_dir: &Path,
    record: TaskRecord,
) -> Result<bool, String> {
    let state = app.state::<RuntimeState>();
    let deleted_tasks = state.deleted_tasks.lock().map_err(|_| "删除状态锁定失败")?;
    if deleted_tasks.contains(&record.id) {
        return Ok(false);
    }
    upsert_history(data_dir, record)?;
    Ok(true)
}

/// 完成运行中删除任务的收尾：清队列、清请求文件、清运行态标记。
fn finish_deleted_task(app: &AppHandle, data_dir: &Path, task_id: &str) -> Result<(), String> {
    if let Ok(mut requests) = app.state::<RuntimeState>().cancel_requests.lock() {
        requests.remove(task_id);
    }
    clear_running_task(data_dir, task_id)?;
    let mut history = read_history(data_dir)?;
    history.retain(|record| record.id != task_id);
    write_history(data_dir, &history)?;
    let request_file = request_path(data_dir, task_id);
    if request_file.exists() {
        std::fs::remove_file(request_file).map_err(|error| format!("删除任务请求失败: {error}"))?;
    }
    if let Ok(mut tasks) = app.state::<RuntimeState>().deleted_tasks.lock() {
        tasks.remove(task_id);
    }
    prune_unreferenced_files(data_dir)?;
    Ok(())
}

/// 将取消请求落盘为历史失败态，并释放取消标记。
fn mark_cancelled(app: &AppHandle, data_dir: &Path, task_id: &str) -> Result<(), String> {
    if let Ok(mut requests) = app.state::<RuntimeState>().cancel_requests.lock() {
        requests.remove(task_id);
    }
    if let Some(mut record) = history_record(data_dir, task_id)? {
        record.status = "cancelled".into();
        record.error = Some("任务已取消".into());
        record.completed_at = Some(utc_now());
        record.updated_at = utc_now();
        upsert_task_history(app, data_dir, record)?;
    }
    Ok(())
}
