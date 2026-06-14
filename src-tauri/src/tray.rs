use tauri::image::Image;
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::panel;

fn should_toggle_panel(button_state: MouseButtonState) -> bool {
    button_state == MouseButtonState::Up
}

pub fn create(app_handle: &AppHandle) -> tauri::Result<()> {
    let tray_icon_path = app_handle
        .path()
        .resolve("icons/tray-icon.png", BaseDirectory::Resource)?;
    let icon = Image::from_path(tray_icon_path)?;

    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Codex 用量")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button_state, .. } = event {
                if should_toggle_panel(button_state) {
                    panel::toggle_panel(tray.app_handle());
                }
            }
        })
        .build(app_handle)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_click_up_toggles_panel() {
        assert!(should_toggle_panel(MouseButtonState::Up));
    }

    #[test]
    fn tray_click_down_does_not_toggle_panel() {
        assert!(!should_toggle_panel(MouseButtonState::Down));
    }
}
