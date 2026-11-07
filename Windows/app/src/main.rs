#![cfg_attr(windows, windows_subsystem = "windows")]

mod audacity;
mod browser;
mod player;
mod topmost;
mod ui;

use macroquad::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

use audacity::AudacityLink;
use browser::{Browser, BrowserAction};
use player::Player;
use ui::*;

const SAMPLE: &[u8] = include_bytes!("../assets/sample.mp4");
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/vidsync.png");

/// Scale the bundled app icon down to the three sizes the window manager asks
/// for. The icon is the same rounded tile and play mark drawn beside the name.
fn icon_at(source: &Image, size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    let sw = source.width as usize;
    let sh = source.height as usize;
    if sw == 0 || sh == 0 {
        return out;
    }
    for y in 0..size {
        let sy = ((y as f32 + 0.5) * sh as f32 / size as f32) as usize;
        let sy = sy.min(sh - 1);
        for x in 0..size {
            let sx = ((x as f32 + 0.5) * sw as f32 / size as f32) as usize;
            let sx = sx.min(sw - 1);
            let src = (sy * sw + sx) * 4;
            let dst = (y * size + x) * 4;
            if src + 3 < source.bytes.len() {
                out[dst..dst + 4].copy_from_slice(&source.bytes[src..src + 4]);
            }
        }
    }
    out
}

fn app_icon() -> Option<macroquad::miniquad::conf::Icon> {
    let image = Image::from_file_with_format(APP_ICON_PNG, Some(ImageFormat::Png)).ok()?;
    let small: [u8; 16 * 16 * 4] = icon_at(&image, 16).try_into().ok()?;
    let medium: [u8; 32 * 32 * 4] = icon_at(&image, 32).try_into().ok()?;
    let big: [u8; 64 * 64 * 4] = icon_at(&image, 64).try_into().ok()?;
    Some(macroquad::miniquad::conf::Icon { small, medium, big })
}

fn window_conf() -> Conf {
    let (width, height) = std::env::var("VIDSYNC_WINDOW")
        .or_else(|_| std::env::var("VIDCUE_WINDOW"))
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?))
        })
        .unwrap_or((1120, 720));
    Conf {
        window_title: "VIDSYNC".to_owned(),
        window_width: width,
        window_height: height,
        window_resizable: true,
        high_dpi: true,
        sample_count: 4,
        icon: app_icon(),
        platform: macroquad::miniquad::conf::Platform {
            linux_wm_class: "vidsync",
            ..Default::default()
        },
        ..Default::default()
    }
}

struct Layout {
    top: Rect,
    video: Rect,
    status: Rect,
    open_btn: Rect,
    fullscreen_btn: Rect,
    settings_btn: Rect,
    settings_panel: Rect,
    mode_dark: Rect,
    mode_light: Rect,
    window_row: Rect,
    window_toggle: Rect,
    swatches: [Rect; 6],
    time_area: Rect,
    audacity: Rect,
    compact: bool,
    ultra: bool,
}

