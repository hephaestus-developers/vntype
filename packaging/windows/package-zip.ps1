[CmdletBinding()]
param(
    [ValidateSet("x86_64", "aarch64")]
    [string]$Arch = "",
    [string]$Target = "",
    [string]$OutDir = "",
    [switch]$Offline
)

$ErrorActionPreference = "Stop"

$RootDir = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if ([string]::IsNullOrEmpty($OutDir)) {
    $OutDir = Join-Path $RootDir "dist\windows"
}

if ([string]::IsNullOrEmpty($Arch)) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
        $Arch = "aarch64"
    } else {
        $Arch = "x86_64"
    }
}

$CargoToml = Get-Content (Join-Path $RootDir "Cargo.toml") -Raw
if ($CargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    $Version = "0.1.0"
}

$CargoArgs = @("build", "--release", "-p", "vntype-cli", "-p", "vntype-settings", "-p", "vntype-hook", "-p", "vntype-tsf")
if (-not [string]::IsNullOrEmpty($Target)) {
    $CargoArgs += @("--target", $Target)
    $BinDir = Join-Path $RootDir "target\$Target\release"
} else {
    $BinDir = Join-Path $RootDir "target\release"
}

if ($Offline -or $env:VNTYPE_OFFLINE -eq "1") {
    $CargoArgs += "--offline"
}

Write-Host "Building VNType $Version ($Arch) for Windows..."
& cargo @CargoArgs --manifest-path (Join-Path $RootDir "Cargo.toml")
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed with exit code $LASTEXITCODE"
}

$StageDir = Join-Path ([System.IO.Path]::GetTempPath()) ("vntype-win-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

try {
    $Files = @("vntype.exe", "vntype-settings.exe", "vntype-hook.exe", "vntype_tsf.dll")
    foreach ($File in $Files) {
        $Src = Join-Path $BinDir $File
        if (-not (Test-Path $Src)) {
            throw "Expected build artifact not found: $Src"
        }
        Copy-Item -Path $Src -Destination (Join-Path $StageDir $File) -Force
    }

    $Readme = @"
VNType $Version Portable ($Arch)
================================

Files:
  - vntype-hook.exe      Low-level keyboard hook IME (portable, no admin required)
  - vntype-settings.exe  Settings GUI
  - vntype.exe           CLI utility
  - vntype_tsf.dll       Text Services Framework (TSF) in-process DLL

Quick Start (Portable Hook):
  Run vntype-hook.exe directly. Use vntype-settings.exe to configure input method,
  tone placement, and per-app rules.

Optional TSF Registration (Administrator):
  regsvr32 vntype_tsf.dll
  (To unregister: regsvr32 /u vntype_tsf.dll)
"@
    Set-Content -Path (Join-Path $StageDir "README.txt") -Value $Readme -Encoding UTF8

    $ZipPath = Join-Path $OutDir "vntype-$Version-windows-$Arch-portable.zip"
    if (Test-Path $ZipPath) {
        Remove-Item -Path $ZipPath -Force
    }
    Compress-Archive -Path (Join-Path $StageDir "*") -DestinationPath $ZipPath -CompressionLevel Optimal
    Write-Host "Created portable package: $ZipPath"
}
finally {
    if (Test-Path $StageDir) {
        Remove-Item -Path $StageDir -Recurse -Force
    }
}
