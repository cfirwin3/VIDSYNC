use crate::ui::*;
use macroquad::prelude::*;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ROW: f32 = 34.0;

const VIDEO_EXTS: &[&str] = &[
    "mp4", "m4v", "mkv", "mov", "webm", "avi", "mpg", "mpeg", "mts", "m2ts", "ts", "vob", "ogv",
    "ogm", "flv", "wmv", "asf", "3gp", "3g2", "mxf", "dv", "gif", "apng", "y4m", "rm", "rmvb",
    "divx", "f4v", "nut", "m1v", "m2v",
];

pub fn is_video(path: &Path) -> bool {
    path.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            VIDEO_EXTS.contains(&e.as_str())
        })
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn home_dir() -> PathBuf {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| {
            let drive = env::var_os("HOMEDRIVE");
            let path = env::var_os("HOMEPATH");
            match (drive, path) {
                (Some(d), Some(p)) => Some(PathBuf::from(d).join(p)),
                _ => None,
            }
        })
        .unwrap_or_else(|| PathBuf::from("C:\\"))
}

#[cfg(not(windows))]
pub fn home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

pub fn default_dir() -> PathBuf {
    let home = home_dir();
    for name in ["Videos", "Downloads", "Desktop", "Documents"] {
        let p = home.join(name);
        if p.is_dir() {
            return p;
        }
    }
    home
}

pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_video: bool,
    pub size: u64,
}

pub struct BrowserLayout {
    pub modal: Rect,
    pub close: Rect,
    pub path_row: Rect,
    pub up: Rect,
    pub sidebar: Rect,
    pub list: Rect,
    pub footer: Rect,
    pub cancel: Rect,
    pub open: Rect,
}

pub fn layout(sw: f32, sh: f32) -> BrowserLayout {
    let compact = sw < 780.0 || sh < 560.0;
    let pad = if compact { 12.0 } else { 22.0 };
    let w = (sw - if compact { 20.0 } else { 80.0 }).clamp(300.0, 900.0);
    let h = (sh - if compact { 20.0 } else { 80.0 }).clamp(280.0, 640.0);
    let modal = Rect::new((sw - w) / 2.0, (sh - h) / 2.0, w, h);
    let close_s = if compact { 26.0 } else { 30.0 };
    let close = Rect::new(
        modal.x + w - pad - close_s,
        modal.y + if compact { 12.0 } else { 16.0 },
        close_s,
        close_s,
    );
    let path_row = Rect::new(
        modal.x + pad,
        modal.y + if compact { 46.0 } else { 58.0 },
        w - pad * 2.0,
        if compact { 28.0 } else { 34.0 },
    );
    let up = Rect::new(
        path_row.x,
        path_row.y,
        if compact { 42.0 } else { 52.0 },
        path_row.h,
    );
    let content_y = path_row.y + path_row.h + 10.0;
    let footer_h = if compact { 52.0 } else { 62.0 };
    let content_h = h - (content_y - modal.y) - footer_h - 8.0;
    let sidebar_w = if w < 540.0 {
        0.0
    } else if compact {
        130.0
    } else {
        176.0
    };
    let sidebar = Rect::new(modal.x + pad, content_y, sidebar_w, content_h);
    let list_x = if sidebar_w > 0.0 {
        sidebar.x + sidebar.w + 12.0
    } else {
        modal.x + pad
    };
    let list = Rect::new(list_x, content_y, modal.x + w - pad - list_x, content_h);
    let footer = Rect::new(
        modal.x + pad,
        modal.y + h - footer_h,
        w - pad * 2.0,
        footer_h,
    );
    let btn_h = if compact { 34.0 } else { 40.0 };
    let btn_y = footer.y + (footer_h - btn_h) / 2.0 + 2.0;
    BrowserLayout {
        modal,
        close,
        path_row,
        up,
        sidebar,
        list,
        footer,
        cancel: Rect::new(
            footer.x + footer.w - 190.0,
            btn_y,
            if compact { 82.0 } else { 96.0 },
            btn_h,
        ),
        open: Rect::new(
            footer.x + footer.w - 100.0,
            btn_y,
            if compact { 96.0 } else { 116.0 },
            btn_h,
        ),
    }
}

pub enum BrowserAction {
    None,
    Cancel,
    Open(PathBuf),
}

pub struct Browser {
    pub open: bool,
    pub dir: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: Option<usize>,
    pub scroll: usize,
    pub error: Option<String>,
    pub last_click: (Option<usize>, f64),
}

impl Browser {
    pub fn new() -> Self {
        let dir = default_dir();
        let mut b = Browser {
            open: false,
            dir: dir.clone(),
            entries: Vec::new(),
            selected: None,
            scroll: 0,
            error: None,
            last_click: (None, -10.0),
        };
        b.open_at(dir);
        b
    }

