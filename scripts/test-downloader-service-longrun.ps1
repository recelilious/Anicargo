param(
    [string]$BaseUrl = "http://127.0.0.1:4010",
    [int]$SubjectId = 517057,
    [int]$TargetLastEpisode = 13,
    [int]$GlobalDownloadLimitMb = 5,
    [int]$MaxConcurrentDownloads = 3,
    [int]$MaxConcurrentSeeds = 2,
    [int]$CheckIntervalSeconds = 10,
    [int]$MaxRuntimeMinutes = 20,
    [int]$MaxChecks = 0,
    [string]$OutputRoot = "services/downloader/runtime/longrun-test",
    [switch]$Cleanup
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function Write-Marker {
    param(
        [string]$Level,
        [string]$Message
    )

    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host ("[{0}] [{1}] {2}" -f $timestamp, $Level, $Message)
}

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

    $json = $Body | ConvertTo-Json -Depth 16
    return Invoke-RestMethod -Method $Method -Uri $uri -ContentType "application/json" -Body $json
}

function Invoke-RestJsonWithRetry {
    param(
        [string]$Uri,
        [int]$MaxAttempts = 5,
        [int]$InitialDelaySeconds = 3
    )

    $delaySeconds = $InitialDelaySeconds
    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        try {
            return Invoke-RestMethod -Method GET -Uri $Uri
        } catch {
            $message = $_.Exception.Message
            $isRateLimited = $message -match '1015|429|Too Many Requests'
            if (-not $isRateLimited -or $attempt -ge $MaxAttempts) {
                throw
            }

            Write-Marker "WARN" ("AnimeGarden rate limited attempt {0}/{1}, retrying after {2}s" -f $attempt, $MaxAttempts, $delaySeconds)
            Start-Sleep -Seconds $delaySeconds
            $delaySeconds = [Math]::Min($delaySeconds * 2, 30)
        }
    }
}

function Get-AnimeGardenResourceCatalog {
    $query = "https://api.animes.garden/resources?subject=$SubjectId&pageSize=100&metadata=true&keyword=LoliHouse&keyword=S3"
    $response = Invoke-RestJsonWithRetry -Uri $query

    $items = @($response.resources) | Where-Object {
        $_.metadata.anipar.season.number -eq 3 -and
        $_.metadata.anipar.fansub.name -eq "LoliHouse" -and
        $null -ne $_.metadata.anipar.episode.number
    }

    return $items | Sort-Object {
        $_.metadata.anipar.episode.number
    }, @{
        Expression = { $_.updatedAt }
        Descending = $true
    }
}

function Get-DmhyTorrentUrl {
    param([string]$PageUrl)

    $html = curl.exe -L -s $PageUrl
    $match = [regex]::Match($html, 'https?://dl\.dmhy\.org/[^"''\s]+\.torrent')
    if ($match.Success) {
        return $match.Value
    }

    $match = [regex]::Match($html, '//dl\.dmhy\.org/[^"''\s]+\.torrent')
    if ($match.Success) {
        return 'https:' + $match.Value
    }

    throw "Failed to locate torrent download link for $PageUrl"
}

function Get-PriorityForEpisode {
    param([int]$Episode)

    if ($Episode -le 3) { return 0 }
    if ($Episode -le 6) { return 1 }
    if ($Episode -le 9) { return 2 }
    return 3
}

function Add-HardFailure {
    param([string]$Message)
    $script:HardFailures.Add($Message) | Out-Null
    Write-Marker "FAIL" $Message
}

function Add-WarningMessage {
    param([string]$Message)
    $script:Warnings.Add($Message) | Out-Null
    Write-Marker "WARN" $Message
}

function Add-PassMessage {
    param([string]$Message)
    Write-Marker "PASS" $Message
}

$script:HardFailures = New-Object System.Collections.Generic.List[string]
$script:Warnings = New-Object System.Collections.Generic.List[string]

New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
$torrentCacheDir = Join-Path $OutputRoot "torrents"
New-Item -ItemType Directory -Path $torrentCacheDir -Force | Out-Null
$defaultOutputDir = Join-Path $OutputRoot "default-downloads"
New-Item -ItemType Directory -Path $defaultOutputDir -Force | Out-Null

Write-Marker "INFO" "Updating downloader runtime settings"
$settings = Invoke-AnicargoApi -Method PATCH -Path "/api/v1/settings" -Body @{
    default_output_dir = $defaultOutputDir
    max_concurrent_downloads = $MaxConcurrentDownloads
    max_concurrent_seeds = $MaxConcurrentSeeds
    global_download_limit_mb = $GlobalDownloadLimitMb
    global_upload_limit_mb = 5
    priority_decay = 0.8
}
$settings | Out-Null
Add-PassMessage "Runtime settings updated"

