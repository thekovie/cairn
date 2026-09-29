# Build the Cairn installer (cairn-<version>-setup.exe) with Inno Setup 6.
# Installs Inno Setup (free) with Chocolatey first if it isn't there, as on
# GitHub's Windows runners.
#
#   pwsh installer/build.ps1 -Version 0.5.0 -SourceDir dist/cairn-0.5.0-windows-x64 -OutDir dist

param(
  [Parameter(Mandatory)] [string] $Version,
  [Parameter(Mandatory)] [string] $SourceDir,
  [Parameter(Mandatory)] [string] $OutDir
)
$ErrorActionPreference = 'Stop'

$iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $iscc)) {
  choco install innosetup --no-progress -y | Out-Null
  if (-not (Test-Path $iscc)) { throw "Inno Setup couldn't be installed." }
}

$script = Join-Path $PSScriptRoot 'cairn.iss'
$source = (Resolve-Path $SourceDir).Path
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$out = (Resolve-Path $OutDir).Path
# Progress goes to the log; the script's only output is the installer's path.
& $iscc /Qp "/DAppVersion=$Version" "/DSourceDir=$source" "/O$out" $script | Out-Host
if ($LASTEXITCODE -ne 0) { throw "The installer couldn't be built (ISCC exit code $LASTEXITCODE)." }
Join-Path $out "cairn-$Version-setup.exe"
