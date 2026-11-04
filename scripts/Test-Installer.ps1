# Compile with Build-Installer.ps1 -SmokeTest before running this isolated fixture.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Installer)
$ErrorActionPreference = 'Stop'
$Installer = (Resolve-Path -LiteralPath $Installer).Path
if (-not $Installer.EndsWith('-smoke-setup.exe')) {throw 'Only the isolated SmokeTest installer may be used by this test.'}
$repoRoot = Split-Path -Parent $PSScriptRoot
$fixture = Join-Path $repoRoot ('target\installer-smoke-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$arguments = '/CURRENTUSER /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /TASKS="" /DIR="' + $fixture + '" /LOG="' + (Join-Path $fixture 'install.log') + '"'
$process = Start-Process $Installer -ArgumentList $arguments -Wait -PassThru -WindowStyle Hidden
if ($process.ExitCode -ne 0) {throw ('Fixture installation failed: ' + $process.ExitCode)}
$uninstaller = Join-Path $fixture 'unins000.exe'
try {
    $app = Join-Path $fixture 'PixelForge.exe'
    $startMenu = Join-Path $fixture 'Test Shortcuts\Start Menu\PixelForge.lnk'
    $desktop = Join-Path $fixture 'Test Shortcuts\Desktop\PixelForge.lnk'
    if (-not(Test-Path $app) -or -not(Test-Path $startMenu)) {throw 'App or Start menu shortcut missing.'}
    if (Test-Path $desktop) {throw 'Desktop shortcut was created without opting in.'}
    if (-not(Test-Path (Join-Path $fixture 'pixelforge-installed.txt'))) {throw 'Installed-update marker missing.'}
    $shellObject = New-Object -ComObject WScript.Shell
    $shortcut = $shellObject.CreateShortcut($startMenu)
    if ($shortcut.TargetPath -ne $app -or $shortcut.WorkingDirectory -ne $fixture) {throw 'Shortcut points to the wrong location.'}
    $beforeHash = (Get-FileHash $app).Hash
    $upgradeArguments = '/CURRENTUSER /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /TASKS="desktopicon" /DIR="' + $fixture + '"'
    $upgrade = Start-Process $Installer -ArgumentList $upgradeArguments -Wait -PassThru -WindowStyle Hidden
    if ($upgrade.ExitCode -ne 0) {throw 'Fixture upgrade failed.'}
    if (-not(Test-Path $desktop)) {throw 'Opted-in desktop shortcut missing.'}
    if ((Get-FileHash ($app+'.previous')).Hash -ne $beforeHash) {throw 'Previous executable backup missing or incorrect.'}
    [IO.File]::WriteAllText((Join-Path $fixture 'photo-project.pxf'),'user project fixture')
    $downloadsFixture = Join-Path $fixture 'test-managed-downloads'
    New-Item -ItemType Directory -Path $downloadsFixture -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $downloadsFixture 'model.bin'),'download fixture')
} finally {
    if (Test-Path $uninstaller) {
        $uninstall = Start-Process $uninstaller -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait -PassThru -WindowStyle Hidden
        if ($uninstall.ExitCode -ne 0) {throw 'Fixture uninstall failed.'}
    }
}
if (Test-Path (Join-Path $fixture 'PixelForge.exe')) {throw 'Uninstall left the executable behind.'}
if (Test-Path (Join-Path $fixture 'Test Shortcuts\Start Menu\PixelForge.lnk')) {throw 'Uninstall left a shortcut behind.'}
if ([IO.File]::ReadAllText((Join-Path $fixture 'photo-project.pxf')) -ne 'user project fixture') {throw 'Uninstall changed the user project.'}
if ([IO.File]::ReadAllText((Join-Path $fixture 'test-managed-downloads\model.bin')) -ne 'download fixture') {throw 'Silent uninstall removed managed downloads.'}
Write-Host 'PASS: installation, Start menu target, optional desktop shortcut, upgrade backup, and uninstall preserving a project.'
Write-Host "Fixture: $fixture"
