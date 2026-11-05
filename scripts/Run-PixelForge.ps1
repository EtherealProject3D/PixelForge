[CmdletBinding()]
param()
# Share loading feedback and duplicate-launch behavior with the desktop shortcut.
& (Join-Path $PSScriptRoot 'Launch-PixelForge.ps1')
