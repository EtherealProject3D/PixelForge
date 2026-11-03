# Isolated updater smoke test. Never replaces or launches the installed PixelForge.
[CmdletBinding()]
param([string]$Installer)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$testRoot = Join-Path $repoRoot ('target\update-smoke-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$builtExe = Join-Path $repoRoot 'target\release\pixelforge.exe'
if (-not (Test-Path -LiteralPath $builtExe)) {throw 'Build the release executable first.'}
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zipFile = Join-Path $testRoot 'update.zip'
$zip = [IO.Compression.ZipFile]::Open($zipFile, [IO.Compression.ZipArchiveMode]::Create)
try { [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip,$builtExe,'PixelForge.exe') | Out-Null } finally { $zip.Dispose() }
$env:PF_UPDATE_ZIP = $zipFile
$env:PF_UPDATE_HASH = (Get-FileHash -LiteralPath $zipFile -Algorithm SHA256).Hash.ToLowerInvariant()
$env:PF_UPDATE_STAGED = Join-Path $testRoot 'verified.exe'
$env:PF_UPDATE_VERSION = [Diagnostics.FileVersionInfo]::GetVersionInfo($builtExe).ProductVersion
function Invoke-TestHelper([string]$helper,[string]$name) {
    $arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + (Join-Path $PSScriptRoot $helper) + '"'
    $process = Start-Process powershell.exe -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $testRoot ($name+'.out')) -RedirectStandardError (Join-Path $testRoot ($name+'.err'))
    return $process.ExitCode
}
if ((Invoke-TestHelper 'Stage-Update.ps1' 'valid') -ne 0) {throw 'Valid package staging failed.'}
if ((Get-FileHash $env:PF_UPDATE_STAGED).Hash -ne (Get-FileHash $builtExe).Hash) {throw 'Staged executable differs.'}
$env:PF_UPDATE_HASH = '0' * 64
$env:PF_UPDATE_STAGED = Join-Path $testRoot 'must-not-exist.exe'
if ((Invoke-TestHelper 'Stage-Update.ps1' 'invalid') -eq 0 -or (Test-Path $env:PF_UPDATE_STAGED)) {throw 'Corrupt package was accepted.'}
if ($Installer) {
    $env:PF_UPDATE_ZIP = (Resolve-Path -LiteralPath $Installer).Path
    $env:PF_UPDATE_HASH = (Get-FileHash -LiteralPath $env:PF_UPDATE_ZIP -Algorithm SHA256).Hash.ToLowerInvariant()
    $env:PF_UPDATE_STAGED = Join-Path $testRoot 'verified-setup.exe'
    $env:PF_UPDATE_SETUP = '1'
    if ((Invoke-TestHelper 'Stage-Update.ps1' 'setup') -ne 0) {throw 'Setup package verification failed.'}
    if ((Get-FileHash $env:PF_UPDATE_STAGED).Hash -ne (Get-FileHash $env:PF_UPDATE_ZIP).Hash) {throw 'Staged installer differs.'}
    $env:PF_UPDATE_SETUP = '0'
}
# A tiny disposable GUI executable stands in for the restarted app.
$mockExe = Join-Path $testRoot 'mock.exe'
Add-Type -TypeDefinition 'public class UpdateSmoke { public static void Main() { System.IO.File.WriteAllText(System.Environment.GetEnvironmentVariable("PF_UPDATE_TEST_MARKER"), "restarted"); } }' -OutputAssembly $mockExe -OutputType WindowsApplication
$env:PF_UPDATE_STAGED = $mockExe
$env:PF_UPDATE_TARGET = Join-Path $testRoot 'installed.exe'
$env:PF_UPDATE_PID = '2147480000'
$env:PF_UPDATE_WORKDIR = $testRoot
$env:PF_UPDATE_LOG = Join-Path $testRoot 'install-error.log'
$env:PF_UPDATE_TEST_MARKER = Join-Path $testRoot 'restarted.txt'
[IO.File]::WriteAllText($env:PF_UPDATE_TARGET,'original executable fixture')
if ((Invoke-TestHelper 'Install-Update.ps1' 'install') -ne 0) {throw 'Fixture installation failed.'}
for ($attempt=0;$attempt -lt 30 -and -not(Test-Path $env:PF_UPDATE_TEST_MARKER);$attempt++) {Start-Sleep -Milliseconds 100}
if (-not(Test-Path $env:PF_UPDATE_TEST_MARKER)) {throw 'Fixture did not restart.'}
if ([IO.File]::ReadAllText($env:PF_UPDATE_TARGET+'.previous') -ne 'original executable fixture') {throw 'Original executable backup was not preserved.'}
Write-Host 'PASS: verified staging, rejected bad checksum, retained original, replaced and restarted isolated fixture.'
Write-Host "Fixture files: $testRoot"
