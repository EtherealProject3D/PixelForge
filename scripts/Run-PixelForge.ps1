[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
Write-Host "Building and launching PixelForge from: $repositoryRoot"
Write-Host 'Expected window title: PixelForge v0.5.1-stage1'
& cargo run --locked --release --manifest-path (Join-Path $repositoryRoot 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw "PixelForge build/run failed ($LASTEXITCODE). Close running copies and check the build output." }
