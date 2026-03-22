param(
    [string]$BackendPath = "backend",
    [switch]$StopServer
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$backendRoot = Resolve-Path (Join-Path $repoRoot $BackendPath)
$runtimeRoot = Join-Path $backendRoot "runtime"
$repoRuntimeRoot = Join-Path $repoRoot "runtime"
$frontendDistRoot = Join-Path $repoRoot "frontend\\web\\dist"

function Clear-PathContent {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    if (Test-Path $Path) {
        Get-ChildItem -LiteralPath $Path -Force | Remove-Item -Recurse -Force
    }
    else {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
    }
}

if ($StopServer) {
    Get-Process -Name "anicargo-server" -ErrorAction SilentlyContinue | Stop-Process -Force
}

foreach ($path in @($runtimeRoot, $repoRuntimeRoot) | Select-Object -Unique) {
    Clear-PathContent -Path $path
}

foreach ($root in @($backendRoot, $repoRoot) | Select-Object -Unique) {
    Get-ChildItem -LiteralPath $root -Filter "anicargo.db*" -ErrorAction SilentlyContinue |
        Remove-Item -Force -ErrorAction SilentlyContinue
}

if (Test-Path $frontendDistRoot) {
    Remove-Item -LiteralPath $frontendDistRoot -Recurse -Force
}

$pathsToCreate = @(
    (Join-Path $runtimeRoot "logs"),
    (Join-Path $runtimeRoot "media")
)

foreach ($path in $pathsToCreate) {
    New-Item -ItemType Directory -Path $path -Force | Out-Null
}

Write-Host "Reset Anicargo runtime state:"
Write-Host "  - database files"
Write-Host "  - download payloads"
Write-Host "  - logs, rqbit session caches, and temporary runtime caches"
Write-Host "  - frontend build output"
Write-Host "  Backend runtime root: $runtimeRoot"
