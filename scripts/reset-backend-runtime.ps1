param(
    [string]$BackendPath = "backend",
    [switch]$StopServer
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$backendRoot = Resolve-Path (Join-Path $repoRoot $BackendPath)
$runtimeRoot = Join-Path $backendRoot "runtime"

if ($StopServer) {
    Get-Process -Name "anicargo-server" -ErrorAction SilentlyContinue | Stop-Process -Force
}

if (Test-Path $runtimeRoot) {
    Get-ChildItem -LiteralPath $runtimeRoot -Force | Remove-Item -Recurse -Force
}
else {
    New-Item -ItemType Directory -Path $runtimeRoot | Out-Null
}

$pathsToCreate = @(
    (Join-Path $runtimeRoot "logs"),
    (Join-Path $runtimeRoot "media")
)

foreach ($path in $pathsToCreate) {
    New-Item -ItemType Directory -Path $path -Force | Out-Null
}

Write-Host "Reset backend runtime at $runtimeRoot"
