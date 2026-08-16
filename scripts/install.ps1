# Local Flow installer for Windows.
#
#   irm https://raw.githubusercontent.com/MRCAUSALIDAD/local-flow/main/scripts/install.ps1 | iex
#
# Downloads the latest published setup .exe and runs it. No build toolchain
# required.

$ErrorActionPreference = 'Stop'

$Repo = if ($env:LOCAL_FLOW_REPO) { $env:LOCAL_FLOW_REPO } else { 'MRCAUSALIDAD/local-flow' }

function Info($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Warn($msg) { Write-Host "warn $msg" -ForegroundColor Yellow }

$apiUrl = if ($env:LOCAL_FLOW_TAG) {
  "https://api.github.com/repos/$Repo/releases/tags/$($env:LOCAL_FLOW_TAG)"
} else {
  "https://api.github.com/repos/$Repo/releases/latest"
}

Info "Looking up the latest release of $Repo..."
try {
  $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ 'User-Agent' = 'local-flow-installer' }
} catch {
  throw "Could not reach the GitHub API. Are you online? ($_)"
}

# On ARM64 Windows, fall back to the x64 build (it runs under emulation).
$archs = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { @('arm64', 'x64') } else { @('x64') }

# Prefer the NSIS setup .exe; fall back to the .msi.
$asset = $null
foreach ($a in $archs) {
  foreach ($pattern in @("$a.*-setup\.exe$", "$a.*\.msi$")) {
    if (-not $asset) {
      $asset = $release.assets | Where-Object { $_.name -match $pattern } | Select-Object -First 1
    }
  }
  if ($asset) { break }
}
if (-not $asset) {
  throw "No Windows installer found in release $($release.tag_name) for $env:PROCESSOR_ARCHITECTURE."
}

$dest = Join-Path $env:TEMP $asset.name
Info "Downloading $($asset.name)"
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $dest -UseBasicParsing

Info "Running the installer..."
if ($dest -like '*.msi') {
  Start-Process msiexec.exe -ArgumentList "/i", "`"$dest`"" -Wait
} else {
  Start-Process $dest -Wait
}

Warn "The build is unsigned. If SmartScreen blocks it: More info -> Run anyway."

Write-Host @"

Next steps:
  1. Settings -> Voice model -> Download a model (one time, then fully offline).
  2. Allow microphone access when Windows asks.
  3. Hold Alt+Space anywhere, speak, release.
"@
