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
        .invoke_handler(tauri::generate_handler![
            commands::about_info,
            commands::audit_skill_package,
            commands::copy_image_to_clipboard,
            commands::create_agent_session,
            commands::create_agent_direct_image_task,
            commands::create_agent_image_tasks,
            commands::cancel_agent_task_group,
            commands::get_task_status,
            commands::retry_agent_task_group,
            commands::cleanup_data_files,
            commands::cancel_agent_turn,
            commands::delete_task,
            commands::delete_agent_session,
            commands::delete_skill,
            commands::delete_template,
            commands::download_output,
            commands::enqueue_generation,
            commands::enqueue_generation_batch,
            commands::export_api_providers,
            commands::export_templates,
            commands::fill_prompt_template,
            commands::fetch_skill_markdown,
            commands::import_templates,
            commands::load_app_state,
            commands::list_provider_models,
            commands::mark_template_used,
            commands::move_template,
            commands::queue_snapshot,
            commands::read_api_providers_file,
            commands::read_skill_markdown_file,
            commands::reference_from_clipboard,
            commands::reference_from_path,
            commands::retry_task,
            commands::reveal_path,
            commands::runtime_logs,
            commands::list_agent_sessions,
            commands::get_agent_session,
            commands::install_skill,
            commands::use_skill,
            commands::send_agent_message,
            commands::scan_cleanup_candidates,
            commands::save_settings,
            commands::save_skill,
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
    use super::store::{
        default_template_title, is_safe_skill_directory, normalize_model_type, skill_directory_name,
    };
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
            normalize_model_type("image", "seedream-4-0", ""),
            "image-seedream"
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
    fn skill_directory_names_are_codex_style_and_path_safe() {
        assert_eq!(
            skill_directory_name("Image Director", "abc"),
            "image-director"
        );
        assert_eq!(skill_directory_name("构图 导演", "abc"), "构图-导演");
        assert!(is_safe_skill_directory("image-director"));
        assert!(!is_safe_skill_directory("../image-director"));
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
