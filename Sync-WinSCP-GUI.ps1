# ================= LOAD CONFIG =================
$config = Get-Content "C:\Scripts\SyncConfig.json" | ConvertFrom-Json
$localRoot = $config.LocalPath
$remoteRoot = $config.RemotePath
$winscpDll = $config.WinSCPDll

# ================= LOAD UI =================
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# ================= TRAY =================
$tray = New-Object System.Windows.Forms.NotifyIcon
$tray.Icon = [System.Drawing.SystemIcons]::Application
$tray.Visible = $true
$tray.Text = "WinSCP Sync Running"

# ================= MENU =================
$menu = New-Object System.Windows.Forms.ContextMenuStrip
$exitItem = $menu.Items.Add("Exit")
$exitItem.Add_Click({
        $tray.Visible = $false
        $tray.Dispose()
        Stop-Process -Id $PID
    })
$tray.ContextMenuStrip = $menu

# ================= LOAD WinSCP =================
Add-Type -Path $winscpDll

$sessionOptions = New-Object WinSCP.SessionOptions -Property @{
    Protocol = [WinSCP.Protocol]::Ftp
    HostName = $config.Host
    UserName = $config.User
    Password = $config.Pass
}

$session = New-Object WinSCP.Session

try {
    $session.Open($sessionOptions)
    $tray.ShowBalloonTip(3000, "Connected", "FTP Connected to $($config.Host)", "Info")
}
catch {
    $tray.ShowBalloonTip(3000, "Error", "Failed to connect: $_", "Info")
    exit
}

# ================= NOTIFICATION BATCHER =================
$notifyQueue = New-Object System.Collections.Concurrent.ConcurrentQueue[string]
$lastError = @{}

function Queue-Notify($type, $msg) {
    if ($type -eq "Error") {
        $now = Get-Date
        $key = "$type|$msg"
        if ($lastError.ContainsKey($key)) {
            if (($now - $lastError[$key]).TotalSeconds -lt 30) { return }
        }
        $lastError[$key] = $now
    }
    $notifyQueue.Enqueue("$type|$msg")
}

function Flush-Notifications {
    $items = @()
    $item = $null
    while ($notifyQueue.TryDequeue([ref]$item)) { $items += $item }
    if ($items.Count -eq 0) { return }

    if ($items.Count -eq 1) {
        $parts = $items[0] -split '\|', 2
        $tray.ShowBalloonTip(3000, $parts[0], $parts[1], "Info")
        return
    }

    $uploaded = ($items | Where-Object { $_ -match '^Uploaded\|' }).Count
    $deleted = ($items | Where-Object { $_ -match '^Deleted\|' }).Count
    $renamed = ($items | Where-Object { $_ -match '^Renamed\|' }).Count
    $errors = ($items | Where-Object { $_ -match '^Error\|' }).Count

    $lines = @()
    if ($uploaded) { $lines += "$uploaded uploaded" }
    if ($deleted) { $lines += "$deleted deleted" }
    if ($renamed) { $lines += "$renamed renamed" }
    if ($errors) { $lines += "$errors failed" }

    $tray.ShowBalloonTip(4000, "Sync complete", ($lines -join " · "), "Info")
}

# ================= HELPERS =================
$lastRun = @{}

function Should-Run($path) {
    $now = Get-Date
    if ($lastRun.ContainsKey($path)) {
        if (($now - $lastRun[$path]).TotalMilliseconds -lt 800) { return $false }
    }
    $lastRun[$path] = $now
    return $true
}

function Get-RemotePath($fullPath) {
    $relative = $fullPath.Substring($localRoot.Length).TrimStart('\')
    return ($remoteRoot + "/" + ($relative -replace "\\", "/"))
}

# ================= QUEUE =================
$queue = New-Object System.Collections.Concurrent.ConcurrentQueue[string]

# ================= ACTIONS =================
function Upload-File($file) {
    try {
        if (!(Test-Path $file)) { return }
        if ($file -match "~$|\.tmp$") { return }
        if (Test-Path $file -PathType Container) { return }

        Start-Sleep -Milliseconds 400

        $remotePath = Get-RemotePath $file
        $remoteDir = ([System.IO.Path]::GetDirectoryName($remotePath)) -replace "\\", "/"

        try { $session.CreateDirectory($remoteDir) } catch {}

        $opt = New-Object WinSCP.TransferOptions
        $opt.TransferMode = [WinSCP.TransferMode]::Binary

        $session.PutFiles($file, $remotePath, $false, $opt).Check()
        Queue-Notify "Uploaded" ([System.IO.Path]::GetFileName($file))
    }
    catch {
        Queue-Notify "Error" "Upload failed: $([System.IO.Path]::GetFileName($file))"
        $queue.Enqueue($file)
    }
}

function Delete-Remote($file) {
    try {
        $session.RemoveFiles((Get-RemotePath $file)).Check()
        Queue-Notify "Deleted" ([System.IO.Path]::GetFileName($file))
    }
    catch {
        Queue-Notify "Error" "Delete failed: $([System.IO.Path]::GetFileName($file))"
    }
}

function Rename-Remote($old, $new) {
    try {
        $session.MoveFile((Get-RemotePath $old), (Get-RemotePath $new))
        Queue-Notify "Renamed" ([System.IO.Path]::GetFileName($new))
    }
    catch {
        Upload-File $new
        Delete-Remote $old
    }
}

# ================= WATCHER =================
$watcher = New-Object System.IO.FileSystemWatcher
$watcher.Path = $localRoot
$watcher.IncludeSubdirectories = $true
$watcher.EnableRaisingEvents = $true

Register-ObjectEvent $watcher Changed -Action {
    $p = $Event.SourceEventArgs.FullPath
    if (Should-Run $p) { $queue.Enqueue($p) }
}

Register-ObjectEvent $watcher Created -Action {
    $queue.Enqueue($Event.SourceEventArgs.FullPath)
}

Register-ObjectEvent $watcher Deleted -Action {
    $queue.Enqueue("DELETE:" + $Event.SourceEventArgs.FullPath)
}

Register-ObjectEvent $watcher Renamed -Action {
    $queue.Enqueue("RENAME:" + $Event.SourceEventArgs.OldFullPath + "|" + $Event.SourceEventArgs.FullPath)
}

$tray.ShowBalloonTip(3000, "Running", "Watching $localRoot", "Info")

# ================= MESSAGE LOOP =================
$lastFlush = Get-Date
$item = $null

while ($tray.Visible) {
    [System.Windows.Forms.Application]::DoEvents()

    $processed = 0
    while ($processed -lt 5 -and $queue.TryDequeue([ref]$item)) {
        if ($item.StartsWith("DELETE:")) { Delete-Remote ($item.Substring(7)) }
        elseif ($item.StartsWith("RENAME:")) {
            $parts = $item.Substring(7) -split '\|', 2
            Rename-Remote $parts[0] $parts[1]
        }
        else { Upload-File $item }
        $processed++
    }

    if (((Get-Date) - $lastFlush).TotalSeconds -ge 2) {
        Flush-Notifications
        $lastFlush = Get-Date
    }

    Start-Sleep -Milliseconds 200
}

$session.Dispose()