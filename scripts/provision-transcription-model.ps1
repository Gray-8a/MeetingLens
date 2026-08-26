[CmdletBinding()]
param(
    [Parameter()]
    [string]$ModelsRoot = (Join-Path $env:LOCALAPPDATA "MeetingLens\transcription\models\small-536b0662742c02347bc0e980a01041f333bce120"),

    [Parameter()]
    [switch]$VerifyOnly
)

$ErrorActionPreference = "Stop"
$repositoryRoot = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$workerRoot = Join-Path $repositoryRoot "workers\transcription"
$resolvedModelsRoot = [System.IO.Path]::GetFullPath($ModelsRoot)
$systemTemp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$repositoryPrefix = $repositoryRoot.TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
$systemTempPrefix = $systemTemp.TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar

if ($resolvedModelsRoot.Equals($repositoryRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
    $resolvedModelsRoot.StartsWith($repositoryPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Transcription model artifacts must not be stored inside the Git repository."
}
if ($resolvedModelsRoot.Equals($systemTemp, [System.StringComparison]::OrdinalIgnoreCase) -or
    $resolvedModelsRoot.StartsWith($systemTempPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Transcription model artifacts must not use the long-lived system Temp directory."
}

$uvVersion = (& uv --version).Trim()
if ($uvVersion -notmatch '^uv 0\.11\.21(?:\s|$)') {
    throw "MeetingLens requires uv 0.11.21 for transcription model provisioning."
}

$arguments = @(
    "run",
    "--project", $workerRoot,
    "--frozen",
    "python", "-I", "-m", "meetinglens_transcription_worker.models",
    "--models-root", $resolvedModelsRoot
)
if ($VerifyOnly) {
    $arguments += "--verify-only"
}

& uv @arguments
if ($LASTEXITCODE -ne 0) {
    throw "Transcription model provisioning or verification failed."
}