    pub fn open_at(&mut self, path: PathBuf) {
        match fs::read_dir(&path) {
            Ok(rd) => {
                let mut entries = Vec::new();
                for item in rd.flatten() {
                    let p = item.path();
                    let name = item.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') {
                        continue;
                    }
                    let md = fs::metadata(&p).ok();
                    let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                    let size = md.as_ref().map(|m| m.len()).unwrap_or(0);
                    entries.push(Entry {
                        name,
                        path: p.clone(),
                        is_dir,
                        is_video: !is_dir && is_video(&p),
                        size,
                    });
                }
                entries.sort_by(|a, b| {
                    let ka = (!a.is_dir, !a.is_video, a.name.to_lowercase());
                    let kb = (!b.is_dir, !b.is_video, b.name.to_lowercase());
                    ka.cmp(&kb)
                });
                self.entries = entries;
                self.dir = path;
                self.scroll = 0;
                self.selected = self.entries.iter().position(|e| e.is_video);
                if let Some(sel) = self.selected {
                    self.ensure_visible(sel);
                }
                self.error = None;
                self.last_click = (None, -10.0);
            }
            Err(e) => {
                self.error = Some(format!("Can't open this folder: {e}"));
            }
        }
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self.dir.parent() {
            let parent = parent.to_path_buf();
            self.open_at(parent);
        }
    }

    pub fn visible_rows(&self, list: Rect) -> usize {
        ((list.h - 8.0) / ROW).floor().max(1.0) as usize
    }

    fn ensure_visible(&mut self, idx: usize) {
        let visible = 12;
        if idx < self.scroll {
            self.scroll = idx;
        } else if idx >= self.scroll + visible {
            self.scroll = idx.saturating_sub(visible - 1);
        }
    }

    fn activate(&mut self, idx: usize) -> BrowserAction {
        if let Some(e) = self.entries.get(idx) {
            if e.is_dir {
                let p = e.path.clone();
                self.open_at(p);
                BrowserAction::None
            } else {
                BrowserAction::Open(e.path.clone())
            }
        } else {
            BrowserAction::None
        }
    }

    pub fn handle_keys(&mut self) -> BrowserAction {
        if is_key_pressed(KeyCode::Escape) {
            return BrowserAction::Cancel;
        }
        if is_key_pressed(KeyCode::Backspace) {
            self.go_up();
            return BrowserAction::None;
        }
        if is_key_pressed(KeyCode::Up) {
            if let Some(sel) = self.selected {
                let next = sel.saturating_sub(1);
                self.selected = Some(next);
                self.ensure_visible(next);
            } else if !self.entries.is_empty() {
                self.selected = Some(0);
            }
        }
        if is_key_pressed(KeyCode::Down) {
            if let Some(sel) = self.selected {
                let next = (sel + 1).min(self.entries.len().saturating_sub(1));
                self.selected = Some(next);
                self.ensure_visible(next);
            } else if !self.entries.is_empty() {
                self.selected = Some(0);
            }
        }
        if is_key_pressed(KeyCode::Enter) {
            if let Some(sel) = self.selected {
                return self.activate(sel);
            }
        }
        BrowserAction::None
    }

    pub fn handle_mouse(&mut self, l: &BrowserLayout, now: f64) -> BrowserAction {
        if clicked(l.close) || clicked(l.cancel) {
            return BrowserAction::Cancel;
        }
        if clicked(l.up) {
            self.go_up();
            return BrowserAction::None;
        }

        // Sidebar shortcuts.
        let shortcuts = shortcuts();
        for (i, (_, path)) in shortcuts.iter().enumerate() {
            let r = Rect::new(
                l.sidebar.x + 8.0,
                l.sidebar.y + 10.0 + i as f32 * 36.0,
                l.sidebar.w - 16.0,
                30.0,
            );
            if clicked(r) {
                self.open_at(path.clone());
                return BrowserAction::None;
            }
        }

        let (_, my) = mouse_position();
        if mouse_in(l.list) {
            let (_, wy) = mouse_wheel();
            if wy.abs() > 0.01 {
                let max = self.entries.len().saturating_sub(self.visible_rows(l.list));
                if wy > 0.0 {
                    self.scroll = self.scroll.saturating_sub(1);
                } else {
                    self.scroll = (self.scroll + 1).min(max);
                }
            }
            if is_mouse_button_pressed(MouseButton::Left) {
                let rel = ((my - l.list.y - 4.0) / ROW).floor();
                if rel >= 0.0 {
                    let idx = self.scroll + rel as usize;
                    if idx < self.entries.len() {
                        let double = self.last_click.0 == Some(idx)
                            && now - self.last_click.1 < 0.38
                            && is_mouse_button_pressed(MouseButton::Left);
                        self.selected = Some(idx);
                        self.last_click = (Some(idx), now);
                        if double {
                            return self.activate(idx);
                        }
                    }
                }
            }
        }

        if clicked(l.open) {
            if let Some(idx) = self.selected {
                return self.activate(idx);
            }
        }
        BrowserAction::None
    }

    pub fn draw(&self, l: &BrowserLayout, fonts: &Fonts) {
        let (sw, sh) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.62));
        // soft shadow
        rounded_rect(
            l.modal.x - 6.0,
            l.modal.y + 8.0,
            l.modal.w + 12.0,
            l.modal.h + 12.0,
            20.0,
            Color::new(0.0, 0.0, 0.0, 0.28),
        );
        rounded_rect(l.modal.x, l.modal.y, l.modal.w, l.modal.h, 16.0, panel());

        text_left(
            &fonts.bold,
            "Open a video",
            l.modal.x + 24.0,
            l.modal.y + 18.0,
            22,
            text_color(),
        );
        text_left(
            &fonts.ui,
            "Files are played without their audio track.",
            l.modal.x + 24.0,
            l.modal.y + 43.0,
            12,
            muted(),
        );
        // close X
        let c = l.close;
        if mouse_in(c) {
            rounded_rect(c.x, c.y, c.w, c.h, 8.0, panel2());
        }
        draw_line(c.x + 10.0, c.y + 10.0, c.x + 20.0, c.y + 20.0, 1.6, muted());
        draw_line(c.x + 20.0, c.y + 10.0, c.x + 10.0, c.y + 20.0, 1.6, muted());

        // path row
        button(fonts, l.up, "Up", BtnStyle::Ghost, true);
        let path_text = l.path_row.x + 64.0;
        let avail = l.modal.x + l.modal.w - 24.0 - path_text;
        text_left(
            &fonts.mono,
            &tail_ellipsize(&fonts.mono, &self.dir.to_string_lossy(), 12, avail),
            path_text,
            l.path_row.y + 9.0,
            12,
            muted(),
        );

        // sidebar
        if l.sidebar.w > 0.0 {
            rounded_rect(
                l.sidebar.x,
                l.sidebar.y,
                l.sidebar.w,
                l.sidebar.h,
                10.0,
                panel2(),
            );
            let shortcuts = shortcuts();
            for (i, (label, path)) in shortcuts.iter().enumerate() {
                let r = Rect::new(
                    l.sidebar.x + 8.0,
                    l.sidebar.y + 10.0 + i as f32 * 36.0,
                    l.sidebar.w - 16.0,
                    30.0,
                );
                let active = *path == self.dir;
                let hot = mouse_in(r);
                if active || hot {
                    rounded_rect(
                        r.x,
                        r.y,
                        r.w,
                        r.h,
                        7.0,
                        if active { panel3() } else { panel() },
                    );
                }
                folder_icon(
                    r.x + 9.0,
                    r.y + 8.0,
                    15.0,
                    if active { accent() } else { muted() },
                );
                text_left(
                    &fonts.ui,
                    label,
                    r.x + 34.0,
                    r.y + 8.0,
                    12,
                    if active { text_color() } else { muted() },
                );
            }
            text_left(
                &fonts.ui,
                "Double-click to open",
                l.sidebar.x + 10.0,
                l.sidebar.y + l.sidebar.h - 24.0,
                10,
                dim(),
            );
        }

        // file list
        draw_rectangle(l.list.x, l.list.y, l.list.w, l.list.h, bg());
        let visible = self.visible_rows(l.list);
        let mut y = l.list.y + 4.0;
        for i in self.scroll..self.entries.len().min(self.scroll + visible) {
            let e = &self.entries[i];
            let r = Rect::new(l.list.x + 6.0, y, l.list.w - 14.0, ROW - 2.0);
            let selected = self.selected == Some(i);
            let hot = mouse_in(r);
            if selected {
                rounded_rect(r.x, r.y, r.w, r.h, 7.0, accent_soft());
                draw_rectangle(r.x + 1.0, r.y + 7.0, 2.5, r.h - 14.0, accent());
            } else if hot {
                rounded_rect(r.x, r.y, r.w, r.h, 7.0, panel2());
            }
            if e.is_dir {
                folder_icon(r.x + 10.0, r.y + 8.0, 16.0, accent());
            } else if e.is_video {
                film_icon(r.x + 9.0, r.y + 9.0, 20.0, 14.0, accent());
            } else {
                film_icon(r.x + 9.0, r.y + 9.0, 20.0, 14.0, dim());
            }
            let name_color = if e.is_video || e.is_dir {
                text_color()
            } else {
                muted()
            };
            let name = ellipsize(&fonts.ui, &e.name, 13, r.w - 210.0);
            text_left(&fonts.ui, &name, r.x + 38.0, r.y + 9.0, 13, name_color);
            let right = if e.is_dir {
                "folder".to_string()
            } else if e.is_video {
                format_size(e.size)
            } else {
                format!("{}  {}", extension(&e.path), format_size(e.size))
            };
            text_right(&fonts.mono, &right, r.x + r.w - 8.0, r.y + 10.0, 11, dim());
            y += ROW;
        }

        if self.entries.is_empty() {
            text_center(
                &fonts.ui,
                "Nothing in this folder",
                l.list.x + l.list.w / 2.0,
                l.list.y + l.list.h / 2.0 - 6.0,
                14,
                muted(),
            );
        }

        // scrollbar
        let max_scroll = self.entries.len().saturating_sub(visible);
        if max_scroll > 0 {
            let track = Rect::new(
                l.list.x + l.list.w - 6.0,
                l.list.y + 6.0,
                3.0,
                l.list.h - 12.0,
            );
            draw_rectangle(track.x, track.y, track.w, track.h, panel2());
            let frac = (self.scroll as f32) / (max_scroll as f32);
            let thumb_h = (visible as f32 / self.entries.len() as f32) * track.h;
            let thumb_y = track.y + frac * (track.h - thumb_h);
            rounded_rect(track.x, thumb_y, track.w, thumb_h.max(18.0), 1.5, accent());
        }

        // footer
        draw_line(
            l.footer.x,
            l.footer.y - 2.0,
            l.footer.x + l.footer.w,
            l.footer.y - 2.0,
            1.0,
            line_color(),
        );
        let info = match self.selected.and_then(|i| self.entries.get(i)) {
            Some(e) => format!(
                "{}  ·  {}",
                e.name,
                if e.is_dir {
                    "folder".to_string()
                } else {
                    format_size(e.size)
                }
            ),
            None => "Choose a file to play".to_string(),
        };
        text_left(
            &fonts.ui,
            &ellipsize(&fonts.ui, &info, 12, l.footer.w - 250.0),
            l.footer.x,
            l.footer.y + 18.0,
            12,
            muted(),
        );
        let has_selection = self.selected.is_some();
        let _ = button(fonts, l.cancel, "Cancel", BtnStyle::Ghost, true);
        button(fonts, l.open, "Open", BtnStyle::Primary, has_selection);
        if let Some(err) = &self.error {
            text_left(
                &fonts.ui,
                &ellipsize(&fonts.ui, err, 11, l.footer.w - 250.0),
                l.footer.x,
                l.footer.y + 38.0,
                11,
                signal_red(),
            );
        }
    }
}