$resourceCatalog = @(Get-AnimeGardenResourceCatalog)
$taskPlans = @()
for ($episode = 1; $episode -le $TargetLastEpisode; $episode++) {
    $resource = @($resourceCatalog | Where-Object {
        $_.metadata.anipar.episode.number -eq $episode
    }) | Select-Object -First 1
    if ($null -eq $resource) {
        Add-WarningMessage "Episode $episode resource not found on AnimeGarden for LoliHouse S3; skipping"
        continue
    }

    $useTorrentFile = ($episode % 2 -eq 0)
    if ($useTorrentFile) {
        $torrentUrl = Get-DmhyTorrentUrl -PageUrl $resource.href
        $torrentPath = Join-Path $torrentCacheDir ("oshi-no-ko-s3-{0:D2}.torrent" -f $episode)
        curl.exe -L -s -o $torrentPath $torrentUrl | Out-Null
        $source = @{
            kind  = "torrent_file"
            value = $torrentPath
        }
    } else {
        $source = @{
            kind  = "url"
            value = $resource.magnet
        }
    }

    $outputDir = $null
    if ($episode % 3 -ne 0) {
        $outputDir = Join-Path $OutputRoot ("episode-{0:D2}" -f $episode)
        New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    }

    $taskPlans += [pscustomobject]@{
        Episode    = $episode
        Priority   = Get-PriorityForEpisode -Episode $episode
        Source     = $source
        OutputDir  = $outputDir
        Title      = $resource.title
        Magnet     = $resource.magnet
    }
}

if ($taskPlans.Count -eq 0) {
    throw "No AnimeGarden resources were prepared for the longrun test"
}

$createdTasks = @{}
foreach ($plan in $taskPlans) {
    $body = @{
        kind = "download"
        source = $plan.Source
        priority = $plan.Priority
        start_enabled = $true
        seed_after_download = $true
    }
    if ($null -ne $plan.OutputDir) {
        $body.output_dir = $plan.OutputDir
    }

    $created = Invoke-AnicargoApi -Method POST -Path "/api/v1/tasks" -Body $body
    $task = $created.data.task
    $createdTasks[$plan.Episode] = [pscustomobject]@{
        Episode = $plan.Episode
        TaskId = $task.id
        Created = [bool]$created.data.created
        Priority = $task.priority
        Title = $plan.Title
    }
    Add-PassMessage ("Created task for episode {0:D2}: created={1}, task_id={2}" -f $plan.Episode, $created.data.created, $task.id)
}

if ($createdTasks.ContainsKey(1)) {
    Write-Marker "INFO" "Verifying duplicate prevention with episode 01"
    $firstPlan = $taskPlans | Where-Object Episode -eq 1 | Select-Object -First 1
    $duplicateBody = @{
        kind = "download"
        source = $firstPlan.Source
        priority = $firstPlan.Priority
        start_enabled = $true
        seed_after_download = $true
    }
    if ($null -ne $firstPlan.OutputDir) {
        $duplicateBody.output_dir = $firstPlan.OutputDir
    }
    $duplicateResponse = Invoke-AnicargoApi -Method POST -Path "/api/v1/tasks" -Body $duplicateBody
    if ($duplicateResponse.data.created) {
        Add-HardFailure "Duplicate task creation unexpectedly created a new task for episode 01"
    } elseif ($duplicateResponse.data.task.id -ne $createdTasks[1].TaskId) {
        Add-HardFailure "Duplicate task returned a different task id for episode 01"
    } else {
        Add-PassMessage "Duplicate prevention returned the existing task for episode 01"
    }
}

$startTime = Get-Date
$checkCount = 0
$seenActivity = $false
$peerlessSince = @{}

