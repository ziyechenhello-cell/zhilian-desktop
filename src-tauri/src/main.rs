#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            if let Some(win) = app.get_webview_window("main") {
                let js = format!(
                    "window.__ZL_DESKTOP_VERSION__ = '{}';",
                    VERSION
                );
                let _ = win.eval(&js);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running 智恋bot");
}