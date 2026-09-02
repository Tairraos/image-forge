use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Local};
use rusqlite::{params, params_from_iter, Connection, Transaction};

use crate::models::{AgentLibraryPage, LibraryDayCount, LibraryPage, TaskRecord};

const DATABASE_FILE: &str = "library.sqlite";
const LEGACY_HISTORY_FILE: &str = "history.json";
const MIGRATION_KEY: &str = "history_json_v1";

pub(crate) fn initialize(data_dir: &Path, output_root: &Path) -> Result<(), String> {
    let mut connection = open(data_dir)?;
    if migration_completed(&connection)? {
        return Ok(());
    }
    let legacy_path = data_dir.join(LEGACY_HISTORY_FILE);
    if !legacy_path.is_file() {
        set_migration_completed(&mut connection)?;
        return Ok(());
    }

    let text =
        fs::read_to_string(&legacy_path).map_err(|error| format!("读取旧历史记录失败: {error}"))?;
    let mut records: Vec<TaskRecord> =
        serde_json::from_str(&text).map_err(|error| format!("解析旧历史记录失败: {error}"))?;
    let backup = data_dir.join("history.json.pre-sqlite.bak");
    if !backup.exists() {
        fs::copy(&legacy_path, &backup).map_err(|error| format!("备份旧历史记录失败: {error}"))?;
    }

    let mut moved = Vec::new();
    if let Err(error) = migrate_output_files(&mut records, output_root, &mut moved)
        .and_then(|_| import_legacy(&mut connection, &records))
    {
        rollback_moves(&moved);
        return Err(error);
    }

    let archived = unique_archive_path(data_dir);
    fs::rename(&legacy_path, &archived).map_err(|error| format!("归档旧历史记录失败: {error}"))?;
    Ok(())
}

