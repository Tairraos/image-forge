// 反向同步：从浏览器 Web 版导出数据，写回桌面版 ~/.image-forge。
// 读取浏览器 localStorage 和 IndexedDB 数据，写入 settings.json、templates.json、SQLite。
//
// 用法：
//   1. 打开 Web 版 → F12 → Console → 粘贴以下代码 → 回车
//   2. 复制输出的 JSON，保存为 /tmp/if-export.json
//   3. 运行：node scripts/sync-web-to-desktop.mjs /tmp/if-export.json

// === 在浏览器控制台执行以下代码 ===
/*
(async function exportData() {
  const data = {};

  // 从 localStorage 读取
  ["if_settings", "if_templates", "if_agent_sessions"].forEach((key) => {
    const raw = localStorage.getItem(key);
    if (raw) {
      data[key] = JSON.parse(raw);
    }
  });

  // 从 IndexedDB 读取任务
  const db = await new Promise((resolve, reject) => {
    const req = indexedDB.open("ImageForge", 1);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  if (db.objectStoreNames.contains("tasks")) {
    const tx = db.transaction("tasks", "readonly");
    const store = tx.objectStore("tasks");
    const rows = await new Promise((resolve) => {
      const req = store.getAll();
      req.onsuccess = () => resolve(req.result);
    });
    data.tasks = rows.map((row) => JSON.parse(row.record_json));
  }
  db.close();

  copy(JSON.stringify(data, null, 2));
  console.log("✅ 已导出 " + (data.tasks?.length || 0) + " 条任务记录");
  console.log("📋 JSON 已复制到剪贴板，保存为文件后运行：");
  console.log("   node scripts/sync-web-to-desktop.mjs <文件路径>");
})();
*/

// === 以下为 Node.js 脚本 ===

import { readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import Database from "better-sqlite3";

const DATA_DIR = join(homedir(), ".image-forge");
const SETTINGS_FILE = join(DATA_DIR, "settings.json");
const TEMPLATES_FILE = join(DATA_DIR, "prompt-templates.json");
const AGENT_DIR = join(DATA_DIR, "agent", "sessions");
const SQLITE_FILE = join(DATA_DIR, "library.sqlite");

async function main() {
  const filePath = process.argv[2];
  if (!filePath) {
    console.log("用法: node scripts/sync-web-to-desktop.mjs <导出文件路径>");
    console.log("\n先导出 Web 数据：");
    console.log("  1. 打开 Web 版 → F12 → Console");
    console.log("  2. 粘贴本文件顶部的浏览器脚本，回车执行");
    console.log("  3. 保存输出的 JSON 到文件");
    console.log("  4. 运行: node scripts/sync-web-to-desktop.mjs <文件路径>");
    process.exit(1);
  }

  const raw = await readFile(filePath, "utf-8");
  const data = JSON.parse(raw);

  const results = [];

  // 写入设置
  if (data.if_settings) {
    await writeFile(SETTINGS_FILE, JSON.stringify(data.if_settings, null, 2));
    results.push(`✅ 设置（${data.if_settings.providers?.length || 0} 个 API 源）`);
  }

  // 写入模板
  if (data.if_templates) {
    await writeFile(TEMPLATES_FILE, JSON.stringify(data.if_templates, null, 2));
    results.push(`✅ 模板（${data.if_templates.length} 个）`);
  }

  // 写入 Agent 会话
  if (data.if_agent_sessions) {
    const { mkdir } = await import("node:fs/promises");
    await mkdir(AGENT_DIR, { recursive: true });
    for (const session of data.if_agent_sessions) {
      const sessionFile = join(AGENT_DIR, `${session.id}.json`);
      await writeFile(sessionFile, JSON.stringify(session, null, 2));
    }
    results.push(`✅ Agent 会话（${data.if_agent_sessions.length} 个）`);
  }

  // 写入 SQLite 图片库
  if (data.tasks?.length) {
    const db = new Database(SQLITE_FILE);
    const upsert = db.prepare(`
      INSERT INTO tasks (id, created_at, updated_at, completed_at, library_date, prompt, model, provider_name, origin, task_group_id, status, record_json)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET
        updated_at=excluded.updated_at, completed_at=excluded.completed_at,
        library_date=excluded.library_date, prompt=excluded.prompt, model=excluded.model,
        provider_name=excluded.provider_name, origin=excluded.origin,
        task_group_id=excluded.task_group_id, status=excluded.status, record_json=excluded.record_json
    `);
    const deleteOutputs = db.prepare("DELETE FROM task_outputs WHERE task_id = ?");
    const insertOutput = db.prepare("INSERT INTO task_outputs (task_id, position, path) VALUES (?, ?, ?)");

    const tx = db.transaction(() => {
      for (const task of data.tasks) {
        const now = task.updated_at || task.created_at || new Date().toISOString();
        upsert.run(
          task.id,
          task.created_at || "",
          now,
          task.completed_at || "",
          task.library_date || (task.completed_at || task.created_at || "").slice(0, 10),
          task.prompt || "",
          task.model || "",
          task.provider_name || task.providerName || "",
          task.origin || (task.agent_session_id || task.task_group_id ? "agent" : "drawing"),
          task.task_group_id || "",
          task.status || "completed",
          JSON.stringify(task),
        );
        deleteOutputs.run(task.id);
        if (task.outputs) {
          for (let i = 0; i < task.outputs.length; i++) {
            insertOutput.run(task.id, i, task.outputs[i].path || "");
          }
        }
      }
    });
    tx();
    db.close();
    results.push(`✅ 图片库（${data.tasks.length} 条记录）`);
  }

  console.log(results.join("\n"));
  console.log("🎉 反向同步完成！重新打开桌面版即可看到数据。");
}

main().catch((error) => {
  console.error("反向同步失败:", error.message);
  process.exit(1);
});