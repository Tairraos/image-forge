fn main() {
    let build_time = std::process::Command::new("date")
        .env("TZ", "Asia/Shanghai")
        .arg("+%Y-%m-%d %H:%M:%S UTC+8")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=IMAGE_FORGE_BUILD_TIME={build_time}");
    tauri_build::build()
}