impl Layout {
    fn new(w: f32, h: f32, _has_media: bool) -> Self {
        let compact = w < 880.0;
        let ultra = w < 500.0;
        let margin = if compact { 10.0 } else { 16.0 };
        let top_h = if compact { 44.0 } else { 52.0 };
        let top = Rect::new(0.0, 0.0, w, top_h);

        // The picture runs edge-to-edge between the header and the status bar.
        // Aspect ratio is protected by the black stage around the texture, not
        // by insetting the viewport itself.
        let status_h = if compact { 58.0 } else { 70.0 };
        let status = Rect::new(0.0, (h - status_h).max(top_h), w, status_h);
        let video = Rect::new(0.0, top_h, w, (status.y - top_h).max(1.0));

        let gear_s = if compact { 28.0 } else { 32.0 };
        let settings_btn = Rect::new(w - margin - gear_s, (top_h - gear_s) / 2.0, gear_s, gear_s);
        let fullscreen_btn = Rect::new(
            settings_btn.x - 8.0 - gear_s,
            (top_h - gear_s) / 2.0,
            gear_s,
            gear_s,
        );
        let open_w = if compact { 94.0 } else { 112.0 };
        let open_h = if compact { 26.0 } else { 30.0 };
        let open_btn = Rect::new(
            fullscreen_btn.x - 8.0 - open_w,
            (top_h - open_h) / 2.0,
            open_w,
            open_h,
        );

        let panel_w = 238.0;
        let panel_h = 168.0;
        let panel_x = (settings_btn.x + gear_s / 2.0 - panel_w / 2.0)
            .clamp(margin, (w - margin - panel_w).max(margin));
        let settings_panel = Rect::new(panel_x, top_h + 6.0, panel_w, panel_h);
        let mode_w = 58.0;
        let mode_h = 24.0;
        let mode_y = settings_panel.y + 30.0;
        let mode_light = Rect::new(
            settings_panel.x + panel_w - 16.0 - mode_w,
            mode_y,
            mode_w,
            mode_h,
        );
        let mode_dark = Rect::new(mode_light.x - 8.0 - mode_w, mode_y, mode_w, mode_h);
        let window_row = Rect::new(
            settings_panel.x + 12.0,
            settings_panel.y + 70.0,
            panel_w - 24.0,
            28.0,
        );
        let window_toggle = Rect::new(
            settings_panel.x + panel_w - 16.0 - 40.0,
            window_row.y + 4.0,
            40.0,
            20.0,
        );
        let mut swatches = [Rect::new(0.0, 0.0, 0.0, 0.0); 6];
        for (i, s) in swatches.iter_mut().enumerate() {
            *s = Rect::new(
                panel_x + 16.0 + i as f32 * 34.0,
                settings_panel.y + 132.0,
                24.0,
                24.0,
            );
        }

        let status_pad = if compact { 12.0 } else { 18.0 };
        let tc_size = if ultra {
            18.0
        } else if compact {
            22.0
        } else {
            32.0
        };
        let tc_h = tc_size + 2.0;
        let time_w = if ultra {
            160.0
        } else if compact {
            200.0
        } else {
            280.0
        };
        let time_area = Rect::new(
            status.x + status.w - status_pad - time_w,
            status.y + (status.h - tc_h) / 2.0,
            time_w,
            tc_h,
        );
        let audacity = Rect::new(
            status.x + status_pad,
            status.y,
            (time_area.x - status.x - status_pad * 2.0).max(100.0),
            status.h,
        );

        Layout {
            top,
            video,
            status,
            open_btn,
            fullscreen_btn,
            settings_btn,
            settings_panel,
            mode_dark,
            mode_light,
            window_row,
            window_toggle,
            swatches,
            time_area,
            audacity,
            compact,
            ultra,
        }
    }
}

struct Toast {
    text: String,
    until: f64,
    error: bool,
}

struct App {
    fonts: Fonts,
    player: Player,
    browser: Browser,
    sync: AudacityLink,
    now: f64,
    toast: Option<Toast>,
    fullscreen: bool,
    settings_open: bool,
    timecode: f64,
    timecode_live: bool,
    timecode_ready: bool,
    timecode_moving: bool,
    timecode_note: String,
    simulated: bool,
}

impl App {
    fn new() -> Self {
        App {
            fonts: Fonts::load(),
            player: Player::new(),
            browser: Browser::new(),
            sync: AudacityLink::new(),
            now: 0.0,
            toast: None,
            fullscreen: false,
            settings_open: false,
            timecode: 0.0,
            timecode_live: false,
            timecode_ready: false,
            timecode_moving: false,
            timecode_note: "looking for Audacity…".to_string(),
            simulated: false,
        }
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toast = Some(Toast {
            text: text.into(),
            until: self.now + 4.5,
            error,
        });
    }

    fn tick(&mut self, dt: f64) {
        self.now += dt;
        self.player.update(dt);
        if !self.simulated {
            let snapshot = self.sync.snapshot();
            self.consume_snapshot(&snapshot);
        }
        if let Some(t) = &self.toast {
            if self.now > t.until {
                self.toast = None;
            }
        }
    }