#[cfg(windows)]
fn drive_roots() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for letter in b'A'..=b'Z' {
        let label = format!("{}:", letter as char);
        let path = PathBuf::from(format!("{label}\\"));
        if path.is_dir() {
            out.push((label, path));
        }
    }
    out
}

fn shortcuts() -> Vec<(String, PathBuf)> {
    let home = home_dir();
    let mut out = vec![("Home".to_string(), home.clone())];
    for (label, folder) in [
        ("Videos", "Videos"),
        ("Downloads", "Downloads"),
        ("Documents", "Documents"),
    ] {
        let p = home.join(folder);
        if p.is_dir() {
            out.push((label.to_string(), p));
        }
    }
    #[cfg(windows)]
    {
        // Windows has no single root folder, so each available drive is a
        // shortcut instead of the original's "Computer" entry.
        for (label, path) in drive_roots() {
            if path != home {
                out.push((label, path));
            }
        }
    }
    #[cfg(not(windows))]
    out.push(("Computer".to_string(), PathBuf::from("/")));
    out
}

fn extension(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| "FILE".to_string())
        .chars()
        .take(6)
        .collect()
}

pub fn format_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB", b / (1024.0 * 1024.0 * 1024.0))
    } else if b >= 1024.0 * 1024.0 {
        format!("{:.0} MB", b / (1024.0 * 1024.0))
    } else if b >= 1024.0 {
        format!("{:.0} KB", b / 1024.0)
    } else {
        format!("{b:.0} B")
    }
}

fn tail_ellipsize(font: &Font, s: &str, size: u16, max_w: f32) -> String {
    if text_width(font, s, size) <= max_w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for ch in chars.iter().rev() {
        let mut candidate = ch.to_string();
        candidate.push_str(&out);
        if text_width(font, &candidate, size) + text_width(font, "…", size) > max_w {
            break;
        }
        out = candidate;
    }
    format!("…{out}")
}
