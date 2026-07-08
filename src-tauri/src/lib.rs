mod commands;
mod defaults;
mod models;
mod services;
mod state;
mod store;
mod utils;

use state::RuntimeState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(RuntimeState::new())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            if let Ok(data_dir) = store::ensure_data_dir(&handle) {
                let _ = services::queue::recover_stale_running(&handle, &data_dir);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::add_gallery_item,
            commands::cancel_task,
            commands::delete_gallery_item,
            commands::delete_snippet,
            commands::delete_template,
            commands::enqueue_generation,
            commands::load_app_state,
            commands::mark_template_used,
            commands::promote_task,
            commands::queue_snapshot,
            commands::reference_from_path,
            commands::retry_task,
            commands::reveal_path,
            commands::save_settings,
            commands::save_snippet,
            commands::save_template,
            commands::update_gallery_item
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::utils::{normalize_base_url, sanitize_id, should_send_input_fidelity};

    #[test]
    fn normalize_base_url_strips_known_endpoints() {
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/images/generations").unwrap(),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn input_fidelity_skips_gpt_image_2() {
        assert!(!should_send_input_fidelity("gpt-image-2", "high"));
        assert!(should_send_input_fidelity("gpt-image-1", "high"));
    }

    #[test]
    fn provider_ids_are_stable() {
        assert_eq!(sanitize_id("OpenAI Official"), "OpenAI-Official");
        assert_eq!(sanitize_id(""), "default");
    }
}