    /// Take the latest AT-SPI reading and make the picture follow it. Audacity's
    /// timecode is the only clock here, so the video has no transport of its
    /// own: moving timecode scrolls the picture, still timecode freezes it.
    fn consume_snapshot(&mut self, snapshot: &audacity::Snapshot) {
        self.timecode_live = snapshot.connected;
        self.timecode_ready = snapshot.connected && snapshot.format_ok;
        self.timecode_moving = snapshot.connected && snapshot.moving;
        if !snapshot.message.is_empty() {
            self.timecode_note = snapshot.message.clone();
        }
        if let Some(position) = snapshot.position {
            self.timecode = position;
        }

        if self.player.media.is_none() {
            return;
        }
        if snapshot.connected {
            if let Some(position) = snapshot.position {
                if snapshot.moving {
                    self.player.follow(position);
                } else {
                    self.player.hold(position);
                }
            }
        } else {
            // Keep the last picture on screen while Audacity is unavailable.
            self.player.hold(self.timecode);
        }
    }

    /// Used by the screenshot and self-test modes to stand in for Audacity.
    fn drive_timecode(&mut self, seconds: f64, moving: bool) {
        self.simulated = true;
        self.timecode_live = true;
        self.timecode_ready = true;
        self.timecode_moving = moving;
        self.timecode = seconds.max(0.0);
        self.timecode_note = "timecode live".to_string();
        if self.player.media.is_none() {
            return;
        }
        if moving {
            self.player.follow(self.timecode);
        } else {
            self.player.hold(self.timecode);
        }
    }

    fn timecode_text(&self) -> String {
        fmt_time(self.timecode, true)
    }

    fn open_browser(&mut self) {
        self.settings_open = false;
        let dir = self
            .player
            .media
            .as_ref()
            .and_then(|m| m.path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(browser::default_dir);
        self.browser.open_at(dir);
        self.browser.open = true;
    }

    fn load_media(&mut self, path: PathBuf) {
        match self.player.open(&path) {
            Ok(()) => {
                self.browser.open = false;
                if self.timecode_live {
                    self.player.hold(self.timecode);
                }
                let name = self
                    .player
                    .media
                    .as_ref()
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                self.toast(format!("Loaded {name}"), false);
            }
            Err(e) => {
                self.toast(e, true);
            }
        }
    }

    fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.settings_open = false;
        set_fullscreen(self.fullscreen);
        // Fullscreen is picture-only, so the pointer goes away too.
        show_mouse(!self.fullscreen);
    }