fn unique_archive_path(data_dir: &Path) -> PathBuf {
    let first = data_dir.join("history.json.migrated");
    if !first.exists() {
        return first;
    }
    for index in 2.. {
        let candidate = data_dir.join(format!("history.json.migrated-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

pub(crate) fn read_all(data_dir: &Path) -> Result<Vec<TaskRecord>, String> {
    read_records(data_dir, None)
}

pub(crate) fn read_recent(data_dir: &Path, limit: usize) -> Result<Vec<TaskRecord>, String> {
    read_records(data_dir, Some(limit))
}

pub(crate) fn record(data_dir: &Path, task_id: &str) -> Result<Option<TaskRecord>, String> {
    let connection = open(data_dir)?;
    let mut statement = connection
        .prepare("SELECT record_json FROM tasks WHERE id = ?1")
        .map_err(db_error)?;
    let mut rows = statement.query([task_id]).map_err(db_error)?;
    let Some(row) = rows.next().map_err(db_error)? else {
        return Ok(None);
    };
    parse_record(row.get::<_, String>(0).map_err(db_error)?).map(Some)
}

pub(crate) fn replace_all(data_dir: &Path, records: &[TaskRecord]) -> Result<(), String> {
    let mut connection = open(data_dir)?;
    let transaction = connection.transaction().map_err(db_error)?;
    transaction
        .execute("DELETE FROM tasks", [])
        .map_err(db_error)?;
    for record in records {
        upsert_in_transaction(&transaction, record)?;
    }
    transaction.commit().map_err(db_error)
}

pub(crate) fn upsert(data_dir: &Path, record: &TaskRecord) -> Result<(), String> {
    let mut connection = open(data_dir)?;
    let transaction = connection.transaction().map_err(db_error)?;
    upsert_in_transaction(&transaction, record)?;
    transaction.commit().map_err(db_error)
}

pub(crate) fn library_page(
    data_dir: &Path,
    month: &str,
    date: &str,
    query: &str,
    origin: &str,
    page: u32,
    page_size: u32,
) -> Result<LibraryPage, String> {
    let connection = open(data_dir)?;
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);
    let mut conditions = vec![
        "tasks.status = 'completed'".to_string(),
        "EXISTS (SELECT 1 FROM task_outputs output WHERE output.task_id = tasks.id)".to_string(),
        "tasks.library_date LIKE ?".to_string(),
    ];
    let mut values = vec![format!("{}%", normalized_month(month))];
    if !date.trim().is_empty() {
        conditions.push("tasks.library_date = ?".into());
        values.push(date.trim().to_string());
    }
    match origin {
        "agent" => conditions.push("tasks.origin = 'agent'".into()),
        "drawing" => conditions.push("tasks.origin <> 'agent'".into()),
        _ => {}
    }
    if !query.trim().is_empty() {
        conditions.push("(tasks.id LIKE ? OR tasks.prompt LIKE ? OR tasks.model LIKE ? OR tasks.provider_name LIKE ?)".into());
        let pattern = format!("%{}%", query.trim());
        values.extend([pattern.clone(), pattern.clone(), pattern.clone(), pattern]);
    }
    let where_clause = conditions.join(" AND ");

    let total_tasks_sql = format!("SELECT COUNT(*) FROM tasks WHERE {where_clause}");
    let total_tasks = connection
        .query_row(&total_tasks_sql, params_from_iter(values.iter()), |row| {
            row.get::<_, u64>(0)
        })
        .map_err(db_error)?;
    let total_sql = format!(
        "SELECT COUNT(output.path) FROM tasks JOIN task_outputs output ON output.task_id = tasks.id WHERE {where_clause}"
    );
    let total_images = connection
        .query_row(&total_sql, params_from_iter(values.iter()), |row| {
            row.get::<_, u64>(0)
        })
        .map_err(db_error)?;

    let day_conditions = conditions
        .iter()
        .filter(|condition| condition.as_str() != "tasks.library_date = ?")
        .cloned()
        .collect::<Vec<_>>();
    let mut day_values = values.clone();
    if !date.trim().is_empty() {
        day_values.remove(1);
    }
    let day_sql = format!(
        "SELECT tasks.library_date, COUNT(output.path) FROM tasks JOIN task_outputs output ON output.task_id = tasks.id WHERE {} GROUP BY tasks.library_date ORDER BY tasks.library_date",
        day_conditions.join(" AND ")
    );
    let mut day_statement = connection.prepare(&day_sql).map_err(db_error)?;
    let day_counts = day_statement
        .query_map(params_from_iter(day_values.iter()), |row| {
            Ok(LibraryDayCount {
                date: row.get(0)?,
                image_count: row.get(1)?,
            })
        })
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;

    let tasks_sql = format!(
        "SELECT record_json FROM tasks WHERE {where_clause} ORDER BY created_at DESC LIMIT ? OFFSET ?"
    );
    let mut task_values = values;
    task_values.push(page_size.to_string());
    task_values.push(((page - 1) * page_size).to_string());
    let mut task_statement = connection.prepare(&tasks_sql).map_err(db_error)?;
    let tasks = task_statement
        .query_map(params_from_iter(task_values.iter()), |row| {
            row.get::<_, String>(0)
        })
        .map_err(db_error)?
        .map(|value| value.map_err(db_error).and_then(parse_record))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LibraryPage {
        tasks,
        day_counts,
        total_tasks,
        total_images,
        page,
        page_size,
    })
}

/// agent 视图内嵌图片库：无关键词时按月份列出图片，有关键词时跨月份搜索，
/// 并返回所有有图片的月份列表（供月份选择器与上/下月切换）。
pub(crate) fn agent_library(
    data_dir: &Path,
    month: &str,
    query: &str,
) -> Result<AgentLibraryPage, String> {
    let connection = open(data_dir)?;
    let query = query.trim();
    let mut conditions = vec![
        "tasks.status = 'completed'".to_string(),
        "EXISTS (SELECT 1 FROM task_outputs output WHERE output.task_id = tasks.id)".to_string(),
    ];
    let mut values: Vec<String> = Vec::new();
    if query.is_empty() {
        conditions.push("tasks.library_date LIKE ?".into());
        values.push(format!("{}%", normalized_month(month)));
    } else {
        conditions.push("(tasks.prompt LIKE ? OR tasks.model LIKE ? OR tasks.provider_name LIKE ? OR tasks.id LIKE ?)".into());
        let pattern = format!("%{query}%");
        values.extend([pattern.clone(), pattern.clone(), pattern.clone(), pattern]);
    }
    let where_clause = conditions.join(" AND ");

    let total_sql = format!(
        "SELECT COUNT(output.path) FROM tasks JOIN task_outputs output ON output.task_id = tasks.id WHERE {where_clause}"
    );
    let total_images = connection
        .query_row(&total_sql, params_from_iter(values.iter()), |row| {
            row.get::<_, u64>(0)
        })
        .map_err(db_error)?;

    let tasks_sql =
        format!("SELECT record_json FROM tasks WHERE {where_clause} ORDER BY created_at DESC");
    let mut task_statement = connection.prepare(&tasks_sql).map_err(db_error)?;
    let tasks = task_statement
        .query_map(params_from_iter(values.iter()), |row| {
            row.get::<_, String>(0)
        })
        .map_err(db_error)?
        .map(|value| value.map_err(db_error).and_then(parse_record))
        .collect::<Result<Vec<_>, _>>()?;

    let months_sql =
        "SELECT substr(tasks.library_date, 1, 7) AS month, COUNT(output.path) AS image_count
        FROM tasks JOIN task_outputs output ON output.task_id = tasks.id
        WHERE tasks.status = 'completed'
          AND EXISTS (SELECT 1 FROM task_outputs present WHERE present.task_id = tasks.id)
        GROUP BY month ORDER BY month DESC";
    let mut month_statement = connection.prepare(months_sql).map_err(db_error)?;
    let months = month_statement
        .query_map([], |row| {
            Ok(LibraryDayCount {
                date: row.get(0)?,
                image_count: row.get(1)?,
            })
        })
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;

    Ok(AgentLibraryPage {
        tasks,
        months,
        total_images,
    })
}

fn open(data_dir: &Path) -> Result<Connection, String> {
    let connection = Connection::open(data_dir.join(DATABASE_FILE)).map_err(db_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(db_error)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")
        .map_err(db_error)?;
    let schema_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
        .map_err(db_error)?;
    if schema_version == 0 {
        initialize_schema(&connection)?;
    }
    if schema_version < 2 {
        migrate_json_to_sqlite(data_dir, &connection)?;
    }
    Ok(connection)
}

fn initialize_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS metadata (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS tasks (
               id TEXT PRIMARY KEY,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL,
               completed_at TEXT NOT NULL,
               library_date TEXT NOT NULL,
               prompt TEXT NOT NULL,
               model TEXT NOT NULL,
               provider_name TEXT NOT NULL,
               origin TEXT NOT NULL,
               task_group_id TEXT NOT NULL,
               status TEXT NOT NULL,
               record_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS task_outputs (
               task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
               position INTEGER NOT NULL,
               path TEXT NOT NULL,
               PRIMARY KEY (task_id, position)
             );
             CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at DESC);
             CREATE INDEX IF NOT EXISTS idx_tasks_origin ON tasks(origin);
             CREATE INDEX IF NOT EXISTS idx_tasks_task_group_id ON tasks(task_group_id);
             CREATE INDEX IF NOT EXISTS idx_tasks_library_date ON tasks(library_date DESC);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_task_outputs_path ON task_outputs(path);
             CREATE TABLE IF NOT EXISTS app_settings (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS app_templates (
               id TEXT PRIMARY KEY,
               position INTEGER NOT NULL DEFAULT 0,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL,
               record_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS agent_sessions (
               id TEXT PRIMARY KEY,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL,
               title TEXT NOT NULL DEFAULT '',
               model_provider_id TEXT NOT NULL DEFAULT '',
               status TEXT NOT NULL DEFAULT 'idle',
               record_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS app_queue (
               id TEXT PRIMARY KEY,
               position INTEGER NOT NULL DEFAULT 0,
               status TEXT NOT NULL DEFAULT 'waiting',
               provider_id TEXT NOT NULL DEFAULT '',
               record_json TEXT NOT NULL
             );
             PRAGMA user_version = 2;",
        )
        .map_err(db_error)
}

fn read_records(data_dir: &Path, limit: Option<usize>) -> Result<Vec<TaskRecord>, String> {
    let connection = open(data_dir)?;
    let sql = match limit {
        Some(_) => "SELECT record_json FROM tasks ORDER BY created_at DESC LIMIT ?1",
        None => "SELECT record_json FROM tasks ORDER BY created_at DESC",
    };
    let mut statement = connection.prepare(sql).map_err(db_error)?;
    fn record_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<String> {
        row.get(0)
    }
    let rows = match limit {
        Some(limit) => statement.query_map([limit as i64], record_json),
        None => statement.query_map([], record_json),
    }
    .map_err(db_error)?;
    rows.map(|value| value.map_err(db_error).and_then(parse_record))
        .collect()
}

fn upsert_in_transaction(transaction: &Transaction<'_>, record: &TaskRecord) -> Result<(), String> {
    let record_json =
        serde_json::to_string(record).map_err(|error| format!("序列化任务记录失败: {error}"))?;
    transaction
        .execute(
            "INSERT INTO tasks (id, created_at, updated_at, completed_at, library_date, prompt, model, provider_name, origin, task_group_id, status, record_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
               created_at=excluded.created_at, updated_at=excluded.updated_at,
               completed_at=excluded.completed_at, library_date=excluded.library_date,
               prompt=excluded.prompt, model=excluded.model, provider_name=excluded.provider_name,
               origin=excluded.origin, task_group_id=excluded.task_group_id,
               status=excluded.status, record_json=excluded.record_json",
            params![
                record.id,
                record.created_at,
                record.updated_at,
                record.completed_at.as_deref().unwrap_or_default(),
                library_date(record),
                record.prompt,
                record.model,
                record.provider_name,
                normalized_origin(record),
                record.task_group_id,
                record.status,
                record_json,
            ],
        )
        .map_err(db_error)?;
    transaction
        .execute("DELETE FROM task_outputs WHERE task_id = ?1", [&record.id])
        .map_err(db_error)?;
    for (position, output) in record.outputs.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO task_outputs (task_id, position, path) VALUES (?1, ?2, ?3)",
                params![record.id, position as i64, output.path],
            )
            .map_err(db_error)?;
    }
    Ok(())
}

fn import_legacy(connection: &mut Connection, records: &[TaskRecord]) -> Result<(), String> {
    let transaction = connection.transaction().map_err(db_error)?;
    transaction
        .execute("DELETE FROM tasks", [])
        .map_err(db_error)?;
    for record in records {
        upsert_in_transaction(&transaction, record)?;
    }
    transaction
        .execute(
            "INSERT INTO metadata (key, value) VALUES (?1, '1') ON CONFLICT(key) DO UPDATE SET value='1'",
            [MIGRATION_KEY],
        )
        .map_err(db_error)?;
    transaction.commit().map_err(db_error)
}

fn migration_completed(connection: &Connection) -> Result<bool, String> {
    let mut statement = connection
        .prepare("SELECT value FROM metadata WHERE key = ?1")
        .map_err(db_error)?;
    let mut rows = statement.query([MIGRATION_KEY]).map_err(db_error)?;
    Ok(rows.next().map_err(db_error)?.is_some())
}

fn set_migration_completed(connection: &mut Connection) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO metadata (key, value) VALUES (?1, '1') ON CONFLICT(key) DO UPDATE SET value='1'",
            [MIGRATION_KEY],
        )
        .map(|_| ())
        .map_err(db_error)
}

fn migrate_output_files(
    records: &mut [TaskRecord],
    output_root: &Path,
    moved: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), String> {
    for record in records {
        let date = library_date(record);
        let target_dir = output_root.join(&date[..4]).join(&date[5..7]);
        fs::create_dir_all(&target_dir)
            .map_err(|error| format!("创建年月图片目录失败: {error}"))?;
        for output in &mut record.outputs {
            let source = PathBuf::from(&output.path);
            if !source.is_file() || source.starts_with(&target_dir) {
                continue;
            }
            let file_name = source
                .file_name()
                .ok_or_else(|| format!("旧图片路径无效: {}", source.display()))?;
            let target = unique_target(&target_dir, file_name);
            fs::rename(&source, &target)
                .map_err(|error| format!("迁移旧图片失败（{}）: {error}", source.display()))?;
            output.path = target.to_string_lossy().into_owned();
            output.file_name = target
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| output.file_name.clone());
            moved.push((source, target));
        }
    }
    Ok(())
}

