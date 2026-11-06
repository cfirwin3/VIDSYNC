# VIDSYNC mechanics

This sheet lists the values that shape VIDSYNC. There is no original program to
compare against, so design choices are marked **estimated**. Values taken from
the person's requests are marked with that source; values read from Audacity on
the build machine are marked **observed**. The same values live in
[`mechanics.json`](mechanics.json).

VIDSYNC is a video player, not a game. "Mechanics" here means following
Audacity 4.0's timecode, playback, the status bar and the small settings panel.

## Following Audacity

The person asked for a player with no transport of its own: Audacity 4.0's
timecode is the clock, the player is the slave, and the hierarchy of desktop
windows must not matter.

| Value | Setting | Source |
| --- | --- | --- |
| Timecode source | AT-SPI accessible name `Playback position:00 h 00 m 00.000 s`; any short name ending in a clock also works | observed (Audacity 4.0.2) / fallback |
| Timecode readout format | `HH:MM:SS.mmm`, e.g. `00:00:11.037` | user request |
| Required Audacity display | `00h00m00.000s` for a green dot | user request |
| Green dot | Audacity found and timecode uses the hour/minute/second display | user request |
| Red dot | Audacity not found, or its timecode uses another display | user request |
| Window order | irrelevant; no focus or key injection | user request |
| Timecode poll interval | `33 ms` | estimated |
| Movement window | `220 ms` | estimated |
| Movement threshold | `0.0005 s` | estimated |
| Timecode jump threshold | `0.4 s` (restart the decoder nearby) | estimated |
| Still-frame refresh threshold | `0.02 s` | estimated |
| Audacity rediscovery interval | `12 s` | estimated |
| Failed discovery retry | `1 s` | estimated |
| Audacity version observed | `4.0.2` | observed (build machine) |
| Display server | any Linux desktop with AT-SPI, X11 or Wayland | observed / estimated |

The background thread walks the accessibility tree once, caches the node whose
name starts with `Playback position`, and reads it about thirty times a second.
A value that changes within the movement window marks Audacity as playing. The
UI then either follows the clock or holds a still frame. Nothing is sent to
Audacity and no window is raised, so the two applications can be stacked in any
order.

When Audacity is playing, VIDSYNC follows each reading. A drift of more than
`0.4 s` restarts `ffmpeg` from the new position; smaller drift is absorbed by
the decoder already running. When the clock stops changing, the decoder is
stopped and a still frame is fetched for the held position. If Audacity jumps
backwards, or its stop action returns the play head to zero, the picture follows
that jump.

## Playback

| Value | Setting | Source |
| --- | --- | --- |
| Playback rate | `1.0x`, paced by the external timecode | user request |
| Output frame rate | `30 fps` | estimated |
| Maximum decoded width | `1280 px` | estimated |
| Maximum decoded height | `720 px` | estimated |
| Decoded frame queue | `12 frames` | estimated |
| Decoder channel buffer | `8 frames` | estimated |
| Longest accepted frame step | `0.1 s` | estimated |
| Audio output | none (audio streams are dropped) | user request |
| Supported formats | whatever the bundled `ffmpeg` can decode | user request |

Frames are decoded by `ffmpeg` to raw RGBA, scaled with bicubic filtering to fit
inside 1280x720 while preserving aspect ratio, and sampled at a fixed 30 fps.
The player displays the newest frame whose timestamp has been reached, so a slow
decode skips frames instead of slowing the follow. The image is letterboxed in
the window; it is never stretched.

`ffmpeg` decodes as fast as the machine allows, so the 12-frame queue keeps the
*earliest* frames and stops reading from the decoder while it is full. Draining
every available frame would throw away the pictures the clock is at and leave
only frames from seconds in the future. When Audacity runs past the end of a
short clip, the player parks on the last frame instead of restarting the
decoder on every timecode tick.

Audio, subtitles and data streams are explicitly dropped (`-an -sn -dn`). Video
codecs and containers are whatever the bundled/system `ffmpeg` can decode:
MP4, MKV, MOV, WebM, AVI, MPEG, TS, MTS, OGV, FLV, WMV and more.

## Controls and transport

The person asked to remove the start-time field, the Audacity on/off switch and
the play, pause and record buttons. Audacity is the only transport.

