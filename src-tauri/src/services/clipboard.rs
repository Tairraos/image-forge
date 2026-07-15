use std::{
    borrow::Cow,
    io::Cursor,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
use std::process::Command;

use arboard::{Clipboard, ImageData};
use tauri::AppHandle;
use url::Url;

use crate::{
    models::ReferencePreview,
    services::{images::reference_preview, references::persist_reference_bytes},
    store::ensure_data_dir,
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

pub(crate) fn reference_from_clipboard(
    app: &AppHandle,
) -> Result<Option<ReferencePreview>, String> {
    if let Some(paths) = macos_clipboard_file_paths() {
        for path in paths {
            if let Ok(preview) = reference_preview(&path) {
                return Ok(Some(preview));
            }
        }
        return Ok(None);
    }

    let data_dir = ensure_data_dir(app)?;
    let mut clipboard = Clipboard::new().map_err(|error| format!("打开剪贴板失败: {error}"))?;
    let image = match clipboard.get_image() {
        Ok(image) => image,
        Err(_) => return Ok(None),
    };
    let rgba = image::RgbaImage::from_raw(
        image.width as u32,
        image.height as u32,
        image.bytes.into_owned(),
    )
    .ok_or("剪贴板图片数据无效")?;
    let mut cursor = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|error| format!("编码剪贴板图片失败: {error}"))?;
    let path = persist_reference_bytes(&data_dir, cursor.get_ref(), "png")?;
    reference_preview(Path::new(&path)).map(Some)
}

#[cfg(target_os = "macos")]
fn macos_clipboard_file_paths() -> Option<Vec<PathBuf>> {
    const SCRIPT: &str = r#"
ObjC.import("AppKit");
const pasteboard = $.NSPasteboard.generalPasteboard;
const items = pasteboard.pasteboardItems;
const fileUrls = [];
for (let index = 0; index < Number(items.count); index += 1) {
  const value = items.objectAtIndex(index).stringForType("public.file-url");
  if (value) fileUrls.push(ObjC.unwrap(value));
}
fileUrls.join("\n");
"#;
    let output = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let paths = parse_file_url_paths(&String::from_utf8_lossy(&output.stdout));
    (!paths.is_empty()).then_some(paths)
}

#[cfg(not(target_os = "macos"))]
fn macos_clipboard_file_paths() -> Option<Vec<PathBuf>> {
    None
}

fn parse_file_url_paths(value: &str) -> Vec<PathBuf> {
    value
        .lines()
        .filter_map(|line| Url::parse(line.trim()).ok())
        .filter(|url| url.scheme() == "file")
        .filter_map(|url| url.to_file_path().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_file_url_paths;

    #[test]
    fn parses_macos_file_urls() {
        let paths = parse_file_url_paths(
            "file:///Users/test/Picture%201.png\nhttps://example.com/image.png\n",
        );
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].to_string_lossy(), "/Users/test/Picture 1.png");
    }
}
