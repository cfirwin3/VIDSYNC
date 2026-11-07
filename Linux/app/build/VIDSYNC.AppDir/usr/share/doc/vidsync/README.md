# VIDSYNC

VIDSYNC is a minimalist Linux video player that follows Audacity 4.0's playback
position. It reads the timecode Audacity already shows — `00h00m00.000s` — over
the desktop accessibility bus and scrolls the picture to the same time. It does
not send keys, take focus or care which window is on top. When Audacity plays
the picture moves; when Audacity pauses or stops the picture freezes.

The player shows video only. Audio is dropped, so it can run beside an audio
session without doubling the sound. There are no play, pause, record or start
controls: Audacity's transport is the only transport.

```
┌──────────────────────────────────────────────────────────────┐
│  ▶ VIDSYNC                                   sample.mp4  ⚙  │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                         video picture                        │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│  ● Audacity                                   00:00:11.037   │
│    Following Audacity                                        │
└──────────────────────────────────────────────────────────────┘
```

## What it does

- **Follows Audacity 4.0.** The timecode is read from Audacity's accessible
  `Playback position` object about thirty times a second. It is independent of
  window order, focus and stacking, so the player can stay behind or in front.
- **Green / red status dot.** Green means Audacity 4.0 is running and its
  timecode display is set to `00h00m00.000s`; red means either was not found.
  The matching `00:00:00.000` readout is shown on the right.
- **Moves with the clock.** A changing Audacity timecode scrolls the video in
  real time. A still timecode holds the current frame. A backward jump, a seek,
  or Audacity's stop-and-return-to-zero moves the picture with it.
- **Video only.** Audio, subtitle and data streams are dropped during decode.
- **Most formats.** Decoding is delegated to `ffmpeg`, so MP4, MKV, MOV, WebM,
  AVI, MPEG, TS, MTS, OGV, FLV, WMV and other common containers work as long as
  the bundled or system `ffmpeg` supports them.
- **Minimal interface.** A compact header, the picture, and one status bar.
  The picture runs edge-to-edge between the header and the status bar, with
  black letterbox or pillarbox bars protecting its aspect ratio. There is no
  timecode entry field and no transport strip.
- **Optional appearance and window settings.** The gear offers Dark / Light,
  six accent colours (orange, yellow, green, blue, purple and red), and a
  **Keep on top** switch that holds the player above other windows. Dark,
  purple and off are the defaults, and all three choices are remembered on the
  next launch. The menu, desktop and window icons use the same purple play tile
  shown beside the name.
- **File browser only.** Videos are opened from the built-in browser. There is
  no drag-and-drop target, no sample-clip shortcut and no empty-stage prompt;
  when nothing is loaded the picture area stays black. No GTK dialogs are
  needed.
- **Ships as an AppImage** with `ffmpeg`/`ffprobe` inside, so the target machine
  does not need ffmpeg installed.

## Build and run

Requirements for building:

- Rust and Cargo (edition 2021)
- A Linux desktop with OpenGL and an accessibility bus
- `ffmpeg` and `ffprobe` for running from a normal build
- `curl`, `tar`, `xz` and `appimagetool` (downloaded automatically) for the
  AppImage script

From this folder:

```sh
cargo build --release
./target/release/vidsync
```

Videos are opened from the Open video button inside the player.

Debug build and tests:

```sh
cargo run
cargo build
cargo test
```

The app looks for `ffmpeg` and `ffprobe` next to its executable first, then on
`PATH`. Override them with `CUE_FFMPEG` and `CUE_FFPROBE` if needed.

## Connecting Audacity 4.0

1. Start Audacity 4.0 and leave its playback toolbar visible.
2. Set Audacity's timecode display to `00h00m00.000s` (the hour / minute /
   second / millisecond form). The colon form still follows, but the status dot
   stays red because it is not the requested display.
3. Start VIDSYNC and open a video. The dot turns green when Audacity's timecode
   object is found.
4. Play, pause or stop in Audacity. VIDSYNC follows the readout; it never seeks
   Audacity and never brings a window to the front.

If the dot stays red, make sure the desktop accessibility bus is running
(`org.a11y.Bus`) and that Audacity's playback toolbar is shown. Minimal window
managers sometimes start without it.

