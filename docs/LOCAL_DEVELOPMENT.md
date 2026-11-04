# Installed release and development copy

- **PixelForge**: installed release in `C:\Program Files\Pixel Forge`, opened from its normal desktop/Start menu shortcuts. Uses published releases and the in-app updater.
- **PixelForge Dev**: the source checkout, opened by `scripts\Launch-PixelForge.vbs` or `scripts\Run-PixelForge.ps1`. The launcher builds the current local checkout before opening it and sets `PIXELFORGE_DEV=1`.

Development windows are titled **PixelForge Dev v…**. Window placement, mouse/UI controls, and sidebar preferences are stored separately under `%APPDATA%\PixelForgeDev`; the installed release keeps `%APPDATA%\PixelForge`.

Window size and screen position use the framework's native persistence in `window-state.ron`, saved while idle and on exit. Old `window.cfg` and `window_native.cfg` files are no longer applied. After upgrading to this mechanism, position the window once to establish its new saved placement.

The dev copy does not install published app updates. Pull or edit source, close the dev copy, then reopen its shortcut to build it. Changes stay on a development branch until intentionally merged and released.

Model files, AI engines, and the local ComfyUI endpoint remain shared to avoid duplicating large installations. Dependency/model installation, updates, or removal can therefore affect both copies. Use separate `.pxf` files for test projects when preserving a production project.

To launch manually in PowerShell:

```powershell
$env:PIXELFORGE_DEV = '1'
cargo run --locked --release
```

The flag is inherited only by that process tree; it is not added to Windows system environment settings. The installed app launched from its normal shortcuts runs normally.

## v0.6.3 display testing

Open PixelForge Dev using the existing shortcut. In Settings > Display choose
Fullscreen, or press F11. The title bar and taskbar should disappear across the
entire current monitor. Press Esc to return to the previous window placement.
Repeat from a maximized window. Close while fullscreen, reopen, and press Esc.
Check at 150% Windows scaling and on a 32:9 display; change UI scale while
fullscreen and check that returning to windowed preserves the physical size.
Native monitor behavior still requires visual hardware verification.

Display mode and the last normal rectangle are stored in display.json in the
existing profile folder. Existing controls and native window-state files remain.
The stable installation, models, and ComfyUI environments are not replaced.
