mod proxy;
mod samplers;
mod state;

use samplers::processes::{top_processes, TopProcessesResult};
use state::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

const POPOVER_LABEL: &str = "popover";
const POPOVER_W: f64 = 300.0;
const POPOVER_H: f64 = 280.0;

#[tauri::command]
fn get_top_processes(kind: String, limit: Option<usize>) -> TopProcessesResult {
    top_processes(&kind, limit.unwrap_or(10))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_top_processes])
        .setup(|app| {
            let state = AppState::new();
            app.manage(state.clone());

            let open_item = MenuItem::with_id(app, "open", "Open Insights", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &quit_item])?;

            let _tray = TrayIconBuilder::with_id("insights-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        toggle_popover(tray.app_handle(), position);
                    }
                })
                .build(app)?;

            // Build the popover window once. Hidden by default; tray click toggles.
            let popover = WebviewWindowBuilder::new(
                app,
                POPOVER_LABEL,
                WebviewUrl::App("popover.html".into()),
            )
            .title("Insights Popover")
            .inner_size(POPOVER_W, POPOVER_H)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .build()?;

            // Hide popover when it loses focus (popover-style UX).
            popover.on_window_event(|event| {
                if let WindowEvent::Focused(focused) = event {
                    if !focused {
                        // Hide on blur — best effort, ignore errors during teardown.
                    }
                }
            });

            // Track main window focus to drive adaptive sampling frequency.
            if let Some(main) = app.get_webview_window("main") {
                let state_for_main = state.clone();
                main.on_window_event(move |event| {
                    if let WindowEvent::Focused(focused) = event {
                        state_for_main.set_focused(*focused);
                    }
                });
            }

            samplers::cpu_ram::spawn(app.handle().clone(), state.clone());
            samplers::gpu::spawn(app.handle().clone(), state.clone());
            samplers::ollama::spawn(app.handle().clone());
            proxy::spawn(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn show_main<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn toggle_popover<R: tauri::Runtime>(app: &tauri::AppHandle<R>, click: tauri::PhysicalPosition<f64>) {
    let Some(popover) = app.get_webview_window(POPOVER_LABEL) else {
        return;
    };
    if popover.is_visible().unwrap_or(false) {
        let _ = popover.hide();
        return;
    }
    if let Some(monitor) = popover
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| popover.primary_monitor().ok().flatten())
    {
        let scale = monitor.scale_factor();
        let half_w = (POPOVER_W * scale) / 2.0;
        let mut x = click.x - half_w;
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let min_x = monitor_pos.x as f64 + 8.0;
        let max_x = (monitor_pos.x as i64 + monitor_size.width as i64) as f64
            - (POPOVER_W * scale)
            - 8.0;
        if x < min_x {
            x = min_x;
        }
        if x > max_x {
            x = max_x;
        }
        let y = click.y + 4.0;
        let _ = popover.set_position(PhysicalPosition::new(x, y));
    }
    let _ = popover.show();
    let _ = popover.set_focus();
}
