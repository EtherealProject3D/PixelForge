# Installed release and development copy

- **PixelForge**: installed release in `C:\Program Files\Pixel Forge`, opened from its normal desktop/Start menu shortcuts. Uses published releases and the in-app updater.
- **PixelForge Dev**: the source checkout, opened by `scripts\Launch-PixelForge.vbs` or `scripts\Run-PixelForge.ps1`. The launcher builds the current local checkout before opening it and sets `PIXELFORGE_DEV=1`.

Development windows are titled **PixelForge Dev v…**. Window placement, mouse/UI controls, and sidebar preferences are stored separately under `%APPDATA%\PixelForgeDev`; the installed release keeps `%APPDATA%\PixelForge`.

The dev copy does not install published app updates. Pull or edit source, close the dev copy, then reopen its shortcut to build it. Changes stay on a development branch until intentionally merged and released.

Model files, AI engines, and the local ComfyUI endpoint remain shared to avoid duplicating large installations. Dependency/model installation, updates, or removal can therefore affect both copies. Use separate `.pxf` files for test projects when preserving a production project.

To launch manually in PowerShell:

```powershell
$env:PIXELFORGE_DEV = '1'
cargo run --locked --release
```

The flag is inherited only by that process tree; it is not added to Windows system environment settings. The installed app launched from its normal shortcuts runs normally.
