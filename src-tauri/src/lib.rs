#[cfg(target_os = "macos")]
mod app_nap;
mod codex;
mod log_path;
mod panel;
mod tray;

use tauri_plugin_log::{Target, TargetKind};

#[tauri::command]
async fn get_codex_usage() -> Result<codex::CodexUsageSnapshot, String> {
    tauri::async_runtime::spawn_blocking(codex::load_usage_snapshot)
        .await
        .map_err(|error| format!("Codex usage task failed: {error}"))?
}

#[tauri::command]
async fn refresh_codex_usage() -> Result<codex::CodexUsageSnapshot, String> {
    get_codex_usage().await
}

#[tauri::command]
fn get_log_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    log_path::for_app(&app_handle)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn quit_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("Codex 用量".to_string()),
                    }),
                ])
                .max_file_size(10_000_000)
                .level(log::LevelFilter::Info)
                .level_for("hyper", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_codex_usage,
            refresh_codex_usage,
            get_log_path,
            quit_app,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                app_nap::disable_app_nap();
            }

            log::info!("Codex 用量 v{} starting", app.package_info().version);
            panel::init(app.handle())?;
            tray::create(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
