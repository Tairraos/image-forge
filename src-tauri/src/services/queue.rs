use std::{collections::HashMap, path::Path, time::Duration};

use tauri::{AppHandle, Emitter, Manager};

use crate::{
    models::{ApiProvider, GenerateRequest, QueueSnapshot, TaskRecord},
    services::images::{execute_generation, save_outputs},
    services::references::prune_unreferenced_files,
    state::{record_operation, RuntimeState},
    store::{
        clear_running_task, enqueue_task, ensure_data_dir, fallback_failed_record, history_record,
        output_dir_for, pop_next_runnable, read_history, read_json, read_queue, read_settings,
        request_path, upsert_history, write_history, write_queue,
    },
    utils::{http_client_with_proxy, utc_now, REQUEST_TIMEOUT_SECONDS},
};

const QUEUE_UPDATED_EVENT: &str = "queue-updated";

/// 确保全局只有一个后台队列 worker 在运行。
pub(crate) fn ensure_queue_worker(app: &AppHandle) {
    let Ok(data_dir) = ensure_data_dir(app) else {
        return;
    };
    if read_queue(&data_dir)
        .map(|queue| queue.waiting.is_empty())
        .unwrap_or(true)
    {
        return;
    }
    if !mark_worker_active_if_idle(&app.state::<RuntimeState>()) {
        return;
    }
    emit_queue_updated_for_app(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        worker_loop(app.clone()).await;
        set_worker_active(&app, false);
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

fn mark_worker_active_if_idle(state: &RuntimeState) -> bool {
    let Ok(mut active) = state.worker_active.lock() else {
        return false;
    };
    if *active {
        return false;
    }
    *active = true;
    true
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
pub(crate) fn recover_stale_running(
    app: &AppHandle,
    data_dir: &Path,
    history: Option<&mut Vec<TaskRecord>>,
) -> Result<bool, String> {
    let worker_active = app
        .state::<RuntimeState>()
        .worker_active
        .lock()
        .map(|active| *active)
        .unwrap_or(false);
    if worker_active {
        return Ok(false);
    }
    let mut queue = read_queue(data_dir)?;
    if queue.running.is_empty() {
        return Ok(false);
    }
    let stale: Vec<String> = queue
        .running
        .iter()
        .map(|run| run.task_id.clone())
        .collect();
    queue.running.clear();
    for task_id in &stale {
        if !queue.waiting.contains(&task_id) {
            queue.waiting.insert(0, task_id.clone());
        }
    }
    let changed_history = match history {
        Some(history) => {
            let changed_history = mark_stale_records_queued(history, &stale);
            if changed_history {
                write_history(data_dir, history)?;
            }
            changed_history
        }
        None => {
            let mut history = read_history(data_dir)?;
            let changed_history = mark_stale_records_queued(&mut history, &stale);
            if changed_history {
                write_history(data_dir, &history)?;
            }
            changed_history
        }
    };
    write_queue(data_dir, &queue)?;
    Ok(changed_history || !stale.is_empty())
}

pub(crate) fn emit_queue_updated(app: &AppHandle, data_dir: &Path) -> Result<(), String> {
    let history = read_history(data_dir)?;
    let snapshot = build_queue_snapshot(app, data_dir, history)?;
    app.emit(QUEUE_UPDATED_EVENT, snapshot)
        .map_err(|error| format!("发送队列更新事件失败: {error}"))
}

fn emit_queue_updated_for_app(app: &AppHandle) {
    let Ok(data_dir) = ensure_data_dir(app) else {
        return;
    };
    let _ = emit_queue_updated(app, &data_dir);
}

fn set_worker_active(app: &AppHandle, next: bool) {
    let changed = if let Ok(mut active) = app.state::<RuntimeState>().worker_active.lock() {
        if *active == next {
            false
        } else {
            *active = next;
            true
        }
    } else {
        false
    };
    if changed {
        emit_queue_updated_for_app(app);
    }
}

#[cfg(test)]
mod tests {
    use super::mark_worker_active_if_idle;
    use crate::state::RuntimeState;

    #[test]
    fn worker_slot_can_only_be_claimed_once_until_reset() {
        let state = RuntimeState::new();
        let first = mark_worker_active_if_idle(&state);
        assert!(first);
        let second = mark_worker_active_if_idle(&state);
        assert!(!second);
    }
}

fn mark_stale_records_queued(history: &mut [TaskRecord], task_ids: &[String]) -> bool {
    let mut changed = false;
    for task_id in task_ids {
        if let Some(record) = history.iter_mut().find(|item| item.id == *task_id) {
            if record.status == "running" || record.status == "cancelling" {
                record.status = "queued".into();
                record.updated_at = utc_now();
                changed = true;
            }
        }
    }
    changed
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
    recover_stale_running(app, &data_dir, None)?;
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
                let _ = emit_queue_updated(&app, &data_dir);
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
    let _ = emit_queue_updated(&app, &data_dir);

    if is_deleted(&app, &task_id) {
        finish_deleted_task(&app, &data_dir, &task_id)?;
        return Ok(());
    }

    if is_cancel_requested(&app, &task_id) {
        mark_cancelled(&app, &data_dir, &task_id)?;
        clear_running_task(&data_dir, &task_id)?;
        let _ = emit_queue_updated(&app, &data_dir);
        return Ok(());
    }

    let network_params = format!(
        "task_id={} provider_id={} provider_name={} model={} reference_count={} size={} quality={}",
        task_id,
        provider.id,
        provider.name,
        provider.image_model,
        request.reference_paths.len(),
        request.size,
        request.quality
    );
    let proxy_used = !provider.proxy_url.trim().is_empty();
    record_operation(
        "调用生图模型",
        "开始",
        &network_params,
        Some(proxy_used),
        None,
    );
    let client = match http_client_with_proxy(&provider.proxy_url, REQUEST_TIMEOUT_SECONDS, false) {
        Ok(client) => client,
        Err(error) => {
            record_operation(
                "调用生图模型",
                "失败",
                &network_params,
                Some(proxy_used),
                Some(&error),
            );
            return Err(error);
        }
    };
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
        let _ = emit_queue_updated(&app, &data_dir);
        return Ok(());
    }

    match result {
        Ok(images) => {
            let image_count = images.len();
            record_operation(
                "调用生图模型",
                "成功",
                format!("{network_params} image_count={image_count}"),
                Some(proxy_used),
                None,
            );
            let output_dir = output_dir_for(&data_dir, &settings)?;
            let outputs = match save_outputs(&output_dir, &task_id, &request, images) {
                Ok(outputs) => {
                    record_operation(
                        "写入生成图片",
                        "成功",
                        format!(
                            "task_id={} output_dir={} file_count={}",
                            task_id,
                            output_dir.display(),
                            outputs.len()
                        ),
                        None,
                        None,
                    );
                    outputs
                }
                Err(error) => {
                    record_operation(
                        "写入生成图片",
                        "失败",
                        format!("task_id={} output_dir={}", task_id, output_dir.display()),
                        None,
                        Some(&error),
                    );
                    return Err(error);
                }
            };
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
            let _ = emit_queue_updated(&app, &data_dir);
            Ok(())
        }
        Err(error) => {
            record_operation(
                "调用生图模型",
                "失败",
                &network_params,
                Some(proxy_used),
                Some(&error),
            );
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
                let _ = emit_queue_updated(&app, &data_dir);
                Ok(())
            } else {
                record.status = "failed".into();
                record.error = Some(error);
                record.completed_at = Some(utc_now());
                record.updated_at = utc_now();
                if !upsert_task_history(&app, &data_dir, record)? {
                    finish_deleted_task(&app, &data_dir, &task_id)?;
                } else {
                    let _ = emit_queue_updated(&app, &data_dir);
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
    let _ = emit_queue_updated(app, data_dir);
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
