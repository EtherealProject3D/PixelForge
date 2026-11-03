$ErrorActionPreference = 'Stop'
$backup = $env:PF_UPDATE_TARGET + '.previous'
$replaced = $false
$backupCreated = $false
try {
    $parentProcess = Get-Process -Id ([int]$env:PF_UPDATE_PID) -ErrorAction SilentlyContinue
    if ($parentProcess) { $parentProcess | Wait-Process -Timeout 60 -ErrorAction Stop }
    Copy-Item -LiteralPath $env:PF_UPDATE_TARGET -Destination $backup -Force
    $backupCreated = $true
    Copy-Item -LiteralPath $env:PF_UPDATE_STAGED -Destination $env:PF_UPDATE_TARGET -Force
    $replaced = $true
    Start-Process -FilePath $env:PF_UPDATE_TARGET -WorkingDirectory $env:PF_UPDATE_WORKDIR
} catch {
    $message = $_.Exception.Message
    if ($backupCreated -and (Test-Path -LiteralPath $backup)) {
        try { Copy-Item -LiteralPath $backup -Destination $env:PF_UPDATE_TARGET -Force } catch {}
    }
    $message | Set-Content -LiteralPath $env:PF_UPDATE_LOG
    Add-Type -AssemblyName System.Windows.Forms
    [Windows.Forms.MessageBox]::Show("PixelForge could not finish updating. Your previous executable has been retained. Details: $message", 'PixelForge update') | Out-Null
    if ($replaced) { try { Start-Process -FilePath $env:PF_UPDATE_TARGET -WorkingDirectory $env:PF_UPDATE_WORKDIR } catch {} }
    exit 1
}