    fn handle_input(&mut self) {
        if self.browser.open {
            match self.browser.handle_keys() {
                BrowserAction::None => {}
                BrowserAction::Cancel => self.browser.open = false,
                BrowserAction::Open(p) => self.load_media(p),
            }
            return;
        }
        if self.fullscreen {
            if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::F) {
                self.toggle_fullscreen();
            }
            return;
        }
        if is_key_pressed(KeyCode::Escape) {
            self.settings_open = false;
        }
        if is_key_pressed(KeyCode::O) {
            self.open_browser();
        }
        if is_key_pressed(KeyCode::F) {
            self.toggle_fullscreen();
        }
    }

    fn handle_mouse(&mut self) {
        // Fullscreen hides every control, so there is nothing to click.
        if self.fullscreen {
            return;
        }

        let has_media = self.player.media.is_some();
        let l = Layout::new(screen_width(), screen_height(), has_media);

        if self.browser.open {
            let bl = browser::layout(screen_width(), screen_height());
            match self.browser.handle_mouse(&bl, self.now) {
                BrowserAction::None => {}
                BrowserAction::Cancel => self.browser.open = false,
                BrowserAction::Open(p) => self.load_media(p),
            }
            return;
        }

        // Settings popover sits above the rest of the interface.
        if clicked(l.settings_btn) {
            self.settings_open = !self.settings_open;
            return;
        }
        if self.settings_open {
            if is_mouse_button_pressed(MouseButton::Left) {
                if mouse_in(l.window_row) || mouse_in(l.window_toggle) {
                    let next = !keep_on_top();
                    set_keep_on_top(next);
                    topmost::apply(next);
                    save_settings();
                    return;
                }
                if mouse_in(l.mode_dark) {
                    set_light_mode(false);
                    save_settings();
                    return;
                }
                if mouse_in(l.mode_light) {
                    set_light_mode(true);
                    save_settings();
                    return;
                }
                for (i, swatch) in l.swatches.iter().enumerate() {
                    if mouse_in(*swatch) {
                        set_theme(THEMES[i]);
                        save_settings();
                        self.settings_open = false;
                        return;
                    }
                }
                if !mouse_in(l.settings_panel) {
                    self.settings_open = false;
                }
            }
            return;
        }

        if clicked(l.fullscreen_btn) {
            self.toggle_fullscreen();
            return;
        }

        if clicked(l.open_btn) {
            self.open_browser();
        }
    }

    fn draw(&self) {
        let w = screen_width();
        let h = screen_height();
        if self.fullscreen {
            self.draw_fullscreen(w, h);
            return;
        }
        let l = Layout::new(w, h, self.player.media.is_some());
        clear_background(bg());

        // Soft accent glow in the top-left corner.
        let glow = accent();
        let glow_alpha = if light_mode() { 0.02 } else { 0.006 };
        for i in 0..10 {
            let r = 210.0 - i as f32 * 18.0;
            draw_circle(
                70.0,
                42.0,
                r,
                Color::new(glow.r, glow.g, glow.b, glow_alpha),
            );
        }

        self.draw_top(&l);
        self.draw_video(&l);
        self.draw_status(&l);
        if self.settings_open {
            self.draw_settings(&l);
        }

        if let Some(toast) = &self.toast {
            let alpha = ((toast.until - self.now) / 0.5).clamp(0.0, 1.0) as f32;
            let tw = (text_width(&self.fonts.ui, &toast.text, 13) + 46.0).min(860.0);
            let tr = Rect::new((w - tw) / 2.0, l.status.y - 50.0, tw, 38.0);
            rounded_rect(tr.x, tr.y, tr.w, tr.h, 10.0, with_alpha(panel2(), alpha));
            draw_rectangle(
                tr.x + 1.0,
                tr.y + 8.0,
                3.0,
                tr.h - 16.0,
                with_alpha(if toast.error { signal_red() } else { accent() }, alpha),
            );
            let msg = ellipsize(&self.fonts.ui, &toast.text, 13, tr.w - 42.0);
            text_left(
                &self.fonts.ui,
                &msg,
                tr.x + 18.0,
                tr.y + 12.0,
                13,
                with_alpha(text_color(), alpha),
            );
        }

        if self.browser.open {
            let bl = browser::layout(w, h);
            self.browser.draw(&bl, &self.fonts);
        }
    }

    /// Fullscreen is the picture and nothing else. The screen itself is the
    /// black stage; the texture is fitted inside it so the aspect ratio is
    /// never stretched.
    fn draw_fullscreen(&self, w: f32, h: f32) {
        clear_background(BLACK);
        if let Some(tex) = self.player.texture() {
            let ar = self.player.aspect();
            let mut dw = w;
            let mut dh = dw / ar;
            if dh > h {
                dh = h;
                dw = dh * ar;
            }
            draw_texture_ex(
                tex,
                (w - dw) / 2.0,
                (h - dh) / 2.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(dw, dh)),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_top(&self, l: &Layout) {
        draw_line(0.0, l.top.h, screen_width(), l.top.h, 1.0, line_color());
        let margin = if l.compact { 10.0 } else { 16.0 };
        let brand_size = if l.compact { 15 } else { 17 };
        let icon_s = brand_size as f32 + 3.0;
        let icon_y = (l.top.h - icon_s) / 2.0;
        rounded_rect(margin, icon_y, icon_s, icon_s, 6.0, accent_soft());
        play_icon(
            margin + icon_s / 2.0,
            icon_y + icon_s / 2.0,
            icon_s * 0.44,
            accent(),
        );

        let tx = margin + icon_s + 8.0;
        let brand_y = icon_y + (icon_s - brand_size as f32) / 2.0;
        let vid_w = text_width(&self.fonts.bold, "VID", brand_size);
        text_left(
            &self.fonts.bold,
            "VID",
            tx,
            brand_y,
            brand_size,
            text_color(),
        );
        text_left(
            &self.fonts.bold,
            "SYNC",
            tx + vid_w + 1.0,
            brand_y,
            brand_size,
            accent(),
        );

        let right = l.open_btn.x - 12.0;
        if !l.compact {
            if let Some(m) = &self.player.media {
                let folder = m
                    .path
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let room = (right - tx - 210.0).max(80.0);
                let name = ellipsize(&self.fonts.bold, &m.name, 13, room);
                text_right(&self.fonts.bold, &name, right, 8.0, 13, text_color());
                let path = ellipsize(&self.fonts.ui, &folder, 10, room);
                text_right(&self.fonts.ui, &path, right, 28.0, 10, dim());
            } else {
                text_right(&self.fonts.ui, "No video loaded", right, 17.0, 12, muted());
            }
        }

        // Fullscreen button.
        let fs_hot = mouse_in(l.fullscreen_btn);
        if fs_hot {
            rounded_rect(
                l.fullscreen_btn.x,
                l.fullscreen_btn.y,
                l.fullscreen_btn.w,
                l.fullscreen_btn.h,
                8.0,
                panel2(),
            );
        }
        fullscreen_icon(
            l.fullscreen_btn.x + l.fullscreen_btn.w / 2.0,
            l.fullscreen_btn.y + l.fullscreen_btn.h / 2.0,
            l.fullscreen_btn.w * 0.26,
            if fs_hot { text_color() } else { muted() },
        );

        // Settings gear.
        let hot = mouse_in(l.settings_btn);
        if self.settings_open || hot {
            rounded_rect(
                l.settings_btn.x,
                l.settings_btn.y,
                l.settings_btn.w,
                l.settings_btn.h,
                8.0,
                if self.settings_open {
                    panel3()
                } else {
                    panel2()
                },
            );
        }
        gear_icon(
            l.settings_btn.x + l.settings_btn.w / 2.0,
            l.settings_btn.y + l.settings_btn.h / 2.0,
            l.settings_btn.w * 0.27,
            if self.settings_open {
                accent_bright()
            } else if hot {
                text_color()
            } else {
                muted()
            },
        );

        button(&self.fonts, l.open_btn, "Open video", BtnStyle::Ghost, true);
    }

    fn draw_settings(&self, l: &Layout) {
        let p = l.settings_panel;
        rounded_rect(
            p.x + 2.0,
            p.y + 4.0,
            p.w,
            p.h,
            12.0,
            Color::new(0.0, 0.0, 0.0, if light_mode() { 0.14 } else { 0.35 }),
        );
        rounded_rect(p.x, p.y, p.w, p.h, 12.0, panel2());
        draw_line(p.x + 12.0, p.y, p.x + p.w - 12.0, p.y, 1.0, line_color());
        text_left(
            &self.fonts.bold,
            "SETTINGS",
            p.x + 16.0,
            p.y + 10.0,
            9,
            muted(),
        );
        text_left(&self.fonts.bold, "MODE", p.x + 16.0, p.y + 40.0, 9, muted());

        // Dark / light background choice. Dark stays the default.
        for (rect, label, selected) in [
            (l.mode_dark, "Dark", !light_mode()),
            (l.mode_light, "Light", light_mode()),
        ] {
            let hot = mouse_in(rect);
            let (fill, ink) = if selected {
                (accent(), accent_ink())
            } else if hot {
                (panel3(), text_color())
            } else {
                (panel3(), muted())
            };
            rounded_rect(rect.x, rect.y, rect.w, rect.h, 7.0, fill);
            if hot && !selected {
                draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.2, accent());
            }
            text_in_rect(&self.fonts.bold, label, rect, 10, ink);
        }

        draw_line(
            p.x + 12.0,
            p.y + 64.0,
            p.x + p.w - 12.0,
            p.y + 64.0,
            1.0,
            line_color(),
        );

        // Keep-on-top switch. Clicking anywhere on the row flips it.
        let row_hot = mouse_in(l.window_row) || mouse_in(l.window_toggle);
        if row_hot {
            rounded_rect(
                l.window_row.x,
                l.window_row.y,
                l.window_row.w,
                l.window_row.h,
                7.0,
                panel3(),
            );
        }
        text_left(
            &self.fonts.bold,
            "Keep on top",
            p.x + 18.0,
            p.y + 78.0,
            11,
            if keep_on_top() { text_color() } else { muted() },
        );
        toggle_switch(l.window_toggle, keep_on_top(), row_hot);

        draw_line(
            p.x + 12.0,
            p.y + 102.0,
            p.x + p.w - 12.0,
            p.y + 102.0,
            1.0,
            line_color(),
        );
        text_left(
            &self.fonts.bold,
            "ACCENT COLOUR",
            p.x + 16.0,
            p.y + 110.0,
            9,
            muted(),
        );
        text_right(
            &self.fonts.ui,
            theme().label(),
            p.x + p.w - 16.0,
            p.y + 110.0,
            10,
            accent_bright(),
        );
        for (i, swatch) in l.swatches.iter().enumerate() {
            let t = THEMES[i];
            let c = t.accent();
            let cx = swatch.x + swatch.w / 2.0;
            let cy = swatch.y + swatch.h / 2.0;
            let selected = t == theme();
            if selected {
                draw_circle(cx, cy, 14.0, accent_soft());
                draw_circle_lines(cx, cy, 13.0, 1.2, accent_bright());
            }
            draw_circle(cx, cy, 9.0, c);
            if mouse_in(*swatch) && !selected {
                draw_circle_lines(cx, cy, 11.0, 1.4, text_color());
            }
        }
    }

    fn draw_video(&self, l: &Layout) {
        // The stage reaches the window edges and the header/status bars. Any
        // letterbox or pillarbox is black, so the picture keeps its ratio.
        draw_rectangle(l.video.x, l.video.y, l.video.w, l.video.h, BLACK);

        if self.player.media.is_some() {
            let max_w = l.video.w;
            let max_h = l.video.h;
            let ar = self.player.aspect();
            let mut dw = max_w;
            let mut dh = dw / ar;
            if dh > max_h {
                dh = max_h;
                dw = dh * ar;
            }
            let dx = l.video.x + (l.video.w - dw) / 2.0;
            let dy = l.video.y + (l.video.h - dh) / 2.0;
            if let Some(tex) = self.player.texture() {
                draw_texture_ex(
                    tex,
                    dx,
                    dy,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(dw, dh)),
                        ..Default::default()
                    },
                );
            } else {
                // Waiting for the first frame.
                let cx = l.video.x + l.video.w / 2.0;
                let cy = l.video.y + l.video.h / 2.0;
                for i in 0..3 {
                    let phase = (self.now * 2.4 + i as f64 * 0.5) as f32;
                    let a = 0.25 + 0.65 * (phase.sin() * 0.5 + 0.5);
                    draw_circle(
                        cx - 18.0 + i as f32 * 18.0,
                        cy,
                        3.0,
                        Color::new(accent().r, accent().g, accent().b, a),
                    );
                }
                text_center(
                    &self.fonts.ui,
                    "Reading the first frame…",
                    cx,
                    cy + 18.0,
                    12,
                    muted(),
                );
            }
        }
        // With no file loaded the stage stays empty and black. Videos are
        // opened only from the header's browser, so nothing covers the picture.
    }

    fn draw_status(&self, l: &Layout) {
        let p = l.status;
        draw_rectangle(p.x, p.y, p.w, p.h, panel());
        draw_line(p.x, p.y, p.x + p.w, p.y, 1.0, line_color());

        let dot_color = if self.timecode_ready {
            signal_green()
        } else {
            signal_red()
        };
        let label_size = if l.compact { 12 } else { 14 };
        let sub_size = if l.compact { 9 } else { 10 };
        let block_h = label_size as f32 + sub_size as f32 + 4.0;
        let block_y = l.audacity.y + (l.audacity.h - block_h) / 2.0;
        let dot_cx = l.audacity.x + 5.0;
        let dot_cy = block_y + label_size as f32 * 0.55;
        if self.timecode_ready && self.timecode_moving {
            draw_circle(dot_cx, dot_cy, 8.0, with_alpha(signal_green(), 0.18));
            draw_circle(dot_cx, dot_cy, 5.5, with_alpha(signal_green(), 0.30));
        }
        dot(dot_cx, dot_cy, 4.5, dot_color);

        let label_x = l.audacity.x + 16.0;
        text_left(
            &self.fonts.bold,
            "Audacity",
            label_x,
            block_y,
            label_size,
            text_color(),
        );

        let note = if !self.timecode_live {
            self.timecode_note.as_str()
        } else if !self.timecode_ready {
            "Set timecode to 00h00m00.000s"
        } else if self.timecode_moving {
            "Following Audacity"
        } else {
            "Audacity paused"
        };
        text_left(
            &self.fonts.ui,
            &ellipsize(
                &self.fonts.ui,
                note,
                sub_size,
                (l.audacity.w - 22.0).max(60.0),
            ),
            label_x,
            block_y + label_size as f32 + 4.0,
            sub_size,
            muted(),
        );

        let time_size = if l.ultra {
            18
        } else if l.compact {
            22
        } else {
            32
        };
        let text_color_value = if self.timecode_ready {
            accent_bright()
        } else {
            muted()
        };
        text_right(
            &self.fonts.mono_bold,
            &self.timecode_text(),
            l.time_area.x + l.time_area.w,
            l.time_area.y,
            time_size,
            text_color_value,
        );
    }
}

