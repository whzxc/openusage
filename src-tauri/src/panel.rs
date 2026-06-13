#[cfg(target_os = "macos")]
mod platform {
    use tauri::{AppHandle, Emitter, Manager, Position, Size};
    use tauri_nspanel::{
        CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt, tauri_panel,
    };

    fn monitor_contains_physical_point(
        origin_x: f64,
        origin_y: f64,
        width: f64,
        height: f64,
        point_x: f64,
        point_y: f64,
    ) -> bool {
        point_x >= origin_x
            && point_x < origin_x + width
            && point_y >= origin_y
            && point_y < origin_y + height
    }

    unsafe fn set_panel_frame_top_left(panel: &tauri_nspanel::NSPanel, x: f64, y: f64) {
        let point = tauri_nspanel::NSPoint::new(x, y);
        let _: () = objc2::msg_send![panel, setFrameTopLeftPoint: point];
    }

    fn set_panel_top_left_immediately(
        window: &tauri::WebviewWindow,
        app_handle: &AppHandle,
        panel_x: f64,
        panel_y: f64,
        primary_logical_h: f64,
    ) {
        let Ok(panel_handle) = app_handle.get_webview_panel("main") else {
            return;
        };

        let target_x = panel_x;
        let target_y = primary_logical_h - panel_y;

        if objc2_foundation::MainThreadMarker::new().is_some() {
            unsafe {
                set_panel_frame_top_left(panel_handle.as_panel(), target_x, target_y);
            }
            return;
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let panel_handle = panel_handle.clone();

        if let Err(error) = window.run_on_main_thread(move || {
            unsafe {
                set_panel_frame_top_left(panel_handle.as_panel(), target_x, target_y);
            }
            let _ = tx.send(());
        }) {
            log::warn!("Failed to position panel on main thread: {}", error);
            return;
        }

        if rx.recv().is_err() {
            log::warn!("Failed waiting for panel position on main thread");
        }
    }

    macro_rules! get_or_init_panel {
        ($app_handle:expr) => {
            match $app_handle.get_webview_panel("main") {
                Ok(panel) => Some(panel),
                Err(_) => {
                    if let Err(err) = crate::panel::init($app_handle) {
                        log::error!("Failed to init panel: {}", err);
                        None
                    } else {
                        match $app_handle.get_webview_panel("main") {
                            Ok(panel) => Some(panel),
                            Err(err) => {
                                log::error!("Panel missing after init: {:?}", err);
                                None
                            }
                        }
                    }
                }
            }
        };
    }

    fn position_panel_from_tray(app_handle: &AppHandle) {
        let Some(tray) = app_handle.tray_by_id("tray") else {
            log::debug!("position_panel_from_tray: tray icon not found");
            return;
        };
        match tray.rect() {
            Ok(Some(rect)) => {
                position_panel_at_tray_icon(app_handle, rect.position, rect.size);
            }
            Ok(None) => {
                log::debug!("position_panel_from_tray: tray rect not available yet");
            }
            Err(e) => {
                log::warn!("position_panel_from_tray: failed to get tray rect: {}", e);
            }
        }
    }

    pub fn show_panel(app_handle: &AppHandle) {
        if let Some(panel) = get_or_init_panel!(app_handle) {
            let _ = app_handle.emit("panel-will-show", ());
            panel.show_and_make_key();
            position_panel_from_tray(app_handle);
        }
    }

    pub fn toggle_panel(app_handle: &AppHandle) {
        let Some(panel) = get_or_init_panel!(app_handle) else {
            return;
        };

        if panel.is_visible() {
            panel.hide();
        } else {
            let _ = app_handle.emit("panel-will-show", ());
            panel.show_and_make_key();
            position_panel_from_tray(app_handle);
        }
    }

    tauri_panel! {
        panel!(CodexUsagePanel {
            config: {
                can_become_key_window: true,
                is_floating_panel: true
            }
        })

        panel_event!(CodexUsagePanelEventHandler {
            window_did_resign_key(notification: &NSNotification) -> ()
        })
    }

    pub fn init(app_handle: &tauri::AppHandle) -> tauri::Result<()> {
        if app_handle.get_webview_panel("main").is_ok() {
            return Ok(());
        }

        let window = app_handle.get_webview_window("main").unwrap();
        let panel = window.to_panel::<CodexUsagePanel>()?;

        panel.set_has_shadow(false);
        panel.set_opaque(false);
        panel.set_level(PanelLevel::MainMenu.value() + 1);
        panel.set_collection_behavior(
            CollectionBehavior::new()
                .move_to_active_space()
                .full_screen_auxiliary()
                .value(),
        );
        panel.set_style_mask(StyleMask::empty().nonactivating_panel().value());

        let event_handler = CodexUsagePanelEventHandler::new();
        let handle = app_handle.clone();
        event_handler.window_did_resign_key(move |_notification| {
            if let Ok(panel) = handle.get_webview_panel("main") {
                panel.hide();
            }
        });
        panel.set_event_handler(Some(event_handler.as_ref()));

        Ok(())
    }

    pub fn position_panel_at_tray_icon(
        app_handle: &tauri::AppHandle,
        icon_position: Position,
        icon_size: Size,
    ) {
        let window = app_handle.get_webview_window("main").unwrap();

        let (icon_phys_x, icon_phys_y) = match &icon_position {
            Position::Physical(pos) => (pos.x as f64, pos.y as f64),
            Position::Logical(pos) => (pos.x, pos.y),
        };
        let (icon_phys_w, icon_phys_h) = match &icon_size {
            Size::Physical(s) => (s.width as f64, s.height as f64),
            Size::Logical(s) => (s.width, s.height),
        };

        let monitors = window.available_monitors().expect("failed to get monitors");
        let primary_logical_h = window
            .primary_monitor()
            .ok()
            .flatten()
            .map(|m| m.size().height as f64 / m.scale_factor())
            .unwrap_or(0.0);

        let icon_center_x = icon_phys_x + (icon_phys_w / 2.0);
        let icon_center_y = icon_phys_y + (icon_phys_h / 2.0);

        let found_monitor = monitors.iter().find(|monitor| {
            let origin = monitor.position();
            let size = monitor.size();
            monitor_contains_physical_point(
                origin.x as f64,
                origin.y as f64,
                size.width as f64,
                size.height as f64,
                icon_center_x,
                icon_center_y,
            )
        });

        let monitor = match found_monitor {
            Some(m) => m.clone(),
            None => match window.primary_monitor() {
                Ok(Some(m)) => m,
                _ => return,
            },
        };

        let target_scale = monitor.scale_factor();
        let mon_phys_x = monitor.position().x as f64;
        let mon_phys_y = monitor.position().y as f64;
        let mon_logical_x = mon_phys_x / target_scale;
        let mon_logical_y = mon_phys_y / target_scale;

        let icon_logical_x = mon_logical_x + (icon_phys_x - mon_phys_x) / target_scale;
        let icon_logical_y = mon_logical_y + (icon_phys_y - mon_phys_y) / target_scale;
        let icon_logical_w = icon_phys_w / target_scale;
        let icon_logical_h = icon_phys_h / target_scale;

        let panel_width = match (window.outer_size(), window.scale_factor()) {
            (Ok(s), Ok(win_scale)) => s.width as f64 / win_scale,
            _ => 420.0,
        };

        let icon_center_x = icon_logical_x + (icon_logical_w / 2.0);
        let panel_x = icon_center_x - (panel_width / 2.0);
        let panel_y = (icon_logical_y + icon_logical_h - 6.0).max(mon_logical_y);

        set_panel_top_left_immediately(&window, app_handle, panel_x, panel_y, primary_logical_h);
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Position, Size};

    const WINDOW_MARGIN_PX: f64 = 12.0;
    const TRAY_GAP_PX: f64 = 8.0;

    #[cfg(target_os = "windows")]
    fn remove_window_border(window: &tauri::WebviewWindow) {
        use windows::Win32::Graphics::Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DwmSetWindowAttribute,
        };

        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let border_color = DWMWA_COLOR_NONE;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &border_color as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&border_color) as u32,
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn remove_window_border(_window: &tauri::WebviewWindow) {}

