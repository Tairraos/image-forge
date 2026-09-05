mod commands;
mod defaults;
mod history_db;
#[doc(hidden)]
pub mod integration_checks;
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
        .menu(|handle| {
            use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

            let settings = MenuItem::with_id(
                handle,
                "open-settings",
                "设置...",
                true,
                Some("CmdOrCtrl+,"),
            )?;

            #[cfg(target_os = "macos")]
            {
                let app_name = handle.package_info().name.clone();
                Menu::with_items(
                    handle,
                    &[
                        &Submenu::with_items(
                            handle,
                            app_name,
                            true,
                            &[
                                &settings,
                                &PredefinedMenuItem::separator(handle)?,
                                &PredefinedMenuItem::services(handle, None)?,
                                &PredefinedMenuItem::separator(handle)?,
                                &PredefinedMenuItem::hide(handle, None)?,
                                &PredefinedMenuItem::hide_others(handle, None)?,
                                &PredefinedMenuItem::show_all(handle, None)?,
                                &PredefinedMenuItem::separator(handle)?,
                                &PredefinedMenuItem::quit(handle, Some("退出 Image Forge"))?,
                            ],
                        )?,
                        &Submenu::with_items(
                            handle,
                            "编辑",
                            true,
                            &[
                                &PredefinedMenuItem::undo(handle, None)?,
                                &PredefinedMenuItem::redo(handle, None)?,
                                &PredefinedMenuItem::separator(handle)?,
                                &PredefinedMenuItem::cut(handle, None)?,
                                &PredefinedMenuItem::copy(handle, None)?,
                                &PredefinedMenuItem::paste(handle, None)?,
                                &PredefinedMenuItem::select_all(handle, None)?,
                            ],
                        )?,
                    ],
                )
            }

            #[cfg(not(target_os = "macos"))]
            {
                Menu::with_items(
                    handle,
                    &[
                        &Submenu::with_items(
                            handle,
                            "文件",
                            true,
                            &[
                                &settings,
                                &PredefinedMenuItem::separator(handle)?,
                                &PredefinedMenuItem::quit(handle, Some("退出"))?,
                            ],
                        )?,
                        &Submenu::with_items(
                            handle,
                            "编辑",
                            true,
                            &[
                                &PredefinedMenuItem::undo(handle, None)?,
                                &PredefinedMenuItem::redo(handle, None)?,
                                &PredefinedMenuItem::separator(handle)?,
                                &PredefinedMenuItem::cut(handle, None)?,
                                &PredefinedMenuItem::copy(handle, None)?,
                                &PredefinedMenuItem::paste(handle, None)?,
                                &PredefinedMenuItem::select_all(handle, None)?,
                            ],
                        )?,
                    ],
                )
            }
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() != "open-settings" {
                return;
            }
            use tauri::{Emitter, Manager};
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit("menu-open-settings", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::about_info,
            commands::agent_library,
            commands::read_clipboard_text,
            commands::create_agent_session,
            commands::create_agent_direct_image_task,
            commands::cancel_agent_task_group,
            commands::get_task_status,
            commands::retry_agent_task_group,
            commands::cleanup_data_files,
            commands::cancel_agent_turn,
            commands::delete_task,
            commands::delete_agent_session,
            commands::delete_template,
            commands::download_output,
            commands::export_templates,
            commands::export_data_bundle,
            commands::fill_prompt_template,
            commands::import_templates,
            commands::import_data_bundle,
            commands::load_app_state,
            commands::list_provider_models,
            commands::move_template,
            commands::queue_snapshot,
            commands::redraw_task,
            commands::reference_from_clipboard,
            commands::reference_from_path,
            commands::rename_agent_session,
            commands::reveal_path,
            commands::runtime_logs,
            commands::list_agent_sessions,
            commands::get_agent_session,
            commands::send_agent_message,
            commands::scan_cleanup_candidates,
            commands::save_settings,
            commands::save_template,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use super::services::{images::reference_preview, references::persist_reference_bytes};
    use super::store::{default_template_title, normalize_model_type};
    use super::utils::{
        image_prompt_for_transport, normalize_base_url, prompt_with_ratio_instruction, sanitize_id,
        should_send_input_fidelity, size_for_preset,
    };

    #[test]
    fn normalize_base_url_strips_known_endpoints() {
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/images/generations").unwrap(),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_base_url("https://api.openai.com/v1/models").unwrap(),
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

    #[test]
    fn legacy_image_provider_types_are_inferred_from_model_and_url() {
        assert_eq!(
            normalize_model_type("image", "gemini-3.1-flash-image", ""),
            "image-gemini"
        );
        assert_eq!(
            normalize_model_type("image", "grok-imagine-image-quality", ""),
            "image-grok"
        );
        assert_eq!(
            normalize_model_type("image", "gpt-image-2", ""),
            "image-gpt"
        );
        assert_eq!(
            normalize_model_type("image-grok", "gpt-image-2", ""),
            "image-grok"
        );
        assert_eq!(normalize_model_type("chat", "gemini-3.5-flash", ""), "chat");
    }

    #[test]
    fn template_title_uses_the_first_line_and_limits_unicode_length() {
        assert_eq!(
            default_template_title("第一行标题\n第二行内容"),
            "第一行标题"
        );
        let long_title = "字".repeat(30);
        assert_eq!(default_template_title(&long_title), "字".repeat(24));
    }

    #[test]
    fn image_size_presets_match_reference_project() {
        assert_eq!(size_for_preset("standard", "4:5"), "1024x1280");
        assert_eq!(size_for_preset("2k", "16:9"), "2048x1152");
        assert_eq!(size_for_preset("4k", "9:21"), "1632x3808");
    }

    #[test]
    fn ratio_instruction_is_appended_once() {
        let prompt = prompt_with_ratio_instruction("一只猫", "16:9");
        assert_eq!(prompt, "一只猫\n\n将宽高比设为 16:9");
        assert_eq!(
            prompt_with_ratio_instruction(&prompt, "16:9"),
            "一只猫\n\n将宽高比设为 16:9"
        );
    }

    #[test]
    fn strict_prompt_fidelity_wraps_prompt_for_images_transport() {
        let prompt = image_prompt_for_transport("一只猫", "1:1", "strict");
        assert!(prompt.contains("提示词保真规则"));
        assert!(prompt.contains("用户原始提示词：\n一只猫\n\n将宽高比设为 1:1"));
        assert_eq!(
            image_prompt_for_transport("一只猫", "1:1", "off"),
            "一只猫\n\n将宽高比设为 1:1"
        );
    }

    #[test]
    fn reference_resources_are_deduplicated_by_content() {
        let data_dir = std::env::temp_dir().join(format!("image-forge-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&data_dir).unwrap();
        let first = persist_reference_bytes(&data_dir, b"same-image", "png").unwrap();
        let second = persist_reference_bytes(&data_dir, b"same-image", "jpg").unwrap();
        assert_eq!(first, second);
        assert_eq!(
            fs::read_dir(data_dir.join("references")).unwrap().count(),
            1
        );
        let _ = trash::delete(&data_dir);
    }

    #[test]
    fn reference_preview_rejects_text_with_image_extension() {
        let data_dir = std::env::temp_dir().join(format!("image-forge-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&data_dir).unwrap();
        let path = data_dir.join("not-an-image.png");
        fs::write(&path, b"plain text").unwrap();
        assert_eq!(reference_preview(&path).unwrap_err(), "图片文件无法解析");
        let _ = trash::delete(&data_dir);
    }
}
