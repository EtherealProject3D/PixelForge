[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$env:PIXELFORGE_DEV = '1'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$logPath = Join-Path $repositoryRoot 'target\launcher.log'
try {
    Set-Location -LiteralPath $repositoryRoot
    New-Item -ItemType Directory -Path (Join-Path $repositoryRoot 'target') -Force | Out-Null
    # Windows PowerShell can turn native stderr warnings into terminating errors.
    # Redirect the process streams directly and use its exit code instead.
    $errorLogPath = Join-Path $repositoryRoot 'target\launcher-errors.log'
    $build = Start-Process -FilePath (Get-Command cargo -CommandType Application).Source -ArgumentList @('build', '--locked', '--release') -WorkingDirectory $repositoryRoot -WindowStyle Hidden -RedirectStandardOutput $logPath -RedirectStandardError $errorLogPath -Wait -PassThru
    if ($build.ExitCode -ne 0) { throw "Build failed. Details: $errorLogPath`nClose any running PixelForge window and try again." }
    Start-Process -FilePath (Join-Path $repositoryRoot 'target\release\pixelforge.exe') -WorkingDirectory $repositoryRoot
} catch {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'PixelForge could not start') | Out-Null
    exit 1
}
