use std::{borrow::Cow, fs, path::Path};

use arboard::{Clipboard, ImageData};
use chrono::Utc;
use tauri::AppHandle;
use uuid::Uuid;

use crate::{
    models::ReferencePreview, services::images::reference_preview, store::ensure_data_dir,
};

pub(crate) fn copy_image_to_clipboard(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("找不到要复制的图片".into());
    }
    let image = image::open(path).map_err(|error| format!("读取图片失败: {error}"))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut clipboard = Clipboard::new().map_err(|error| format!("打开剪贴板失败: {error}"))?;
    clipboard
        .set_image(ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(rgba.into_raw()),
        })
        .map_err(|error| format!("写入剪贴板失败: {error}"))
}

pub(crate) fn reference_from_clipboard(app: &AppHandle) -> Result<ReferencePreview, String> {
    let data_dir = ensure_data_dir(app)?;
    let mut clipboard = Clipboard::new().map_err(|error| format!("打开剪贴板失败: {error}"))?;
    let image = clipboard
        .get_image()
        .map_err(|error| format!("读取剪贴板图片失败: {error}"))?;
    let rgba = image::RgbaImage::from_raw(
        image.width as u32,
        image.height as u32,
        image.bytes.into_owned(),
    )
    .ok_or("剪贴板图片数据无效")?;
    let clipboard_dir = data_dir.join("clipboard");
    fs::create_dir_all(&clipboard_dir).map_err(|error| format!("创建剪贴板目录失败: {error}"))?;
    let file_name = format!(
        "clipboard-{}-{}.png",
        Utc::now().format("%Y%m%d-%H%M%S"),
        Uuid::new_v4()
    );
    let path = clipboard_dir.join(file_name);
    rgba.save_with_format(&path, image::ImageFormat::Png)
        .map_err(|error| format!("保存剪贴板图片失败: {error}"))?;
    reference_preview(Path::new(&path))
}
