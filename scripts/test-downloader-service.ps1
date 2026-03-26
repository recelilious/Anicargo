param(
    [string]$BaseUrl = "http://127.0.0.1:4010",
    [string]$TorrentFile = "backend/runtime/media/_rqbit/session/beffa75ad2eda16e5409572ffada51131e8bf198.torrent",
    [string]$OutputDir = "services/downloader/runtime/manual-test-output"
)

$ErrorActionPreference = "Stop"

function Invoke-AnicargoApi {
    param(
        [string]$Method,
        [string]$Path,
        [object]$Body
    )

    $uri = "{0}{1}" -f $BaseUrl.TrimEnd('/'), $Path
    if ($null -eq $Body) {
        return Invoke-RestMethod -Method $Method -Uri $uri
    }

    $json = $Body | ConvertTo-Json -Depth 10
    return Invoke-RestMethod -Method $Method -Uri $uri -ContentType "application/json" -Body $json
}

Write-Host "== Health =="
$health = Invoke-AnicargoApi -Method GET -Path "/api/health"
$health | ConvertTo-Json -Depth 10

Write-Host "`n== Runtime =="
$runtime = Invoke-AnicargoApi -Method GET -Path "/api/v1/runtime"
$runtime | ConvertTo-Json -Depth 10

Write-Host "`n== Update Settings =="
$settings = Invoke-AnicargoApi -Method PATCH -Path "/api/v1/settings" -Body @{
    max_concurrent_downloads = 1
    max_concurrent_seeds = 2
    global_download_limit_mb = 0
    global_upload_limit_mb = 5
    priority_decay = 0.8
}
$settings | ConvertTo-Json -Depth 10

Write-Host "`n== Inspect Torrent =="
$inspect = Invoke-AnicargoApi -Method POST -Path "/api/v1/inspect" -Body @{
    source = @{
        kind  = "torrent_file"
        value = $TorrentFile
    }
    output_dir = $OutputDir
}
$inspect | ConvertTo-Json -Depth 10

Write-Host "`n== Create Task A =="
$taskAOutput = Join-Path $OutputDir "task-a"
$taskA = Invoke-AnicargoApi -Method POST -Path "/api/v1/tasks" -Body @{
    kind                     = "download"
    source                   = @{
        kind  = "torrent_file"
        value = $TorrentFile
    }
    output_dir               = $taskAOutput
    priority                 = 0
    start_enabled            = $true
    seed_after_download      = $true
    manual_download_limit_mb = $null
    manual_upload_limit_mb   = $null
}
$taskA | ConvertTo-Json -Depth 10
$taskAId = $taskA.data.task.id

Write-Host "`n== Create Task B =="
$taskBOutput = Join-Path $OutputDir "task-b"
$taskB = Invoke-AnicargoApi -Method POST -Path "/api/v1/tasks" -Body @{
    kind                     = "download"
    source                   = @{
        kind  = "torrent_file"
        value = $TorrentFile
    }
    output_dir               = $taskBOutput
    priority                 = 5
    start_enabled            = $true
    seed_after_download      = $true
    manual_download_limit_mb = $null
    manual_upload_limit_mb   = $null
}
$taskB | ConvertTo-Json -Depth 10
$taskBId = $taskB.data.task.id

Write-Host "`n== Verify Duplicate Prevention =="
if ($taskB.data.created -or $taskBId -ne $taskAId) {
    throw "Duplicate prevention failed: second create should reuse task A"
}
"Duplicate prevention OK"

Start-Sleep -Seconds 3

Write-Host "`n== Downloads List =="
$downloads = Invoke-AnicargoApi -Method GET -Path "/api/v1/downloads"
$downloads | ConvertTo-Json -Depth 10

Write-Host "`n== Pause Task A =="
$pausedA = Invoke-AnicargoApi -Method POST -Path "/api/v1/tasks/${taskAId}/pause"
$pausedA | ConvertTo-Json -Depth 10

Write-Host "`n== Resume Task A =="
$resumedA = Invoke-AnicargoApi -Method POST -Path "/api/v1/tasks/${taskAId}/resume"
$resumedA | ConvertTo-Json -Depth 10

Write-Host "`n== Delete Task A =="
$deletedA = Invoke-AnicargoApi -Method DELETE -Path "/api/v1/tasks/${taskAId}?delete_files=true"
$deletedA | ConvertTo-Json -Depth 10

Write-Host "`n== Delete Task B =="
if ($taskBId -ne $taskAId) {
    $deletedB = Invoke-AnicargoApi -Method DELETE -Path "/api/v1/tasks/${taskBId}?delete_files=true"
    $deletedB | ConvertTo-Json -Depth 10
} else {
    "Task B reused task A; no extra delete needed."
}

Write-Host "`nDownloader API smoke test completed."
