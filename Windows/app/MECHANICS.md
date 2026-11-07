# VIDSYNC mechanics (Windows port)

This sheet records the values, formulas and rules behind VIDSYNC. The original
is the Linux `VIDSYNC-1.0.0-x86_64.AppImage` (SHA-256
`4c2cb950be053d8002703605bb9f19d7d9bee25c77940085666abb1294e641f2`; opened
target `t_4c2cb950be05`, triage evidence `ev_157e60ef7dc561fb1980a730f735317124461339f467ebede84657c374546a18`). Values
recovered from its code cite a function address in the stripped x86_64 binary;
floating-point literals also cite the `.rodata` address. Items that exist only
because the port targets Windows, or because the person asked for them, are
marked **port** or **user request** instead of being presented as original
constants. The same values live in [`mechanics.json`](mechanics.json).

VIDSYNC is a slave video player: Audacity 4.0's playback timecode is the only
clock. The picture follows that clock, so the player has no transport of its
own. This port keeps that model on Windows and replaces the Linux desktop
integration with UI Automation and Win32.

## Following Audacity

| Value | Setting | Source |
| --- | --- | --- |
| Timecode source | UI Automation accessible Name, then the Value pattern; text such as `Playback position:00 h 00 m 11.037 s` (AT-SPI on Linux) | functions `0x3f6f40`, `0x3f6db0`, `0x4018a0`, `0x4019d0`; **port** mapping |
| Timecode readout format | `HH:MM:SS.mmm`, e.g. `00:00:11.037` | user request |
| Required Audacity display | `00h00m00.000s` for a green dot | user request |
| Green dot | Audacity found and its timecode uses the hour/minute/second display | user request |
| Red dot | Audacity not found, or its timecode uses another display | user request |
| Window order | irrelevant; no focus change or key injection | user request |
| Timecode poll interval | `33 ms` (literal `1/30 s`) | function `0x3f7a10`; `.rodata 0x12438` |
| Movement window | `220 ms` | function `0x3f7a10` |
| Movement threshold | `0.0005 s` | function `0x3f7a10`; `.rodata 0x12548` |
| Timecode jump threshold | `0.4 s`; restart the decoder at the new position | function `0x3e2630`; `.rodata 0x12238` |
| Still-frame refresh threshold | `0.02 s` | functions `0x3e2630`, `0x3f0820`; `.rodata 0x120e0` |
| Rediscovery interval | `12 s` (`1.5 s` timer step × 8) | function `0x3f7a10` |
| Failed discovery retry | `1 s` | function `0x3f7a10` |
| Accessible-tree depth limit | `14` | function `0x3f6f40` |
| Accessible-name length limit | `96` bytes | function `0x3f6db0` (`looks_like_timecode`) |
| Audacity version observed | `4.0.2` | observed build machine |
| Desktop layer | Windows 10/11 UI Automation; the Linux build used AT-SPI | **port** |

A background thread finds the Audacity window by process name, attaches to its
UI Automation subtree, and caches the first element whose Name or Value parses
as a clock. It reads that one element about thirty times a second. Discovery is
repeated every twelve seconds because walking the whole tree is far too
expensive to do on every poll. The tree walk stops at depth `14`, and a name
longer than `96` bytes is rejected before parsing.

The recovered follow rules are:

- **Movement.** A reading is moving when
  `|position - previous_position| > 0.0005 s` and that change happened less than
  `220 ms` ago. `last_position` is reset whenever discovery fails or the text
  stops parsing.
- **Jump.** `|position - decoded_position| > 0.4 s` restarts `ffmpeg` at the new
  position. A smaller drift is absorbed by the decoder already running.
- **Hold.** When the timecode stops changing the decoder is stopped and one
  still frame is fetched. A backwards jump, or Audacity's stop-to-zero, is
  followed like any other jump.
- **Loss.** If Audacity disappears the last picture stays on screen and the dot
  turns red.

The Windows implementation reads the same Qt accessibility tree through the
`IUIAutomation` COM interfaces. The Linux AT-SPI path remains in the same module
for the original's build-machine checks. Nothing is ever sent to Audacity and no
window is raised, so stacking order does not matter.

## Playback

| Value | Setting | Source |
| --- | --- | --- |
| Playback rate | `1.0x`, paced by the external timecode | user request |
| Output frame rate | `30 fps` (`-r 30`) | function `0x3e2630`; `.rodata 0x120f0` |
| Maximum decoded width | `1280 px` | function `0x3e2630`; `.rodata 0x10eb0` |
| Maximum decoded height | `720 px` | function `0x3e2630`; `.rodata 0x10eb8` |
| Decoded frame queue | `12 frames` | function `0x3f0820` |
| Decoder channel buffer | `8 frames` | function `0x3e2630` |
| Longest accepted frame step | `0.1 s` | function `0x3d1e10` |
| Frame-due tolerance | `0.045 s` | function `0x3f0820`; `.rodata 0x12320` |
| End tolerance | `0.001 s` | function `0x3e2630`; `.rodata 0x124f8` |
| Audio output | none; audio streams are dropped | user request |
| Dropped streams | audio, subtitles and data (`-an -sn -dn`) | function `0x3e2630` |
| Decoded pixel format | `rgba` | function `0x3e2630` |
| Scaling | bicubic, aspect ratio preserved inside 1280x720 | function `0x3e2630` |
| Supported formats | whatever the bundled `ffmpeg` can decode | user request |

