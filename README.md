# 📋 ClipStore

> A blazing fast, lightweight cross-platform clipboard manager for **Windows & Linux** with system tray integration, global shortcuts (`Win + V`), and automated GitHub Actions multi-format packaging (`.exe`, `.deb`, `.rpm`, `.AppImage`).

[![Build & Release ClipStore](https://github.com/example/clipstore/actions/workflows/build-and-release.yml/badge.svg)](https://github.com/example/clipstore/actions/workflows/build-and-release.yml)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-blue.svg)](https://github.com/example/clipstore)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.90+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-24C8D8.svg)](https://tauri.app/)

---

## ✨ Features

- 🪶 **Ultra Lightweight**: Consumes only ~30MB RAM and produces tiny binaries (~5MB) compared to 200MB+ Electron apps.
- 🪟 **Windows Default Look & Feel**: Styled after the Windows 11 Fluent mica/glass design with rounded corners, drop shadows, and auto-dismiss on blur.
- ⌨️ **Global Shortcut (`Win + V`)**: Summon the clipboard history instantly anywhere with `Win + V` (or universal fallback `Ctrl + Shift + V` / `Alt + V`).
- 📌 **Pin & Star Snippets**: Keep essential code snippets, links, templates, and emojis permanently stored at the top.
- 🔍 **Instant Search & Type Filtering**: Real-time filtering across your clipboard history. Filter by **All**, **Pinned**, **Code**, **Links**, or **Text**.
- 📥 **Auto-Paste**: Click any item or press <kbd>Enter</kbd> to copy it back to your clipboard and seamlessly paste it into your active window.
- 🔔 **System Tray Integration**: Runs silently in the background with a system tray icon, tray context menu, and pause/resume capabilities.
- 💾 **Safe Local Persistence**: Clipboard history is saved locally to your device (`%APPDATA%/clipstore/` on Windows, `~/.config/clipstore/` on Linux). Zero tracking, zero telemetry.
- 📦 **Cross-Platform Installers**:
  - **Windows**: Setup installer (`.exe`), Standalone portable executable (`clipstore-portable.exe`), and Enterprise MSI (`.msi`).
  - **Linux**: Debian/Ubuntu (`.deb`), Fedora/RHEL/openSUSE (`.rpm`), and Universal standalone (`.AppImage`).

---

## 🚀 Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Win + V` or `Ctrl + Shift + V` | **Toggle Clipboard Window** |
| `↑` / `↓` Arrow Keys | **Navigate history items** |
| `Enter` | **Copy & Paste selected item** |
| `P` | **Pin / Unpin item** |
| `Del` or `Backspace` | **Delete selected item** |
| `Ctrl + F` or `/` | **Focus search bar** |
| `Esc` | **Close / Hide window** |

---

## 📦 Downloads & Installation

### Windows

1. **Installer (`.exe`)**:
   Download `clipstore_<version>_x64-setup.exe` from the [Releases](https://github.com/example/clipstore/releases) tab. Run the installer for desktop and Start Menu shortcuts.
2. **Standalone Portable (`.exe`)**:
   Download `clipstore-portable.exe`. No installation required! Just double click and run from any folder or USB drive.
3. **MSI (`.msi`)**:
   Download `clipstore_<version>_x64_en-US.msi` for enterprise deployment via Group Policy or Intune.

### Linux

1. **Ubuntu / Debian / Linux Mint / Pop!_OS (`.deb`)**:
   ```bash
   sudo dpkg -i clipstore_<version>_amd64.deb
   sudo apt-get install -f # install missing dependencies if any
   ```
2. **Fedora / RHEL / CentOS / openSUSE (`.rpm`)**:
   ```bash
   sudo rpm -i clipstore-<version>-1.x86_64.rpm
   # or with dnf:
   sudo dnf install ./clipstore-<version>-1.x86_64.rpm
   ```
3. **Universal AppImage**:
   ```bash
   chmod +x clipstore_<version>_amd64.AppImage
   ./clipstore_<version>_amd64.AppImage
   ```

---

## ⚙️ Automated GitHub Actions CI/CD

ClipStore includes an automated GitHub Actions matrix workflow (`.github/workflows/build-and-release.yml`):

- **Triggers**: On every push to `main`, pull request, new Git tag (`v*`), or manual trigger (`workflow_dispatch`).
- **Build Matrix**:
  - `windows-latest`: Compiles NSIS installer `.exe`, standalone portable `clipstore-portable.exe`, and `.msi`.
  - `ubuntu-22.04`: Sets up GTK/WebKit dependencies and compiles `.deb`, `.rpm`, and `.AppImage`.
- **Automated Releases**: When a version tag (e.g. `v1.0.0`) is pushed, GitHub Actions automatically creates a release with all installer files attached.

---

## 🛠️ Local Development & Building

### Prerequisites

- **Node.js**: v18+ (Node 20+ recommended)
- **Rust & Cargo**: stable toolchain (`rustup update stable`)

### 1. Clone & Install Dependencies

```bash
git clone https://github.com/example/clipstore.git
cd clipstore
npm install
```

### 2. Run in Development Mode

```bash
npm run tauri dev
```

### 3. Build Production Executable & Installers

```bash
# Build frontend assets
npm run build

# Build standalone portable Windows executable
cd src-tauri
cargo build --release

# Build full installer package (.exe / .deb / .rpm)
npm run tauri build
```

The compiled standalone executable will be located at:
`src-tauri/target/release/clipstore.exe`

The bundled installers will be located in:
`src-tauri/target/release/bundle/`

---

## 📁 Project Structure

```
clipstore/
├── .github/
│   └── workflows/
│       └── build-and-release.yml    # GitHub Actions CI/CD matrix build & release
├── src/
│   ├── main.js                      # Reactive frontend logic & keyboard handling
│   └── styles.css                   # Windows 11 Fluent mica dark theme styling
├── src-tauri/
│   ├── capabilities/
│   │   └── default.json             # Tauri security capabilities & permissions
│   ├── icons/                       # High-res app & system tray icons
│   ├── src/
│   │   ├── lib.rs                   # Rust backend: tray, clipboard poller, hotkey, store
│   │   └── main.rs                  # Application entrypoint
│   ├── Cargo.toml                   # Rust dependencies (Tauri, arboard, windows-sys)
│   └── tauri.conf.json              # Window geometry, bundling & packaging config
├── index.html                       # Modern floating clipboard popup interface
├── package.json                     # Node scripts & dependencies
└── vite.config.js                   # Vite build configuration
```

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