## Controls

| Input | Action |
| --- | --- |
| O or the header Open video button | Open the file browser |
| F or the header fullscreen button | Toggle fullscreen |
| Escape | Exit fullscreen, or close the settings panel |
| Gear button | Settings panel: Dark / Light, Keep on top, accent colour |

Fullscreen shows only the video on a black background; the header, status bar
and all other overlays are hidden. The picture keeps its aspect ratio, so black
bars appear when the screen shape differs from the video.

There are deliberately no playback buttons, no start-time field, no drag and
drop, and no sample-clip shortcut. The empty stage has no text or buttons; use
the Open video button in the header to browse for a file.

## AppImage

Build the AppImage with:

```sh
./scripts/build-appimage.sh
```

The script downloads the static `ffmpeg`/`ffprobe` build and `appimagetool`,
stages an AppDir under `build/`, and writes:

```
dist/VIDSYNC-1.0.0-x86_64.AppImage
```

Run it with:

```sh
chmod +x dist/VIDSYNC-1.0.0-x86_64.AppImage
./dist/VIDSYNC-1.0.0-x86_64.AppImage
```

The AppImage contains its own ffmpeg and does not require FUSE to build. If your
system does not have FUSE, run it with
`./dist/VIDSYNC-1.0.0-x86_64.AppImage --appimage-extract-and-run`.

## Verification flags

The binary supports two non-interactive modes used by the build checks:

```sh
# Draw the app for ~120 frames, save a PNG, and exit.
./target/release/vidsync --screenshot shots/now.png

# Drive the picture from a simulated Audacity clock, then write a JSON report.
./target/release/vidsync --self-test shots/test.json
```

The self-test writes `{"version": 1, "steps": [...]}` with `move`, `act` and
`hud` steps. Both modes use the embedded sample clip extracted to the system
temporary directory, so they work without any external media.

## Project layout

| Path | Purpose |
| --- | --- |
| `src/main.rs` | app state, compact layout, drawing, screenshot/self-test |
| `src/player.rs` | ffprobe metadata, ffmpeg decode loop, follow and hold |
| `src/browser.rs` | in-app file browser |
| `src/audacity.rs` | AT-SPI timecode discovery, polling and movement detection |
| `src/topmost.rs` | optional X11 keep-above window request |
| `src/ui.rs` | palette, themes, fonts, shapes, time formatting |
| `assets/sample.mp4` | small generated clip used only by the screenshot and self-test checks |
| `assets/vidsync.png` | app icon matching the in-app purple play logo |
| `assets/fonts/` | bundled DejaVu fonts (Bitstream Vera licence included) |
| `MECHANICS.md`, `mechanics.json` | values behind playback, follow and layout |
| `scripts/build-appimage.sh` | reproducible AppImage build |

## Known limits

- The desktop accessibility bus must be available. VIDSYNC can switch it on, but
  it cannot create one where none is installed.
- Audacity 4.0.2 on Linux is the version observed while building. The accessible
  object name used here is `Playback position:00 h 00 m 00.000 s`. A translated
  label still works as long as the name ends in a readable clock, and a bare
  clock name also works; a future Audacity release could change the format.
- Audacity's stop action returns its play head to the start of the selection or
  project; VIDSYNC follows that position, so the picture returns with it.
- Frame-accurate seeking depends on keyframes and on `ffmpeg`'s input seeking;
  the picture is accurate to about a frame, not sample-accurate.
- Video is capped at 1280x720 and 30 fps to keep decode affordable. The cap can
  be raised in `src/player.rs` (and in `mechanics.json`).
- The dark/light choice, accent colour and Keep on top switch are stored in
  `$XDG_CONFIG_HOME/vidsync/settings.json` (usually under `.config`) and restored
  on the next launch. Keep on top uses the X11 `_NET_WM_STATE_ABOVE` protocol;
  a Wayland compositor that does not expose that protocol may ignore it.
- The AppImage is built on x86_64; other architectures need their own static
  ffmpeg and appimagetool build.

## Licence

VIDSYNC is MIT licensed; see `LICENSE`. The bundled DejaVu fonts remain under the
Bitstream Vera licence in `assets/fonts/LICENSE-DejaVu.txt`.