The decoder runs as a child process and emits raw RGBA frames on stdout. Its
arguments include `-r 30`, `-an`, `-sn`, `-dn`, `-pix_fmt rgba` and a `-vf`
filter of the form `scale=W:H:flags=bicubic,setsar=1,format=rgba`.
`ffprobe` supplies width, height and duration as JSON before playback starts
(function `0x3e7780`).

The player keeps the **earliest** twelve decoded frames and stops reading from
the pipe while that queue is full (`pending.len() < 12`). Draining every
available frame would discard the pictures the clock is at and leave only
future frames. A frame is shown when its timestamp is at most `0.045 s` beyond
the current position, which keeps the picture from running ahead of Audacity.
When the clock passes the end of a clip the player parks on the last frame
instead of respawning the decoder on every timecode tick. The end test uses
`0.001 s`. The image is letterboxed or pillarboxed and is never stretched.

On Windows the sidecar programs are found in this order: `CUE_FFMPEG` /
`CUE_FFPROBE`, the player's own folder (`ffmpeg.exe`, `ffprobe.exe`), a `tools\`
subfolder, then `PATH`. Child processes use `CREATE_NO_WINDOW`, so seeking never
flashes a console window.

## Controls and transport

Audacity is the only transport. The person asked to remove the start-time field,
the Audacity on/off switch and the play, pause and record buttons.

| Value | Setting | Source |
| --- | --- | --- |
| Play button | none | user request |
| Pause button | none | user request |
| Record button | none | user request |
| Start-time field | none | user request |
| Audacity toggle switch | none | user request |
| Open shortcut | `O` | function `0x3d1e10` |
| Fullscreen shortcut | `F` | function `0x3d1e10` |
| Fullscreen button | header, beside the gear | user request |
| Exit fullscreen | `Escape` | user request |
| Close settings | `Escape` | function `0x3d1e10` |
| File loading | built-in browser only | user request |
| Drag and drop | none | user request |
| Empty-stage message | none; the stage stays black | user request |
| Sample-clip shortcut | none | user request |
| Keep-on-top setting | on/off switch in the settings panel | user request |
| Keep-on-top default | off | **port** default |
| Keep-on-top persistence | saved with mode and accent | function `0x3f3ca0` (settings file); user request |
| Keep-on-top mechanism | `SetWindowPos(HWND_TOPMOST / HWND_NOTOPMOST)` | **port** (Win32) |
| Settings location | `%APPDATA%\vidsync\settings.json` | function `0x3f3ca0` (original XDG path); **port** mapping |

The header keeps only the wordmark, the current file name, an Open button, a
fullscreen button and the gear. The body is the picture and the status bar. With
no file loaded the body is an empty black stage with no text or button.

