# build-release.ps1 — Use this instead of "npm run tauri build"
# Syncs source files from root → dist/, then builds and deploys.
# Edit root files only — never touch dist/ manually.

Write-Host "Syncing source files to dist/..." -ForegroundColor Cyan

# Files to copy from root → dist/
$filesToSync = @(
    "index.html",
    "settings.html",
    "lang.js",
    "overlay.html",
    "sleep.html",
    "tray.html"
)

foreach ($file in $filesToSync) {
    if (Test-Path $file) {
        Copy-Item $file "dist\$file" -Force
        Write-Host "  Copied $file -> dist\$file" -ForegroundColor DarkGray
    } else {
        Write-Host "  WARNING: $file not found at root, skipping." -ForegroundColor Yellow
    }
}

# Sync fonts/ folder
if (Test-Path "fonts") {
    if (-not (Test-Path "dist\fonts")) { New-Item -ItemType Directory -Path "dist\fonts" -Force | Out-Null }
    Copy-Item "fonts\*" "dist\fonts\" -Recurse -Force
    Remove-Item "dist\fonts\fonts" -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "  Copied fonts/* -> dist/fonts/" -ForegroundColor DarkGray
}

Write-Host "Sync complete." -ForegroundColor Green

Write-Host "Clearing stale Tauri codegen cache..." -ForegroundColor Cyan
Get-ChildItem "src-tauri\target\release\build\" -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "app-*" } |
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

# Also clear fingerprints to avoid stale resource.lib linker errors
Get-ChildItem "src-tauri\target\release\.fingerprint\" -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "app-*" } |
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

# Touch build.rs so Cargo re-runs the build script and re-embeds assets
(Get-Item "src-tauri\build.rs").LastWriteTime = Get-Date
Write-Host "Cache cleared. Building..." -ForegroundColor Green

npm run tauri build

if ($LASTEXITCODE -eq 0) {
    Write-Host "Build OK. Deploying..." -ForegroundColor Green

    # Kill running app
    taskkill /F /IM "Never Sleep.exe" 2>$null
    Start-Sleep -Milliseconds 2500

    # Clear WebView2 cache
    Remove-Item -Recurse -Force "$env:LOCALAPPDATA\com.neversleep.app\EBWebView\Default\Cache" -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force "$env:LOCALAPPDATA\com.neversleep.app\EBWebView\Default\Code Cache" -ErrorAction SilentlyContinue

    # Copy fresh exe directly (no installer)
    Copy-Item "src-tauri\target\release\Never Sleep.exe" "$env:LOCALAPPDATA\Never Sleep\Never Sleep.exe" -Force
    Get-ChildItem "src-tauri\target\release\bundle\nsis\*.exe" | Select-Object -First 1 | ForEach-Object {
        Copy-Item $_.FullName "$env:USERPROFILE\Desktop\$($_.Name)" -Force
    }

    # Launch
    Start-Process "$env:LOCALAPPDATA\Never Sleep\Never Sleep.exe"
    Write-Host "App launched with fresh build!" -ForegroundColor Green
} else {
    Write-Host "Build FAILED." -ForegroundColor Red
}
