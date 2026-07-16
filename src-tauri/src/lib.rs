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
            commands::about_info,
            commands::copy_image_to_clipboard,
            commands::delete_task,
            commands::delete_skill,
            commands::delete_template,
            commands::download_output,
            commands::enqueue_generation,
            commands::export_api_providers,
            commands::export_templates,
            commands::fill_prompt_template,
            commands::fill_skill_prompt,
            commands::fetch_skill_markdown,
            commands::import_templates,
            commands::load_app_state,
            commands::list_provider_models,
            commands::mark_template_used,
            commands::move_template,
            commands::queue_snapshot,
            commands::read_api_providers_file,
            commands::reference_from_clipboard,
            commands::reference_from_path,
            commands::retry_task,
            commands::reveal_path,
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

    use super::models::SkillEntry;
    use super::services::{images::reference_preview, references::persist_reference_bytes};
    use super::store::{
        default_template_title, normalize_model_type, normalize_skill, skill_name_from_markdown,
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
    fn skill_name_prefers_frontmatter_then_heading() {
        assert_eq!(
            skill_name_from_markdown("---\nname: Composition Director\n---\n# 其它标题"),
            "Composition Director"
        );
        assert_eq!(skill_name_from_markdown("# 光影导演\n\n内容"), "光影导演");
    }

    #[test]
    fn markdown_only_skills_reject_script_references() {
        let skill = SkillEntry {
            id: String::new(),
            name: String::new(),
            source_url: String::new(),
            content: "# 测试\n\n运行 [脚本](scripts/render.py)".into(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!(normalize_skill(skill).unwrap_err().contains("脚本"));
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
        fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn reference_preview_rejects_text_with_image_extension() {
        let data_dir = std::env::temp_dir().join(format!("image-forge-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&data_dir).unwrap();
        let path = data_dir.join("not-an-image.png");
        fs::write(&path, b"plain text").unwrap();
        assert_eq!(reference_preview(&path).unwrap_err(), "图片文件无法解析");
        fs::remove_dir_all(data_dir).unwrap();
    }
}
