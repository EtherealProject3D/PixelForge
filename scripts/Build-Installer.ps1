[CmdletBinding()]
param(
    [string]$Compiler,
    [string]$SourceDir,
    [string]$OutputDir,
    [switch]$SmokeTest
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$version = (Get-Content (Join-Path $repoRoot 'Cargo.toml') | Select-String '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
if (-not $Compiler) {
    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($command) { $Compiler = $command.Source }
    else { $Compiler = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe' }
}
if (-not (Test-Path -LiteralPath $Compiler)) { throw 'Inno Setup 6 compiler not found. Pass -Compiler with the path to ISCC.exe.' }
if (-not $SourceDir) {
    $SourceDir = Join-Path $repoRoot 'target\installer-source'
    New-Item -ItemType Directory -Path $SourceDir -Force | Out-Null
    Copy-Item (Join-Path $repoRoot 'target\release\pixelforge.exe') (Join-Path $SourceDir 'PixelForge.exe') -Force
    Copy-Item (Join-Path $repoRoot 'README.md') $SourceDir -Force
    $docsFolder = Join-Path $SourceDir 'docs'
    New-Item -ItemType Directory -Path $docsFolder -Force | Out-Null
    Copy-Item (Join-Path $repoRoot 'docs\*') $docsFolder -Recurse -Force
}
if (-not $OutputDir) { $OutputDir = Join-Path $repoRoot 'target\installer' }
$OutputDir = [IO.Path]::GetFullPath($OutputDir)
$SourceDir = (Resolve-Path -LiteralPath $SourceDir).Path
$compileArgs = @('/Qp', "/DAppVersion=$version", "/DSourceDir=$SourceDir", "/DOutputDir=$OutputDir")
if ($SmokeTest) { $compileArgs += @('/DSmokeTest', '/DInstallerId=EtherealProject3D.PixelForge.SmokeTest') }
& $Compiler @compileArgs (Join-Path $repoRoot 'installer\PixelForge.iss')
if ($LASTEXITCODE -ne 0) { throw "Installer compilation failed ($LASTEXITCODE)" }
$installerName = if ($SmokeTest) { "PixelForge-v$version-windows-x64-smoke-setup.exe" } else { "PixelForge-v$version-windows-x64-setup.exe" }
& (Join-Path $PSScriptRoot 'Write-ReleaseIntegrity.ps1') -Files @((Join-Path $OutputDir $installerName), (Join-Path $SourceDir 'PixelForge.exe')) -OutputDir $OutputDir
Write-Host "Installer output: $OutputDir"