while ($true) {
    $checkCount++
    $downloads = Invoke-AnicargoApi -Method GET -Path "/api/v1/downloads"
    $runtime = Invoke-AnicargoApi -Method GET -Path "/api/v1/runtime"
    $items = @($downloads.data.items)
    $tracked = @($items | Where-Object { $createdTasks.Values.TaskId -contains $_.id })
    $active = @($tracked | Where-Object { $_.state -in @("starting", "downloading") })
    $queueOrdered = @($tracked |
        Where-Object {
            $_.state -in @("queued", "starting", "downloading") -and
            $null -ne $_.queue_position
        } |
        Sort-Object queue_position)

    if ($active.Count -gt $MaxConcurrentDownloads) {
        Add-HardFailure "Active download count $($active.Count) exceeded max concurrent download count $MaxConcurrentDownloads"
    } else {
        Add-PassMessage "Active downloads within limit: $($active.Count)/$MaxConcurrentDownloads"
    }

    $totalRate = [int64]$runtime.data.total_download_rate_bytes
    $limitBytes = [int64]$GlobalDownloadLimitMb * 1024 * 1024
    $allowedBytes = [int64]([math]::Round($limitBytes * 1.15)) + 262144
    if ($limitBytes -gt 0 -and $totalRate -gt $allowedBytes) {
        Add-HardFailure "Total download rate $totalRate B/s exceeded configured limit window $allowedBytes B/s"
    } else {
        Add-PassMessage "Total download rate within expected limit: $totalRate B/s"
    }

    $orderValid = $true
    for ($i = 1; $i -lt $queueOrdered.Count; $i++) {
        $prev = $queueOrdered[$i - 1]
        $curr = $queueOrdered[$i]
        if ($prev.priority -gt $curr.priority) {
            $orderValid = $false
            break
        }
        if ($prev.priority -eq $curr.priority -and [datetime]$prev.created_at -gt [datetime]$curr.created_at) {
            $orderValid = $false
            break
        }
    }
    if (-not $orderValid) {
        Add-HardFailure "Queue ordering no longer matches priority then creation time"
    } else {
        Add-PassMessage "Queue ordering matches priority then creation time"
    }

    if ($active.Count -gt 0) {
        $seenActivity = $true
    }
    if (-not $seenActivity -and ((Get-Date) - $startTime).TotalSeconds -ge 60) {
        Add-HardFailure "No task entered starting/downloading state within 60 seconds"
    }

    foreach ($item in $active) {
        if ($item.peer_count -gt 0 -or $item.downloaded_bytes -gt 0) {
            $peerlessSince.Remove($item.id) | Out-Null
        } else {
            if (-not $peerlessSince.ContainsKey($item.id)) {
                $peerlessSince[$item.id] = Get-Date
            } elseif (((Get-Date) - $peerlessSince[$item.id]).TotalSeconds -ge 90) {
                Add-WarningMessage ("Task {0} has stayed active with 0 peers and 0 downloaded bytes for over 90 seconds" -f $item.id)
                $peerlessSince.Remove($item.id) | Out-Null
            }
        }
    }

    $trackedCount = $tracked.Count
    if ($trackedCount -ne $createdTasks.Count) {
        Add-HardFailure "Tracked task count changed unexpectedly: expected $($createdTasks.Count), got $trackedCount"
    }

    $queuedCount = @($tracked | Where-Object { $_.state -eq 'queued' }).Count
    Write-Marker "INFO" ("Check #{0}: tracked={1}, active={2}, queued={3}, total_rate={4} B/s" -f $checkCount, $tracked.Count, $active.Count, $queuedCount, $totalRate)

    $elapsedMinutes = ((Get-Date) - $startTime).TotalMinutes
    $allTerminal = ($tracked.Count -gt 0) -and (@($tracked | Where-Object { $_.state -notin @('completed', 'failed', 'paused', 'deleted', 'seeding') }).Count -eq 0)
    if ($allTerminal) {
        Add-PassMessage "All tracked tasks reached terminal or paused/seeding states"
        break
    }
    if ($MaxChecks -gt 0 -and $checkCount -ge $MaxChecks) {
        break
    }
    if ($elapsedMinutes -ge $MaxRuntimeMinutes) {
        Add-WarningMessage "Reached max runtime window before all tasks finished"
        break
    }

    Start-Sleep -Seconds $CheckIntervalSeconds
}

if ($Cleanup) {
    Write-Marker "INFO" "Cleaning up created tasks"
    foreach ($taskInfo in $createdTasks.Values) {
        try {
            Invoke-AnicargoApi -Method DELETE -Path "/api/v1/tasks/$($taskInfo.TaskId)?delete_files=true" | Out-Null
        } catch {
            Add-WarningMessage ("Failed to delete task {0}: {1}" -f $taskInfo.TaskId, $_.Exception.Message)
        }
    }
}

Write-Host ""
Write-Marker "INFO" ("Hard failures: {0}" -f $HardFailures.Count)
Write-Marker "INFO" ("Warnings: {0}" -f $Warnings.Count)

if ($HardFailures.Count -eq 0) {
    Write-Marker "PASS" "Longrun downloader test passed"
    exit 0
}

Write-Marker "FAIL" "Longrun downloader test failed"
exit 1
