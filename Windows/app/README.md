# VIDSYNC for Windows

VIDSYNC is a small video player that follows Audacity 4.0's playback timecode.
Audacity is the only transport: when its clock moves the picture moves, and
when the clock stops the picture holds the current frame. Audio is not played.

This folder is the Windows port of the Linux AppImage. The interface, layout,
colours, shortcuts and follow rules are the same; the desktop integration uses
Windows UI Automation, `%APPDATA%` and Win32 instead of AT-SPI, XDG and X11. The
Windows build is a GUI program, so launching it does not open a console window.

## Ready-to-run package

`dist/VIDSYNC-1.0.0-windows-x86_64.zip` is the complete player: `vidsync.exe`
plus the matching `ffmpeg.exe` and `ffprobe.exe`, so it needs no separate
installation. Unzip it anywhere and run `vidsync.exe`. The loose
`dist/VIDSYNC-1.0.0-x86_64.exe` is the same player without the sidecar tools.

## What you need

- Windows 10 or 11 (64-bit)
- Audacity 4.0 running with its playback toolbar visible
- Rust and Cargo only if you want to build from source
- If you use the loose executable, put `ffmpeg.exe` and `ffprobe.exe` next to
  it, in a `tools` folder beside it, on `PATH`, or name them with `CUE_FFMPEG` /
  `CUE_FFPROBE`

## Build

```powershell
cd app
cargo build --release
```

The player is written in Rust. For a Windows executable from a Linux build
machine, install the MinGW linker and run:

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

The result is `target/x86_64-pc-windows-gnu/release/vidsync.exe`. The included
`dist` executable was produced with that cross-compiler and checked as a
64-bit Windows GUI program. Copy it into a folder together with `ffmpeg.exe`
and `ffprobe.exe`, or install ffmpeg on `PATH`.

## Use

| Input | Action |
| --- | --- |
| Open video button | Browse for a video file |
| `O` | Open the file browser |
| `F` or the fullscreen button | Toggle fullscreen |
| `Escape` | Leave fullscreen or close settings |
| Gear button | Dark / Light, Keep on top, accent colour |

Open a video first, then start playback in Audacity. The dot at the bottom left
is green when Audacity is found and its display is set to `00h00m00.000s`; it is
red otherwise. The timecode on the right mirrors Audacity's position.

The Dark / Light choice, accent colour and Keep on top switch are remembered in
`%APPDATA%\vidsync\settings.json`. The player never sends keys to Audacity and
never changes which window has focus.

## Non-interactive modes

```powershell
vidsync.exe --screenshot shots\now.png
vidsync.exe --self-test shots\test.json
```

The first draws the interface for about 120 frames with a bundled sample clip,
saves a PNG and exits. The second drives the same sample from a simulated
Audacity clock and writes a JSON report. These support the build checks; they
are not part of normal use.

## Limits

- UI Automation must be available. The player cannot install an accessibility
  layer that is missing from Windows.
- Audacity 4.0.2 was the observed version. A future release could change its
  accessible timecode text.
- Video is decoded at up to 1280x720 and 30 frames per second, matching the
  original. Seeking is accurate to about a frame, not sample-accurate.
- The Windows accessibility mapping and the packaged `.exe` have been
  cross-compiled and checked, but not exercised on a Windows machine here.
- The package includes the official gyan.dev ffmpeg 9.0.2 "essentials" build
  for Windows, with its licence, so video decoding works out of the box.

See `MECHANICS.md` for every recovered value and its source.
