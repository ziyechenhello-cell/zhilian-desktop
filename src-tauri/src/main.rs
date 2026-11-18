#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 截图脚本：先声明 DPI 感知，再按物理像素抓屏存 PNG。
const PS: &str = r#"param([int]$X,[int]$Y,[int]$W,[int]$H,[string]$Out)
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class ZlDpi{[DllImport("user32.dll")]public static extern bool SetProcessDPIAware();}'
[ZlDpi]::SetProcessDPIAware() | Out-Null
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap $W,$H
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($X,$Y,0,0,(New-Object System.Drawing.Size $W,$H))
$g.Dispose()
$bmp.Save($Out,[System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"#;

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for c in data.chunks(3) {
        let n = ((c[0] as u32) << 16) | ((*c.get(1).unwrap_or(&0) as u32) << 8) | (*c.get(2).unwrap_or(&0) as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// 打开（或唤起）桌面上的矩形取景框。
#[tauri::command]
async fn open_capture(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("capture") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, "capture", WebviewUrl::App("capture.html".into()))
        .title("智恋bot 截图")
        .inner_size(520.0, 360.0)
        .min_inner_size(160.0, 110.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .shadow(false)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 抓取取景框内部区域，交给主窗口贴进输入框。
#[tauri::command]
async fn capture_frame(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;

    const INSET: i32 = 4;
    let x = pos.x + INSET;
    let y = pos.y + INSET;
    let w = (size.width as i32 - INSET * 2).max(16);
    let h = (size.height as i32 - INSET * 2).max(16);

    // 先藏起来，免得把取景框自己的边框拍进去
    let _ = window.hide();
    std::thread::sleep(std::time::Duration::from_millis(180));

    let cache = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let script = cache.join("capture.ps1");
    std::fs::write(&script, PS).map_err(|e| e.to_string())?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out = std::env::temp_dir().join(format!("zl_shot_{}.png", stamp));

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
        .arg(&script)
        .args(["-X", &x.to_string(), "-Y", &y.to_string(), "-W", &w.to_string(), "-H", &h.to_string(), "-Out"])
        .arg(&out)
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        let _ = window.show();
        return Err(format!("截图失败（退出码 {:?}）", status.code()));
    }

    let bytes = std::fs::read(&out).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&out);

    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
        let payload = serde_json::json!({
            "data": format!("data:image/png;base64,{}", b64(&bytes)),
            "width": w,
            "height": h
        });
        main.emit("zl-shot", payload).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![open_capture, capture_frame])
        .setup(|app| {
            if let Some(win) = app.get_webview_window("main") {
                let js = format!("window.__ZL_DESKTOP_VERSION__ = '{}';", VERSION);
                let _ = win.eval(&js);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running 智恋bot");
}