    fn physical_position(position: &Position) -> (f64, f64) {
        match position {
            Position::Physical(pos) => (pos.x as f64, pos.y as f64),
            Position::Logical(pos) => (pos.x, pos.y),
        }
    }

    fn physical_size(size: &Size) -> (f64, f64) {
        match size {
            Size::Physical(size) => (size.width as f64, size.height as f64),
            Size::Logical(size) => (size.width, size.height),
        }
    }

    fn clamp(value: f64, min: f64, max: f64) -> f64 {
        value.max(min).min(max)
    }

    fn monitor_for_point(
        window: &tauri::WebviewWindow,
        point_x: f64,
        point_y: f64,
    ) -> Option<tauri::Monitor> {
        let monitors = window.available_monitors().ok()?;
        monitors
            .into_iter()
            .find(|monitor| {
                let origin = monitor.position();
                let size = monitor.size();
                point_x >= origin.x as f64
                    && point_x < origin.x as f64 + size.width as f64
                    && point_y >= origin.y as f64
                    && point_y < origin.y as f64 + size.height as f64
            })
            .or_else(|| window.current_monitor().ok().flatten())
            .or_else(|| window.primary_monitor().ok().flatten())
    }

    fn fallback_monitor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
        window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.primary_monitor().ok().flatten())
    }

    fn position_window(app_handle: &AppHandle, window: &tauri::WebviewWindow) {
        let Ok(window_size) = window.outer_size() else {
            return;
        };
        let win_w = window_size.width as f64;
        let win_h = window_size.height as f64;

        if let Some(tray) = app_handle.tray_by_id("tray") {
            if let Ok(Some(rect)) = tray.rect() {
                let (tray_x, tray_y) = physical_position(&rect.position);
                let (tray_w, tray_h) = physical_size(&rect.size);
                let tray_center_x = tray_x + tray_w / 2.0;
                let tray_center_y = tray_y + tray_h / 2.0;

                if let Some(monitor) = monitor_for_point(window, tray_center_x, tray_center_y) {
                    let origin = monitor.position();
                    let size = monitor.size();
                    let min_x = origin.x as f64 + WINDOW_MARGIN_PX;
                    let max_x = origin.x as f64 + size.width as f64 - win_w - WINDOW_MARGIN_PX;
                    let min_y = origin.y as f64 + WINDOW_MARGIN_PX;
                    let max_y = origin.y as f64 + size.height as f64 - win_h - WINDOW_MARGIN_PX;
                    let x = clamp(tray_center_x - win_w / 2.0, min_x, max_x);
                    let preferred_y = tray_y - win_h - TRAY_GAP_PX;
                    let fallback_y = tray_y + tray_h + TRAY_GAP_PX;
                    let y = if preferred_y >= min_y {
                        preferred_y
                    } else {
                        fallback_y
                    };
                    let y = clamp(y, min_y, max_y);
                    let _ = window
                        .set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
                    return;
                }
            }
        }

        if let Some(monitor) = fallback_monitor(window) {
            let origin = monitor.position();
            let size = monitor.size();
            let x = origin.x as f64 + size.width as f64 - win_w - WINDOW_MARGIN_PX;
            let y = origin.y as f64 + size.height as f64 - win_h - WINDOW_MARGIN_PX;
            let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
        }
    }

    pub fn init(app_handle: &AppHandle) -> tauri::Result<()> {
        if let Some(window) = app_handle.get_webview_window("main") {
            remove_window_border(&window);
            window.hide()?;
        }
        Ok(())
    }

    pub fn show_panel(app_handle: &AppHandle) {
        if let Some(window) = app_handle.get_webview_window("main") {
            remove_window_border(&window);
            position_window(app_handle, &window);
            let _ = app_handle.emit("panel-will-show", ());
            let _ = window.show();
            let _ = window.set_focus();
            remove_window_border(&window);
        }
    }

    pub fn toggle_panel(app_handle: &AppHandle) {
        let Some(window) = app_handle.get_webview_window("main") else {
            return;
        };

        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                remove_window_border(&window);
                position_window(app_handle, &window);
                let _ = app_handle.emit("panel-will-show", ());
                let _ = window.show();
                let _ = window.set_focus();
                remove_window_border(&window);
            }
        }
    }
}

pub use platform::{init, show_panel, toggle_panel};