fn rollback_moves(moved: &[(PathBuf, PathBuf)]) {
    for (source, target) in moved.iter().rev() {
        if target.is_file() {
            let _ = fs::rename(target, source);
        }
    }
}

fn unique_target(directory: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let target = directory.join(file_name);
    if !target.exists() {
        return target;
    }
    let source = Path::new(file_name);
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");
    for index in 2.. {
        let candidate = directory.join(format!("{stem}-{index}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn library_date(record: &TaskRecord) -> String {
    let value = record
        .completed_at
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(&record.created_at);
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| value.get(..10).unwrap_or("1970-01-01").to_string())
}

fn normalized_origin(record: &TaskRecord) -> &str {
    if record.origin == "agent"
        || !record.agent_session_id.is_empty()
        || !record.task_group_id.is_empty()
    {
        "agent"
    } else {
        "drawing"
    }
}

fn normalized_month(value: &str) -> String {
    if value.len() == 7
        && value.as_bytes()[4] == b'-'
        && value[..4].chars().all(|value| value.is_ascii_digit())
        && value[5..].chars().all(|value| value.is_ascii_digit())
    {
        value.to_string()
    } else {
        Local::now().format("%Y-%m").to_string()
    }
}

fn parse_record(value: String) -> Result<TaskRecord, String> {
    serde_json::from_str(&value).map_err(|error| format!("解析数据库任务记录失败: {error}"))
}

fn db_error(error: rusqlite::Error) -> String {
    format!("图片库数据库操作失败: {error}")
}

// ── JSON → SQLite 迁移 ──

fn migrate_json_to_sqlite(data_dir: &Path, connection: &Connection) -> Result<(), String> {
    // 迁移 settings.json
    let settings_path = data_dir.join("settings.json");
    if settings_path.is_file() {
        let text = fs::read_to_string(&settings_path)
            .map_err(|e| format!("读取 settings.json 失败: {e}"))?;
        connection
            .execute(
                "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
                params!["settings", text],
            )
            .map_err(db_error)?;
    }

    // 迁移 prompt-templates.json
    let templates_path = data_dir.join("prompt-templates.json");
    if templates_path.is_file() {
        let text = fs::read_to_string(&templates_path)
            .map_err(|e| format!("读取 prompt-templates.json 失败: {e}"))?;
        let templates: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
        let now = chrono::Local::now().to_rfc3339();
        for (i, tpl) in templates.iter().enumerate() {
            let id = tpl["id"].as_str().unwrap_or("").to_string();
            if id.is_empty() {
                continue;
            }
            connection
                .execute(
                    "INSERT OR REPLACE INTO app_templates (id, position, created_at, updated_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, i as i64, now, now, serde_json::to_string(tpl).unwrap_or_default()],
                )
                .map_err(db_error)?;
        }
    }

    // 迁移 agent/sessions/*.json
    let sessions_dir = data_dir.join("agent").join("sessions");
    let now = chrono::Local::now().to_rfc3339();
    if sessions_dir.is_dir() {
        for entry in
            fs::read_dir(&sessions_dir).map_err(|e| format!("读取 Agent 会话目录失败: {e}"))?
        {
            let entry = entry.map_err(|e| format!("读取 Agent 会话失败: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
            let session: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            let id = session["id"].as_str().unwrap_or("").to_string();
            if id.is_empty() {
                continue;
            }
            let title = session["title"].as_str().unwrap_or("").to_string();
            let model_provider_id = session["model_provider_id"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let status = session["status"].as_str().unwrap_or("idle").to_string();
            let created_at = session["created_at"].as_str().unwrap_or(&now).to_string();
            let updated_at = session["updated_at"].as_str().unwrap_or(&now).to_string();
            connection
                .execute(
                    "INSERT OR REPLACE INTO agent_sessions (id, created_at, updated_at, title, model_provider_id, status, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![id, created_at, updated_at, title, model_provider_id, status, text],
                )
                .map_err(db_error)?;
        }
    }

    // 迁移 queue.json
    let queue_path = data_dir.join("queue.json");
    if queue_path.is_file() {
        let text =
            fs::read_to_string(&queue_path).map_err(|e| format!("读取 queue.json 失败: {e}"))?;
        let queue: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let waiting = queue["waiting"].as_array().cloned().unwrap_or_default();
        let running = queue["running"].as_array().cloned().unwrap_or_default();
        for (i, id) in waiting.iter().enumerate() {
            let task_id = id.as_str().unwrap_or("").to_string();
            if task_id.is_empty() {
                continue;
            }
            connection
                .execute(
                    "INSERT OR REPLACE INTO app_queue (id, position, status, provider_id, record_json) VALUES (?1, ?2, 'waiting', '', ?3)",
                    params![task_id, i as i64, serde_json::to_string(&serde_json::json!({"taskId": task_id})).unwrap_or_default()],
                )
                .map_err(db_error)?;
        }
        for (i, run) in running.iter().enumerate() {
            let task_id = run["task_id"].as_str().unwrap_or("").to_string();
            if task_id.is_empty() {
                continue;
            }
            let provider_id = run["provider_id"].as_str().unwrap_or("").to_string();
            connection
                .execute(
                    "INSERT OR REPLACE INTO app_queue (id, position, status, provider_id, record_json) VALUES (?1, ?2, 'running', ?3, ?4)",
                    params![task_id, (waiting.len() + i) as i64, provider_id, serde_json::to_string(run).unwrap_or_default()],
                )
                .map_err(db_error)?;
        }
    }

    connection
        .execute_batch("PRAGMA user_version = 2;")
        .map_err(db_error)?;
    Ok(())
}

// ── 设置 CRUD ──

pub(crate) fn read_settings(data_dir: &Path) -> Result<Option<String>, String> {
    let connection = open(data_dir)?;
    let mut stmt = connection
        .prepare("SELECT value FROM app_settings WHERE key = 'settings'")
        .map_err(db_error)?;
    let mut rows = stmt.query([]).map_err(db_error)?;
    Ok(rows
        .next()
        .map_err(db_error)?
        .map(|row| row.get::<_, String>(0).unwrap_or_default()))
}

pub(crate) fn write_settings(data_dir: &Path, json: &str) -> Result<(), String> {
    let connection = open(data_dir)?;
    connection
        .execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('settings', ?1)",
            params![json],
        )
        .map_err(db_error)?;
    Ok(())
}

// ── 模板 CRUD ──

pub(crate) fn read_templates(data_dir: &Path) -> Result<Vec<String>, String> {
    let connection = open(data_dir)?;
    let mut stmt = connection
        .prepare("SELECT record_json FROM app_templates ORDER BY position ASC")
        .map_err(db_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    let mut templates = Vec::new();
    for row in rows {
        templates.push(row.map_err(db_error)?);
    }
    Ok(templates)
}

pub(crate) fn write_templates(data_dir: &Path, records: &[String]) -> Result<(), String> {
    let mut connection = open(data_dir)?;
    let transaction = connection.transaction().map_err(db_error)?;
    transaction
        .execute("DELETE FROM app_templates", [])
        .map_err(db_error)?;
    let now = chrono::Local::now().to_rfc3339();
    for (i, json) in records.iter().enumerate() {
        let tpl: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
        let id = tpl["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        transaction
            .execute(
                "INSERT INTO app_templates (id, position, created_at, updated_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, i as i64, now, now, json],
            )
            .map_err(db_error)?;
    }
    transaction.commit().map_err(db_error)
}

// ── Agent 会话 CRUD ──

pub(crate) fn read_agent_sessions(data_dir: &Path) -> Result<Vec<String>, String> {
    let connection = open(data_dir)?;
    let mut stmt = connection
        .prepare("SELECT record_json FROM agent_sessions ORDER BY updated_at DESC")
        .map_err(db_error)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row.map_err(db_error)?);
    }
    Ok(sessions)
}

pub(crate) fn read_agent_session(
    data_dir: &Path,
    session_id: &str,
) -> Result<Option<String>, String> {
    let connection = open(data_dir)?;
    let mut stmt = connection
        .prepare("SELECT record_json FROM agent_sessions WHERE id = ?1")
        .map_err(db_error)?;
    let mut rows = stmt.query(params![session_id]).map_err(db_error)?;
    Ok(rows
        .next()
        .map_err(db_error)?
        .map(|row| row.get::<_, String>(0).unwrap_or_default()))
}

pub(crate) fn upsert_agent_session(data_dir: &Path, session: &str) -> Result<(), String> {
    let connection = open(data_dir)?;
    let s: serde_json::Value = serde_json::from_str(session).unwrap_or_default();
    let id = s["id"].as_str().unwrap_or("").to_string();
    let title = s["title"].as_str().unwrap_or("").to_string();
    let model_provider_id = s["model_provider_id"].as_str().unwrap_or("").to_string();
    let status = s["status"].as_str().unwrap_or("idle").to_string();
    let now = chrono::Local::now().to_rfc3339();
    let created_at = s["created_at"].as_str().unwrap_or(&now).to_string();
    let updated_at = s["updated_at"].as_str().unwrap_or(&now).to_string();
    connection
        .execute(
            "INSERT OR REPLACE INTO agent_sessions (id, created_at, updated_at, title, model_provider_id, status, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, created_at, updated_at, title, model_provider_id, status, session],
        )
        .map_err(db_error)?;
    Ok(())
}

pub(crate) fn delete_agent_session(data_dir: &Path, session_id: &str) -> Result<(), String> {
    let connection = open(data_dir)?;
    let affected = connection
        .execute(
            "DELETE FROM agent_sessions WHERE id = ?1",
            params![session_id],
        )
        .map_err(db_error)?;
    if affected == 0 {
        return Err(format!("找不到 Agent 会话: {session_id}"));
    }
    Ok(())
}

// ── 队列 CRUD ──

pub(crate) fn read_queue_json(data_dir: &Path) -> Result<Option<String>, String> {
    let connection = open(data_dir)?;
    let mut stmt = connection
        .prepare("SELECT id, position, status, provider_id, record_json FROM app_queue ORDER BY position ASC")
        .map_err(db_error)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "position": row.get::<_, i64>(1)?,
                "status": row.get::<_, String>(2)?,
                "provider_id": row.get::<_, String>(3)?,
                "record": row.get::<_, String>(4)?,
            }))
        })
        .map_err(db_error)?;
    let items: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
    if items.is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::to_string(&items).unwrap_or_default()))
}

