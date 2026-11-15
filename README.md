# 智恋bot 桌面客户端（Tauri 2）

外壳只负责「开窗」，页面、账号、额度、计费全部在 https://zhilianbot.com 上，
所以服务端改完客户端无需重新发版。

## 目录

```
desktop/
  ui/index.html            启动页（立刻跳转到官网）
  src-tauri/
    tauri.conf.json        窗口尺寸、安装包（NSIS）、图标
    Cargo.toml / build.rs
    src/main.rs            入口（无自定义命令，纯 webview）
    icons/                 icon.ico / icon.png / 32x32 / 128x128
  .github/workflows/build-windows.yml   CI 出 Windows 安装包
```

## 本机构建（需要 Rust + VS Build Tools）

1. 安装 Rust：https://rustup.rs  （勾选 `x86_64-pc-windows-msvc`）
2. 安装「Visual Studio Build Tools」并勾选 **C++ 生成工具** 和 Windows SDK
3. 在本目录执行：

```powershell
cd desktop/src-tauri
cargo tauri build
```

产物：`src-tauri/target/release/bundle/nsis/智恋bot_1.0.0_x64-setup.exe`

## 用 CI 构建（本机不想装 Rust 时）

把 `desktop/` 推到 GitHub 仓库，Actions 里手动跑 `build-windows`，
在 Artifacts 下载 `zhilian-windows` 即可。

## 代码签名（重要）

不签名的话，用户安装时会看到 Windows SmartScreen「未知发布者，可能有害」。
两条路：
- 买 OV/EV 代码签名证书（约 ¥1000–3000/年），用 `signtool` 签 exe
- 内测阶段先不签，把安装说明写清楚（右键 → 属性 → 解除锁定）