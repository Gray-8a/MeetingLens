[CmdletBinding()]
param(
    [Parameter()]
    [string]$ModelsRoot = (Join-Path $env:LOCALAPPDATA "MeetingLens\docling\models\2.117.0"),

    [Parameter()]
    [switch]$VerifyOnly
)

$ErrorActionPreference = "Stop"
$repositoryRoot = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$workerRoot = Join-Path $repositoryRoot "workers\docling"
$resolvedModelsRoot = [System.IO.Path]::GetFullPath($ModelsRoot)
$systemTemp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())

if ($resolvedModelsRoot.StartsWith($repositoryRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Docling model artifacts must not be stored inside the Git repository."
}
if ($resolvedModelsRoot.StartsWith($systemTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Docling model artifacts must not use the long-lived system Temp directory."
}

$arguments = @(
    "run",
    "--project", $workerRoot,
    "--frozen",
    "meetinglens-docling-models",
    "--models-root", $resolvedModelsRoot
)
if ($VerifyOnly) {
    $arguments += "--verify-only"
}

& uv @arguments
if ($LASTEXITCODE -ne 0) {
    throw "Docling model provisioning or verification failed."
}
