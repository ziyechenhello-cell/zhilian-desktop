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

fn logline(msg: &str) {
    use std::io::Write;
    let p = std::env::temp_dir().join("zl_capture.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "{}", msg);
    }
}

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
    logline("open_capture: enter");
    if let Some(w) = app.get_webview_window("capture") {
        let _ = w.show();
        let _ = w.set_focus();
        logline("open_capture: re-show");
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
        .map_err(|e| {
            logline(&format!("open_capture: build err {}", e));
            e.to_string()
        })?;
    logline("open_capture: built");
    Ok(())
}

/// 抓取取景框内部区域，交给主窗口贴进输入框。
#[tauri::command]
async fn capture_frame(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    logline("capture_frame: enter");

    let pos = window.outer_position().map_err(|e| e.to_string())?;
    logline(&format!("capture_frame: pos {},{}", pos.x, pos.y));
    let size = window.outer_size().map_err(|e| e.to_string())?;
    logline(&format!("capture_frame: size {}x{}", size.width, size.height));

    const INSET: i32 = 4;
    let x = pos.x + INSET;
    let y = pos.y + INSET;
    let w = (size.width as i32 - INSET * 2).max(16);
    let h = (size.height as i32 - INSET * 2).max(16);

    // 先藏起来，免得把取景框自己的边框拍进去
    let _ = window.hide();
    logline("capture_frame: hidden");
    std::thread::sleep(std::time::Duration::from_millis(200));

    let cache = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let script = cache.join("capture.ps1");
    std::fs::write(&script, PS).map_err(|e| e.to_string())?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out = std::env::temp_dir().join(format!("zl_shot_{}.png", stamp));

    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .args(["-X", &x.to_string(), "-Y", &y.to_string(), "-W", &w.to_string(), "-H", &h.to_string(), "-Out"])
        .arg(&out)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let mut child = cmd.spawn().map_err(|e| {
        logline(&format!("capture_frame: spawn err {}", e));
        e.to_string()
    })?;
    logline(&format!("capture_frame: spawned pid {}", child.id()));

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut ok = false;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                ok = st.success();
                logline(&format!("capture_frame: exit {:?}", st.code()));
                break;
            }
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    logline("capture_frame: timeout");
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
            Err(e) => {
                logline(&format!("capture_frame: wait err {}", e));
                break;
            }
        }
    }

    if !ok {
        let _ = window.show();
        return Err("截图失败：抓屏进程未正常结束".into());
    }

    let bytes = match std::fs::read(&out) {
        Ok(b) => b,
        Err(e) => {
            let _ = window.show();
            logline(&format!("capture_frame: read err {}", e));
            return Err(format!("读取截图失败：{}", e));
        }
    };
    logline(&format!("capture_frame: png {} bytes", bytes.len()));
    let _ = std::fs::remove_file(&out);

    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
        let payload = serde_json::json!({
            "data": format!("data:image/png;base64,{}", b64(&bytes)),
            "width": w,
            "height": h
        });
        match main.emit("zl-shot", payload) {
            Ok(_) => logline("capture_frame: emitted"),
            Err(e) => logline(&format!("capture_frame: emit err {}", e)),
        }
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
