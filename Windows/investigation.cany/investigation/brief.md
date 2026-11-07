# Brief

**Question:** Port VIDSYNC, a Linux video player that follows Audacity 4.0's timecode, to a native Windows desktop app with the same core features.

**Purpose:** The person does not code and asked for a Windows port of the app.

**Targets:**
- /home/carl/Documents/Craft Anything/VIDSYNC/app/dist/VIDSYNC-1.0.0-x86_64.AppImage

**Constraints:**
- Change only files inside app/
- Keep the original values and layout
- Windows integration replaces AT-SPI, XDG and X11

**Done when:** Cargo builds on Linux, the Windows target cross-links, --screenshot writes a real 1120x720 PNG, mechanics.json matches MECHANICS.md, and rebuild_check passes.