pub(crate) fn write_queue_items(data_dir: &Path, items: &str) -> Result<(), String> {
    let mut connection = open(data_dir)?;
    let transaction = connection.transaction().map_err(db_error)?;
    transaction
        .execute("DELETE FROM app_queue", [])
        .map_err(db_error)?;
    let items: Vec<serde_json::Value> = serde_json::from_str(items).unwrap_or_default();
    for (i, item) in items.iter().enumerate() {
        let id = item["id"].as_str().unwrap_or("").to_string();
        let status = item["status"].as_str().unwrap_or("waiting").to_string();
        let provider_id = item["provider_id"].as_str().unwrap_or("").to_string();
        let record = item["record"].as_str().unwrap_or("").to_string();
        transaction
            .execute(
                "INSERT INTO app_queue (id, position, status, provider_id, record_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, i as i64, status, provider_id, record],
            )
            .map_err(db_error)?;
    }
    transaction.commit().map_err(db_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::fallback_failed_record;
    use uuid::Uuid;

    fn root(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("image-forge-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(path.join("outputs")).unwrap();
        path
    }

    #[test]
    fn stores_records_and_queries_a_month_page() {
        let root = root("sqlite-page");
        initialize(&root, &root.join("outputs")).unwrap();
        let mut record = fallback_failed_record("task-1", "placeholder");
        record.status = "completed".into();
        record.prompt = "月光海岸".into();
        record.created_at = "2026-07-20T08:00:00+08:00".into();
        record.completed_at = Some(record.created_at.clone());
        record.outputs.push(crate::models::OutputImage {
            path: root
                .join("outputs/2026/07/image.png")
                .to_string_lossy()
                .into_owned(),
            file_name: "image.png".into(),
            mime_type: "image/png".into(),
            output_format: "png".into(),
            size: "1024x1024".into(),
            background: String::new(),
            quality: String::new(),
            revised_prompt: String::new(),
            usage: serde_json::Value::Null,
        });
        upsert(&root, &record).unwrap();

        let page = library_page(&root, "2026-07", "", "月光", "all", 1, 40).unwrap();
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.total_images, 1);
        assert_eq!(page.day_counts[0].date, "2026-07-20");
        let _ = trash::delete(&root);
    }

    #[test]
    fn agent_library_filters_by_month_or_searches_all_months() {
        let root = root("sqlite-agent-library");
        initialize(&root, &root.join("outputs")).unwrap();
        let mut july = fallback_failed_record("task-jul", "placeholder");
        july.status = "completed".into();
        july.prompt = "月光海岸".into();
        july.created_at = "2026-07-20T08:00:00+08:00".into();
        july.completed_at = Some(july.created_at.clone());
        july.outputs.push(crate::models::OutputImage {
            path: root
                .join("outputs/2026/07/july.png")
                .to_string_lossy()
                .into_owned(),
            file_name: "july.png".into(),
            mime_type: "image/png".into(),
            output_format: "png".into(),
            size: "1024x1024".into(),
            background: String::new(),
            quality: String::new(),
            revised_prompt: String::new(),
            usage: serde_json::Value::Null,
        });
        let mut august = fallback_failed_record("task-aug", "placeholder");
        august.status = "completed".into();
        august.prompt = "夏日沙滩".into();
        august.created_at = "2026-08-03T09:00:00+08:00".into();
        august.completed_at = Some(august.created_at.clone());
        august.outputs.push(crate::models::OutputImage {
            path: root
                .join("outputs/2026/08/august.png")
                .to_string_lossy()
                .into_owned(),
            file_name: "august.png".into(),
            mime_type: "image/png".into(),
            output_format: "png".into(),
            size: "1024x1024".into(),
            background: String::new(),
            quality: String::new(),
            revised_prompt: String::new(),
            usage: serde_json::Value::Null,
        });
        upsert(&root, &july).unwrap();
        upsert(&root, &august).unwrap();

        let month = agent_library(&root, "2026-07", "").unwrap();
        assert_eq!(month.tasks.len(), 1);
        assert_eq!(month.tasks[0].id, "task-jul");
        assert_eq!(month.total_images, 1);
        assert_eq!(month.months.len(), 2);
        assert_eq!(month.months[0].date, "2026-08");
        assert_eq!(month.months[0].image_count, 1);

        let search = agent_library(&root, "2026-07", "海岸").unwrap();
        assert_eq!(search.tasks.len(), 1);
        assert_eq!(search.tasks[0].id, "task-jul");

        let no_match = agent_library(&root, "2026-07", "不存在的关键词").unwrap();
        assert!(no_match.tasks.is_empty());
        assert_eq!(no_match.total_images, 0);
        let _ = trash::delete(&root);
    }

    #[test]
    fn migrates_legacy_json_and_images_into_year_month_directories() {
        let root = root("sqlite-migration");
        let source = root.join("outputs/legacy.png");
        fs::write(&source, b"png").unwrap();
        let mut record = fallback_failed_record("legacy", "placeholder");
        record.created_at = "2026-07-20T08:00:00+08:00".into();
        record.completed_at = Some(record.created_at.clone());
        record.outputs.push(crate::models::OutputImage {
            path: source.to_string_lossy().into_owned(),
            file_name: "legacy.png".into(),
            mime_type: "image/png".into(),
            output_format: "png".into(),
            size: String::new(),
            background: String::new(),
            quality: String::new(),
            revised_prompt: String::new(),
            usage: serde_json::Value::Null,
        });
        fs::write(
            root.join(LEGACY_HISTORY_FILE),
            serde_json::to_vec(&vec![record]).unwrap(),
        )
        .unwrap();

        initialize(&root, &root.join("outputs")).unwrap();
        let migrated = read_all(&root).unwrap();
        assert_eq!(migrated.len(), 1);
        assert!(Path::new(&migrated[0].outputs[0].path).ends_with("outputs/2026/07/legacy.png"));
        assert!(root.join("history.json.pre-sqlite.bak").is_file());
        assert!(root.join("history.json.migrated").is_file());
        let _ = trash::delete(&root);
    }
}
