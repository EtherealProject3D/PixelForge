[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$env:PIXELFORGE_DEV = '1'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$logPath = Join-Path $repositoryRoot 'target\launcher.log'
$loadingWindow = $null
$launchMutex = $null
$ownsMutex = $false
try {
    Set-Location -LiteralPath $repositoryRoot
    New-Item -ItemType Directory -Path (Join-Path $repositoryRoot 'target') -Force | Out-Null
    $exePath = Join-Path $repositoryRoot 'target\release\pixelforge.exe'
    # Launch a current build directly; the executable owns the instance warning.
    $needsBuild = -not (Test-Path -LiteralPath $exePath)
    if (-not $needsBuild) {
        $builtAt = (Get-Item -LiteralPath $exePath).LastWriteTimeUtc
        $inputs = @('src', 'assets', '.cargo', 'Cargo.toml', 'Cargo.lock', 'build.rs')
        foreach ($inputName in $inputs) {
            $inputPath = Join-Path $repositoryRoot $inputName
            if ((Test-Path -LiteralPath $inputPath) -and (Get-ChildItem -LiteralPath $inputPath -File -Recurse | Where-Object { $_.LastWriteTimeUtc -gt $builtAt } | Select-Object -First 1)) {
                $needsBuild = $true
                break
            }
        }
    }
    if (-not $needsBuild) {
        Start-Process -FilePath $exePath -WorkingDirectory $repositoryRoot
        return
    }
    $launchMutex = New-Object System.Threading.Mutex($false, 'Local\PixelForgeDevBuildLauncher')
    $ownsMutex = $launchMutex.WaitOne(0)
    if (-not $ownsMutex) { return }
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    $loadingWindow = New-Object System.Windows.Forms.Form
    $loadingWindow.Text = 'Pixel Forge Dev is loading...'
    $loadingWindow.StartPosition = 'CenterScreen'
    $loadingWindow.ClientSize = New-Object System.Drawing.Size(360, 95)
    $loadingWindow.FormBorderStyle = 'FixedDialog'
    $loadingWindow.ControlBox = $false
    $loadingWindow.UseWaitCursor = $true
    $loadingLabel = New-Object System.Windows.Forms.Label
    $loadingLabel.Dock = 'Fill'
    $loadingLabel.TextAlign = 'MiddleCenter'
    $loadingLabel.Text = 'Pixel Forge Dev is loading...'
    $loadingWindow.Controls.Add($loadingLabel)
    $loadingWindow.Show()
    [System.Windows.Forms.Application]::DoEvents()
    # Windows PowerShell can turn native stderr warnings into terminating errors.
    # Redirect the process streams directly and use its exit code instead.
    $errorLogPath = Join-Path $repositoryRoot 'target\launcher-errors.log'
    $build = Start-Process -FilePath (Get-Command cargo -CommandType Application).Source -ArgumentList @('build', '--locked', '--release') -WorkingDirectory $repositoryRoot -WindowStyle Hidden -RedirectStandardOutput $logPath -RedirectStandardError $errorLogPath -PassThru
    # Keep the process handle open so Windows PowerShell can read the exit code after Cargo exits.
    $null = $build.Handle
    while (-not $build.HasExited) {
        [System.Windows.Forms.Application]::DoEvents()
        Start-Sleep -Milliseconds 40
    }
    $build.WaitForExit()
    $exitCode = $build.ExitCode
    $build.Dispose()
    if ($null -eq $exitCode) { throw "Could not read Cargo's exit code. Details: $errorLogPath" }
    if ($exitCode -ne 0) { throw "Build failed (Cargo exit code $exitCode). Details: $errorLogPath" }
    $loadingWindow.Close()
    $loadingWindow.Dispose()
    $loadingWindow = $null
    Start-Process -FilePath (Join-Path $repositoryRoot 'target\release\pixelforge.exe') -WorkingDirectory $repositoryRoot
} catch {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'PixelForge could not start') | Out-Null
    exit 1
} finally {
    if ($loadingWindow) { $loadingWindow.Close(); $loadingWindow.Dispose() }
    if ($ownsMutex) { $launchMutex.ReleaseMutex() }
    if ($launchMutex) { $launchMutex.Dispose() }
}
