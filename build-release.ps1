# build-release.ps1 — Use this instead of "npm run tauri build"
# Clears stale Tauri codegen cache, rebuilds, then deploys directly (no installer needed).

Write-Host "Clearing stale Tauri codegen cache..." -ForegroundColor Cyan
Get-ChildItem "src-tauri\target\release\build\" -Directory -ErrorAction SilentlyContinue |
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
    Start-Sleep -Milliseconds 800

    # Clear WebView2 cache
    Remove-Item -Recurse -Force "$env:LOCALAPPDATA\com.neversleep.app\EBWebView\Default\Cache" -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force "$env:LOCALAPPDATA\com.neversleep.app\EBWebView\Default\Code Cache" -ErrorAction SilentlyContinue

    # Copy fresh exe directly (no installer)
    Copy-Item "src-tauri\target\release\Never Sleep.exe" "$env:LOCALAPPDATA\Never Sleep\Never Sleep.exe" -Force
    Copy-Item "src-tauri\target\release\bundle\nsis\Never Sleep_17.0.0_x64-setup.exe" "$env:USERPROFILE\Desktop\Never Sleep_17.0.0_x64-setup.exe" -Force

    # Launch
    Start-Process "$env:LOCALAPPDATA\Never Sleep\Never Sleep.exe"
    Write-Host "App launched with fresh build!" -ForegroundColor Green
} else {
    Write-Host "Build FAILED." -ForegroundColor Red
}