fn ensure_sample() -> PathBuf {
    let path = std::env::temp_dir().join("vidsync-sample.mp4");
    let needs_write = fs::metadata(&path)
        .map(|m| m.len() != SAMPLE.len() as u64)
        .unwrap_or(true);
    if needs_write {
        let _ = fs::write(&path, SAMPLE);
    }
    path
}

async fn run_self_test(app: &mut App, out: &Path) {
    let mut results: Vec<(String, bool, String)> = Vec::new();
    let sample = ensure_sample();
    app.load_media(sample);
    // The self-test drives its own clock; do not let a running Audacity
    // replace the first preview while the sample is loading.
    app.simulated = true;

    // Wait until the first frame is on screen. Use wall-clock time rather than
    // a frame count: a release build can run hundreds of empty frames before
    // the preview thread has finished starting ffmpeg.
    let mut frame_ok = false;
    let started = std::time::Instant::now();
    while started.elapsed().as_secs_f64() < 4.0 {
        app.tick(1.0 / 60.0);
        app.draw();
        if app.player.has_frame {
            frame_ok = true;
            break;
        }
        next_frame().await;
    }

    // move: a rising Audacity timecode scrolls the picture.
    let start_pos = 2.0;
    for i in 0..90 {
        let target = start_pos + i as f64 / 60.0;
        app.drive_timecode(target, true);
        app.tick(1.0 / 60.0);
        app.draw();
        next_frame().await;
    }
    let moved = app.player.position - start_pos;
    results.push((
        "move".to_string(),
        frame_ok && app.player.has_frame && moved > 1.0,
        format!(
            "simulated Audacity timecode advanced the picture {:.2} s with frames visible",
            moved
        ),
    ));

    // act: the picture follows a still clock, resumes, then freezes again.
    app.drive_timecode(4.0, false);
    for _ in 0..12 {
        app.tick(1.0 / 60.0);
        app.draw();
        next_frame().await;
    }
    let held = !app.player.playing && (app.player.position - 4.0).abs() < 0.02;
    let millis_shown = app.timecode_text() == "00:00:04.000";
    let p0 = app.player.position;
    for i in 0..45 {
        let target = 4.0 + i as f64 / 60.0;
        app.drive_timecode(target, true);
        app.tick(1.0 / 60.0);
        app.draw();
        next_frame().await;
    }
    let advanced = app.player.position - p0;
    app.drive_timecode(4.75, false);
    let frozen_from = app.player.position;
    for _ in 0..10 {
        app.tick(1.0 / 60.0);
        app.draw();
        next_frame().await;
    }
    let frozen = !app.player.playing && (app.player.position - frozen_from).abs() < 0.02;
    results.push((
        "act".to_string(),
        held && millis_shown && advanced > 0.4 && frozen,
        format!(
            "00:00:04.000 held the picture; movement advanced {:.2} s and stopping froze it again",
            advanced
        ),
    ));

    // hud: the timecode readout and the green Audacity status dot are painted.
    app.drive_timecode(4.75, false);
    app.tick(1.0 / 60.0);
    app.draw();
    let img = get_screen_data();
    let l = Layout::new(screen_width(), screen_height(), true);
    let scale = img.width as f32 / screen_width();
    let tc_px = count_pixels(&img, l.time_area, scale, |r, g, b| {
        r > 110 && g > 80 && b > 130
    });
    let dot_px = count_pixels(&img, l.audacity, scale, |r, g, b| {
        g > 140 && r < 170 && b < 190 && g > r + 20
    });
    results.push((
        "hud".to_string(),
        tc_px > 80 && dot_px > 4,
        format!("timecode readout ({tc_px} px) and green Audacity dot ({dot_px} px) are drawn"),
    ));

    let steps: Vec<serde_json::Value> = results
        .iter()
        .map(|(step, passed, detail)| {
            serde_json::json!({
                "step": step,
                "passed": passed,
                "detail": detail,
            })
        })
        .collect();
    let json = serde_json::json!({
        "version": 1,
        "steps": steps,
    });
    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(out, serde_json::to_string_pretty(&json).unwrap_or_default());
    next_frame().await;
}

