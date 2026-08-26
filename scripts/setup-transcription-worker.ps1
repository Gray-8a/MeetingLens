[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$workerRoot = Join-Path (Split-Path -Parent $PSScriptRoot) "workers\transcription"

$uvVersion = (& uv --version).Trim()
if ($uvVersion -notmatch '^uv 0\.11\.21(?:\s|$)') {
    throw "MeetingLens requires uv 0.11.21 for the transcription worker lockfile."
}

& uv sync --project $workerRoot --frozen --python 3.12
if ($LASTEXITCODE -ne 0) {
    throw "The isolated transcription worker environment could not be synchronized."
}

Write-Output "The isolated transcription worker environment is synchronized."
