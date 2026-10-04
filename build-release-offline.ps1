# build-release-offline.ps1
# Builds a FAT installer (~140MB) with WebView2 bundled inside.
# Safe for installation on computers with NO internet connection.
# The normal build-release.ps1 is unchanged and still produces a small installer.

$ErrorActionPreference = "Stop"
$configPath = "src-tauri\tauri.conf.json"
$outDir = "releases\offline"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  NeverSleep -- OFFLINE FAT BUILD" -ForegroundColor Cyan
Write-Host "  Bundles WebView2 runtime (~140MB)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 1. Sync source to dist
Write-Host "Syncing source files to dist/..." -ForegroundColor Cyan
$filesToSync = @("index.html","settings.html","lang.js","overlay.html","sleep.html")
foreach ($file in $filesToSync) {
    if (Test-Path $file) {
        Copy-Item $file "dist\$file" -Force
        Write-Host "  Copied $file -> dist\$file" -ForegroundColor DarkGray
    }
}
if (Test-Path "fonts") {
    if (-not (Test-Path "dist\fonts")) { New-Item -ItemType Directory -Path "dist\fonts" -Force | Out-Null }
    Copy-Item "fonts\*" "dist\fonts\" -Recurse -Force
    Remove-Item "dist\fonts\fonts" -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "  Copied fonts/* -> dist/fonts/" -ForegroundColor DarkGray
}
Write-Host "Sync complete." -ForegroundColor Green

# 2. Patch tauri.conf.json — insert webviewInstallMode after "timestampUrl" line
# Uses string replacement + WriteAllText (no BOM) to avoid Tauri JSON parse errors
Write-Host ""
Write-Host "Patching tauri.conf.json for offlineInstaller mode..." -ForegroundColor Cyan
$utf8NoBom = New-Object System.Text.UTF8Encoding $false
$absConfig  = (Resolve-Path $configPath).Path
$originalConfig = [System.IO.File]::ReadAllText($absConfig, $utf8NoBom)

$insertLine     = '        "webviewInstallMode": { "type": "offlineInstaller" },'
$pattern        = '("timestampUrl"\s*:\s*"[^"]*")'
$replacement    = "`$1,`r`n$insertLine"
# Remove existing trailing comma from timestampUrl match group then re-add with new line
$patchedConfig  = [regex]::Replace($originalConfig, '"timestampUrl"\s*:\s*"[^"]*",', {
    param($m)
    $m.Value + "`r`n" + $insertLine
})

[System.IO.File]::WriteAllText($absConfig, $patchedConfig, $utf8NoBom)
Write-Host "  offlineInstaller mode enabled." -ForegroundColor Green

# 3. Clear Tauri cache
Write-Host ""
Write-Host "Clearing stale Tauri codegen cache..." -ForegroundColor Cyan
Get-ChildItem "src-tauri\target\release\build\" -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "app-*" } |
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
Get-ChildItem "src-tauri\target\release\.fingerprint\" -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "app-*" } |
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
(Get-Item "src-tauri\build.rs").LastWriteTime = Get-Date
Write-Host "Cache cleared." -ForegroundColor Green

# 4. Build
Write-Host ""
Write-Host "Building offline fat installer..." -ForegroundColor Cyan
npm run tauri build
$buildOk = $LASTEXITCODE -eq 0

# 5. Restore original config NO MATTER WHAT (also no BOM)
Write-Host ""
Write-Host "Restoring tauri.conf.json..." -ForegroundColor Cyan
[System.IO.File]::WriteAllText($absConfig, $originalConfig, $utf8NoBom)
Write-Host "  Config restored to normal (small) build mode." -ForegroundColor Green

# 6. Save installer — only the one matching the current version
if ($buildOk) {
    Write-Host ""
    Write-Host "Build succeeded! Saving to $outDir..." -ForegroundColor Green
    if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir -Force | Out-Null }

    # Read version from package.json so we copy only the freshly built installer
    $version = (Get-Content "package.json" -Raw | ConvertFrom-Json).version
    $nsisDir  = "src-tauri\target\release\bundle\nsis"
    $match    = Get-ChildItem "$nsisDir\*_${version}_*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1

    if ($match) {
        $dest = "$outDir\$($match.BaseName)-offline$($match.Extension)"
        Copy-Item $match.FullName $dest -Force
        Write-Host "  Saved: $dest" -ForegroundColor Green
        Write-Host ""
        Write-Host "OFFLINE BUILD COMPLETE -> $dest" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: Could not find installer for version $version in $nsisDir\" -ForegroundColor Yellow
        Write-Host "  Files found:" -ForegroundColor Yellow
        Get-ChildItem "$nsisDir\*.exe" | ForEach-Object { Write-Host "    $($_.Name)" -ForegroundColor DarkGray }
    }
} else {
    Write-Host "Build FAILED. tauri.conf.json has been restored." -ForegroundColor Red
}