fn count_pixels<F>(img: &Image, rect: Rect, scale: f32, f: F) -> usize
where
    F: Fn(u8, u8, u8) -> bool,
{
    let iw = img.width as usize;
    let ih = img.height as usize;
    if iw == 0 || ih == 0 {
        return 0;
    }
    let x0 = (rect.x * scale).max(0.0) as usize;
    let x1 = ((rect.x + rect.w) * scale).min(iw as f32) as usize;
    let y0 = (rect.y * scale).max(0.0) as usize;
    let y1 = ((rect.y + rect.h) * scale).min(ih as f32) as usize;
    let mut count = 0;
    for sy in y0..y1 {
        let iy = ih - 1 - sy;
        for x in x0..x1 {
            let idx = (iy * iw + x) * 4;
            if idx + 3 < img.bytes.len()
                && f(img.bytes[idx], img.bytes[idx + 1], img.bytes[idx + 2])
            {
                count += 1;
            }
        }
    }
    count
}

#[macroquad::main(window_conf)]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<String> = None;
    let mut selftest: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" => {
                i += 1;
                if i < args.len() {
                    screenshot = Some(args[i].clone());
                }
            }
            "--self-test" => {
                i += 1;
                if i < args.len() {
                    selftest = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    let mut app = App::new();
    load_settings();
    // Restore the keep-on-top choice once the window exists.
    topmost::apply(keep_on_top());

    if let Some(out) = screenshot {
        let sample = ensure_sample();
        app.load_media(sample);
        app.toast = None;
        for frame in 0..120 {
            app.drive_timecode(11.037 + frame as f64 / 60.0, true);
            app.tick(1.0 / 60.0);
            app.draw();
            if frame == 119 {
                let img = get_screen_data();
                if let Some(parent) = Path::new(&out).parent() {
                    let _ = fs::create_dir_all(parent);
                }
                img.export_png(&out);
            }
            next_frame().await;
        }
        return;
    }

    if let Some(out) = selftest {
        run_self_test(&mut app, Path::new(&out)).await;
        return;
    }

    loop {
        app.handle_mouse();
        app.handle_input();
        app.tick(get_frame_time().min(0.1) as f64);
        app.draw();
        next_frame().await;
    }
}
