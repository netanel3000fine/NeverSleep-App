$source = "\\192.168.1.1\volume(sda1)\files\never-sleep-tauri"
$dest   = "C:\Users\ntnlb\Desktop\never-sleep-tauri"

Write-Host "Starting full mirror sync..." -ForegroundColor Cyan

robocopy $source $dest /MIR /Z /W:3 /R:3 /NP /TEE /LOG:"C:\Scripts\mirror-sync.log"

if ($LASTEXITCODE -le 7) {
    Write-Host "Sync complete." -ForegroundColor Green
} else {
    Write-Host "Sync finished with errors. Check C:\Scripts\mirror-sync.log" -ForegroundColor Red
}