Fullscreen removes the header, status bar and every overlay; only the picture
and its black letterbox/pillarbox remain, and the pointer is hidden. The built-in
browser starts at the current file's folder (or the user profile) and lists
Windows drive roots when the user navigates up from `C:\`. Double-clicking a
video opens it; `Escape` cancels. The original picks `Videos`, `Downloads` or
`Desktop` as its default folder (`0x3f62f0`) and offers `Videos`, `Downloads`
and `Documents` as shortcuts (`0x3f6790`).

The settings popover has Dark/Light, a Keep on top switch and six accent
swatches. Settings are restored from `%APPDATA%\vidsync\settings.json` on the
next launch; a missing file means dark, purple and not on top. The original
computed the same file as
`$XDG_CONFIG_HOME/vidsync/settings.json` (falling back to
`$HOME/.config/vidsync/settings.json`) in function `0x3f3ca0`.

## Interface, layout and themes

| Value | Setting | Source |
| --- | --- | --- |
| Window size | `1120x720` | function `0x3e9490` |
| Video viewport margin | `0 px`; flush to the window sides, header and status bar | user request |
| Top bar height | `52 px` normal, `44 px` compact | function `0x3e9490` |
| Status bar height | `70 px` normal, `58 px` compact | function `0x3e9490` |
| Status bar side padding | `18 px` normal, `12 px` compact | function `0x3e9490` |
| Timecode font | `32 px` normal, `22 px` compact, `18 px` narrow | function `0x3e9490` |
| Timecode area width | `280 px` normal, `200 px` compact, `160 px` narrow | function `0x3e9490` |
| Audacity label | `14 px` normal, `12 px` compact | function `0x3e9490` |
| Status dot | `4.5 px` radius, with a glow while following | function `0x3e9490` |
| Compact breakpoint | `880 px` window width | function `0x3e9490` |
| Narrowest layout breakpoint | `500 px` window width | function `0x3e9490` |
| Smallest tested window | `460x360` | estimated (build check, not an original constant) |
| Toast duration | `4.5 s` | function `0x3d1e10`; `.rodata 0x124d0` |
| File browser double-click window | `0.38 s` | function `0x3e9490`; `.rodata 0x12398` |
| File browser sidebar breakpoint | `540 px` | function `0x3e9490` |
| File browser footer | `62 px` normal, `52 px` compact | function `0x3e9490` |
| Theme choices | `orange, yellow, green, blue, purple, red` | user request |
| Default theme | `purple` | user request |
| Interface modes | `dark, light` | user request |
| Default interface mode | `dark` | user request |
| Settings panel | `238x168 px` popover under the gear | function `0x3e9490` |
| Settings swatch | `24 px`, spaced `34 px` apart | function `0x3e9490` |
| Keep-on-top switch | `40x20 px`, on the right of its row | function `0x3e9490` (panel geometry); user request |
| Keep-on-top row | full panel width, `28 px` high | function `0x3e9490` (panel geometry); user request |

The main layout and status bar are drawn by the UI routine at `0x3e9490`; the
settings popover and the colour/state choices are part of the same routine.
Below 880 px wide the bars get shorter, the timecode drops to 22 px, the
Audacity label drops to 12 px and side padding shrinks. Below 500 px the
timecode drops to 18 px and its area narrows to 160 px. The file browser drops
its sidebar below 540 px.

Choosing a swatch changes every accent immediately: the brand mark, play icon,
settings highlights and folder highlights. Dark is the default; every colour is
paired with a light-mode equivalent so contrast is preserved.

## Verification mode

`--screenshot <file.png>` renders the real interface at `1120x720` with the
bundled sample clip, drives the timecode to a moving position, and writes a PNG.
`--self-test` uses the same path to prove the decoder and the follow/hold rules
without an Audacity installation. Both are build checks, not user-facing
features.

## Sources and unknowns

- Function addresses refer to the stripped Linux x86_64 AppImage:
  `0x3e9490` UI/layout/status/settings/browser draw, `0x3e2630` ffmpeg decoder
  and stream setup, `0x3e7780` ffprobe, `0x3f0820` player tick/queue, `0x3d1e10`
  main/App flow and keyboard shortcuts, `0x3f7a10` Audacity poll loop,
  `0x3f6f40` timecode tree search, `0x3f6db0` accessible Name read,
  `0x4018a0` ChildCount read, `0x4019d0` DBus property read, `0x3f3ca0`
  settings path, `0x3f62f0` default folder, `0x3f6790` browser shortcuts,
  `0x3e19c0` player stop helper, and `0x411c20` the message/string helper.
- Floating-point literals were read from `.rodata` at `0x120e0` (`0.02`),
  `0x12238` (`0.4`), `0x12320` (`0.045`), `0x10eb0` (`1280`), `0x10eb8`
  (`720`), `0x120f0` (`30`), `0x12548` (`0.0005`), `0x124d0` (`4.5`),
  `0x12398` (`0.38`), `0x12438` (`1/30`) and `0x124f8` (`-0.001`).
- Decoder arguments (`-r 30`, `-an`, `-sn`, `-dn`, `-pix_fmt rgba`,
  `scale=...:flags=bicubic,setsar=1,format=rgba`) were read directly from
  function `0x3e2630`.
- Audacity 4.0.2 was observed on the build machine. Its accessible tree exposes
  `Playback position:00 h 00 m 00.000 s`, and the value advanced while Audacity
  played. A future release could change the label or clock format; the parser
  accepts labelled hour/minute/second text and a bare colon clock.
- Windows UI Automation is assumed to expose the same Qt accessibility tree.
  The port calls the COM vtables directly because `windows-sys` does not wrap
  them; the method ordinals follow the Windows SDK interface order
  (`IUIAutomation::CreateTrueCondition` is slot 14,
  `IUIAutomationElement::FindAll` is 6, `get_CurrentName` is 23,
  `get_CurrentPattern` is 16 and `get_CurrentIsOffscreen` is 33). The Windows
  path type-checks with `cargo check --target x86_64-pc-windows-gnu`, but it has
  not been run on Windows here, so the mapping stays unverified until it is
  exercised on that platform.
- UI Automation must be available. The player cannot install or repair a
  missing accessibility stack.
- Keep-on-top, `%APPDATA%` and the `.exe` sidecar search are port additions;
  they have no Linux original to cite and are marked as such above.
- The included `dist` executable was cross-linked with
  `x86_64-w64-mingw32-gcc` 13.2 and checked as a 64-bit PE32+ Windows GUI
  program. It has not been executed on a Windows machine here, so the
  UI Automation path still needs a real Windows run with Audacity 4.0.
- The ready-to-run zip bundles the official gyan.dev Windows ffmpeg 9.0.2
  "essentials" build (`ffmpeg.exe`, `ffprobe.exe` and its licence), so the
  player does not depend on a separate ffmpeg installation. The loose
  `dist/VIDSYNC-1.0.0-x86_64.exe` still searches `CUE_FFMPEG`, its own folder,
  a `tools` folder and `PATH` as described above.
