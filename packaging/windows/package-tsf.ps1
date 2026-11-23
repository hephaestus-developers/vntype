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

$CargoArgs = @("build", "--release", "-p", "vntype-tsf", "-p", "vntype-settings", "-p", "vntype-cli")
if (-not [string]::IsNullOrEmpty($Target)) {
    $CargoArgs += @("--target", $Target)
    $BinDir = Join-Path $RootDir "target\$Target\release"
} else {
    $BinDir = Join-Path $RootDir "target\release"
}

if ($Offline -or $env:VNTYPE_OFFLINE -eq "1") {
    $CargoArgs += "--offline"
}

Write-Host "Building VNType $Version ($Arch) TSF for Windows..."
& cargo @CargoArgs --manifest-path (Join-Path $RootDir "Cargo.toml")
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed with exit code $LASTEXITCODE"
}

$StageDir = Join-Path ([System.IO.Path]::GetTempPath()) ("vntype-tsf-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

try {
    foreach ($File in @("vntype_tsf.dll", "vntype-settings.exe", "vntype-cli.exe")) {
        $Src = Join-Path $BinDir $File
        if (-not (Test-Path $Src)) {
            throw "Expected build artifact not found: $Src"
        }
        Copy-Item -Path $Src -Destination (Join-Path $StageDir $File) -Force
    }

    $Readme = @"
VNType $Version TSF ($Arch)
===========================

Files:
  - vntype_tsf.dll       Text Services Framework (TSF) in-process COM DLL
  - vntype-settings.exe  Settings GUI
  - vntype-cli.exe       CLI utility

Installation (Administrator):
  regsvr32 vntype_tsf.dll

Uninstall (Administrator):
  regsvr32 /u vntype_tsf.dll
"@
    Set-Content -Path (Join-Path $StageDir "README.txt") -Value $Readme -Encoding UTF8

    $ZipPath = Join-Path $OutDir "vntype-$Version-windows-$Arch-tsf.zip"
    if (Test-Path $ZipPath) {
        Remove-Item -Path $ZipPath -Force
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $StageDir,
        $ZipPath,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $false
    )
    Write-Host "Created TSF package: $ZipPath"
}
finally {
    if (Test-Path $StageDir) {
        Remove-Item -Path $StageDir -Recurse -Force
    }
}
