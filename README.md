# A small hot corners implementation for Windows 10/11

Provides hot corner functionality for Windows 10/11, similar to GNOME.

Configuration is currently hard-coded. The following parameters can be modified prior to compilation:

* `HOT_CORNER` - Coordinates of the hot corner, defaults to the top-left corner
* `HOT_CORNER_INPUT` - The input sequence to be sent on activation, defaults to `Win+Tab`
* `EXIT_HOT_KEY` - Base key for exiting the program, combined with `EXIT_HOT_KEY_MODIFIERS`, defaults to `C`
* `EXIT_HOT_KEY_MODIFIERS` - Modifier key(s) for exiting the program, combined with `EXIT_HOT_KEY`, defaults to `Alt+Ctrl`

Inspired by and adapted from https://github.com/taviso/hotcorner

## Build and Install (PowerShell)
```
git clone https://github.com/timrobertsdev/hotcorners.git
cd hotcorners
cargo build
cp .\target\release\hotcorners.exe "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup\"
```

## Uninstall (PowerShell)
```
rm "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup\hotcorners.exe"
```

## Todo:

* Tray icon
* Multi-monitor support
* Multiple hot corners support
* Command-line flag support
* Config file support
* GitHub CI/Release support