| Value | Setting | Source |
| --- | --- | --- |
| Play button | none | user request |
| Pause button | none | user request |
| Record button | none | user request |
| Start-time field | none | user request |
| Audacity toggle switch | none | user request |
| Open shortcut | `O` | estimated |
| Fullscreen shortcut | `F` | estimated |
| Fullscreen button | header, beside the gear | user request |
| Exit fullscreen | `Escape` | user request |
| Close settings | `Escape` | estimated |
| File loading | built-in browser only | user request |
| Drag and drop | none | user request |
| Empty-stage message | none; the stage stays black | user request |
| Sample-clip shortcut | none | user request |
| Keep-on-top setting | on/off switch in the settings panel | user request |
| Keep-on-top default | off | estimated |
| Keep-on-top persistence | saved with mode and accent | estimated |

The header keeps only the wordmark, the current file name, an Open button, a
fullscreen button and the gear. The body is the picture and the status bar. With
no file loaded the body is an empty black stage with no text, icon or button.
The status bar keeps the `Audacity` label, its green/red dot and the
`00:00:00.000` readout. Fullscreen removes the header, status bar and every
other overlay; only the picture and its black letterbox/pillarbox remain.

## Interface, layout and themes

| Value | Setting | Source |
| --- | --- | --- |
| Window size | `1120x720` | estimated |
| Video viewport margin | `0 px`; flush to the window sides, header and status bar | user request |
| Top bar height | `52 px` normal, `44 px` compact | estimated |
| Status bar height | `70 px` normal, `58 px` compact | estimated |
| Status bar side padding | `18 px` normal, `12 px` compact | estimated |
| Timecode font | `32 px` normal, `22 px` compact, `18 px` narrow | estimated |
| Timecode area width | `280 px` normal, `200 px` compact, `160 px` narrow | estimated |
| Audacity label | `14 px` normal, `12 px` compact | estimated |
| Status dot | `4.5 px` radius, with a glow while following | estimated |
| Compact breakpoint | `880 px` window width | estimated |
| Narrowest layout breakpoint | `500 px` window width | estimated |
| Smallest tested window | `460x360` | estimated |
| Toast duration | `4.5 s` | estimated |
| File browser double-click window | `0.38 s` | estimated |
| Theme choices | `orange, yellow, green, blue, purple, red` | user request |
| Default theme | `purple` | user request |
| Interface modes | `dark, light` | user request |
| Default interface mode | `dark` | user request |
| Settings panel | `238x168 px` popover under the gear | estimated |
| Settings panel swatch | `24 px`, spaced `34 px` apart | estimated |
| Keep-on-top switch | `40x20 px`, on the right of its row | estimated |
| Keep-on-top row | full panel width, `28 px` high | estimated |

The layout switches to a compact status bar below 880 px wide: the bars get
shorter, the timecode drops to 22 px, the Audacity label drops to 12 px and the
side padding shrinks. Below 500 px the timecode drops to 18 px and its area
narrows to 160 px. The built-in file browser also drops its sidebar below
540 px and uses a smaller footer.

The gear opens a settings popover rather than a separate window. A Dark / Light
choice switches the whole interface; dark is the default. Choosing a swatch
changes every accent immediately: the brand mark, play icon, settings
highlights and folder highlights. The Keep on top switch adds or removes
`_NET_WM_STATE_ABOVE` on the player window. All three choices are saved to
`$XDG_CONFIG_HOME/vidsync/settings.json` and restored on the next launch, so a
missing file simply means dark, purple and not on top.

## Sources and unknowns

- The latest request removes the empty-stage prompt, the sample-clip shortcut
  and drag-and-drop: videos load only from the built-in browser. The settings
  panel now also has a Keep on top switch, saved between launches.
- Earlier requests define the rest of this version: no transport buttons, no
  start-time field, no Audacity switch, a live `00:00:00.000` readout, a
  green/red dot for Audacity and its `00h00m00.000s` display, a minimalist slave
  player, and window-order independence.
- Audacity 4.0.2 was running on the build machine. Its accessible tree exposes
  the name `Playback position:00 h 00 m 00.000 s`, and the value advanced while
  Audacity played. VIDSYNC's live trace followed it and reset with Audacity's
  stop-to-zero.
- Audacity 4.0 on other Linux desktops, and any future Audacity version, could
  expose a different accessible name. VIDSYNC falls back to any short accessible
  name that ends in a clock, so translated labels work, but a future release
  could change the clock format itself.
- The accessibility bus must be running. VIDSYNC switches `IsEnabled` on, but
  cannot install a missing AT-SPI stack.
- Every numeric layout value is a design choice for this build, not a copied
  constant.
- Decode size and frame rate trade sharpness for CPU and memory; they can be
  changed in `src/player.rs` and kept in step with `mechanics.json`.
