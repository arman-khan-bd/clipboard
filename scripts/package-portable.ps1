# Create portable package zip
$ErrorActionPreference = "Stop"

$portableDir = "dist-portable"
$zipPath = "dist-portable\clipstore-portable-windows-x64.zip"

if (-not (Test-Path "$portableDir\clipstore-portable.exe")) {
    Write-Host "Copying binary to dist-portable..."
    New-Item -ItemType Directory -Force $portableDir | Out-Null
    Copy-Item "src-tauri\target\release\clipstore.exe" "$portableDir\clipstore-portable.exe"
}

# Create quick start readme inside portable bundle
@"
ClipStore - Lightweight Cross-Platform Clipboard Manager (Portable)
===================================================================

How to run:
1. Double click 'clipstore-portable.exe'.
2. The application will start silently and dock in your Windows System Tray (bottom right notification area).
3. Press 'Win + V' (or 'Ctrl + Shift + V') anytime to open the clipboard history popup.
4. Left-click or right-click the system tray icon to view menu options or exit.

Your clipboard history is automatically saved to:
%APPDATA%\clipstore\history.json

No installation or administrative privileges required!
"@ | Out-File -Encoding utf8 "$portableDir\README.txt"

Copy-Item "LICENSE" "$portableDir\LICENSE"

if (Test-Path $zipPath) {
    Remove-Item $zipPath
}

Compress-Archive -Path "$portableDir\clipstore-portable.exe", "$portableDir\README.txt", "$portableDir\LICENSE" -DestinationPath $zipPath
Write-Host "Created portable archive: $zipPath"
