use macroquad::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ---------------------------------------------------------------- palette
// The dark palette is the default. The light palette keeps the same contrast
// relationships so every control stays legible when the background flips.
const DARK_BG: Color = Color::new(0.043, 0.051, 0.063, 1.0);
const DARK_PANEL: Color = Color::new(0.078, 0.090, 0.106, 1.0);
const DARK_PANEL2: Color = Color::new(0.110, 0.125, 0.147, 1.0);
const DARK_PANEL3: Color = Color::new(0.145, 0.163, 0.190, 1.0);
const DARK_LINE: Color = Color::new(0.192, 0.212, 0.243, 1.0);
const DARK_TEXT: Color = Color::new(0.918, 0.902, 0.871, 1.0);
const DARK_MUTED: Color = Color::new(0.616, 0.647, 0.690, 1.0);
const DARK_DIM: Color = Color::new(0.392, 0.424, 0.467, 1.0);
const DARK_GREEN: Color = Color::new(0.400, 0.745, 0.549, 1.0);
const DARK_RED: Color = Color::new(0.878, 0.463, 0.408, 1.0);

const LIGHT_BG: Color = Color::new(0.941, 0.937, 0.925, 1.0);
const LIGHT_PANEL: Color = Color::new(0.984, 0.980, 0.969, 1.0);
const LIGHT_PANEL2: Color = Color::new(0.918, 0.910, 0.890, 1.0);
const LIGHT_PANEL3: Color = Color::new(0.851, 0.839, 0.812, 1.0);
const LIGHT_LINE: Color = Color::new(0.741, 0.722, 0.686, 1.0);
const LIGHT_TEXT: Color = Color::new(0.118, 0.114, 0.106, 1.0);
const LIGHT_MUTED: Color = Color::new(0.365, 0.353, 0.329, 1.0);
const LIGHT_DIM: Color = Color::new(0.541, 0.525, 0.490, 1.0);
const LIGHT_GREEN: Color = Color::new(0.145, 0.510, 0.294, 1.0);
const LIGHT_RED: Color = Color::new(0.769, 0.204, 0.161, 1.0);

static LIGHT_MODE: AtomicBool = AtomicBool::new(false);

pub fn light_mode() -> bool {
    LIGHT_MODE.load(Ordering::Relaxed)
}

pub fn set_light_mode(light: bool) {
    LIGHT_MODE.store(light, Ordering::Relaxed);
}

pub fn bg() -> Color {
    if light_mode() {
        LIGHT_BG
    } else {
        DARK_BG
    }
}

pub fn panel() -> Color {
    if light_mode() {
        LIGHT_PANEL
    } else {
        DARK_PANEL
    }
}

pub fn panel2() -> Color {
    if light_mode() {
        LIGHT_PANEL2
    } else {
        DARK_PANEL2
    }
}

pub fn panel3() -> Color {
    if light_mode() {
        LIGHT_PANEL3
    } else {
        DARK_PANEL3
    }
}

pub fn line_color() -> Color {
    if light_mode() {
        LIGHT_LINE
    } else {
        DARK_LINE
    }
}

pub fn text_color() -> Color {
    if light_mode() {
        LIGHT_TEXT
    } else {
        DARK_TEXT
    }
}

pub fn muted() -> Color {
    if light_mode() {
        LIGHT_MUTED
    } else {
        DARK_MUTED
    }
}

pub fn dim() -> Color {
    if light_mode() {
        LIGHT_DIM
    } else {
        DARK_DIM
    }
}

pub fn signal_green() -> Color {
    if light_mode() {
        LIGHT_GREEN
    } else {
        DARK_GREEN
    }
}

pub fn signal_red() -> Color {
    if light_mode() {
        LIGHT_RED
    } else {
        DARK_RED
    }
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Red,
}

pub const THEMES: [Theme; 6] = [
    Theme::Orange,
    Theme::Yellow,
    Theme::Green,
    Theme::Blue,
    Theme::Purple,
    Theme::Red,
];

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Theme::Orange => "Orange",
            Theme::Yellow => "Yellow",
            Theme::Green => "Green",
            Theme::Blue => "Blue",
            Theme::Purple => "Purple",
            Theme::Red => "Red",
        }
    }

    pub fn accent(self) -> Color {
        match self {
            Theme::Orange => Color::new(0.902, 0.647, 0.290, 1.0),
            Theme::Yellow => Color::new(0.925, 0.804, 0.286, 1.0),
            Theme::Green => Color::new(0.400, 0.745, 0.549, 1.0),
            Theme::Blue => Color::new(0.365, 0.620, 0.925, 1.0),
            Theme::Purple => Color::new(0.690, 0.478, 0.937, 1.0),
            Theme::Red => Color::new(0.878, 0.463, 0.408, 1.0),
        }
    }

    pub fn bright(self) -> Color {
        match self {
            Theme::Orange => Color::new(0.973, 0.729, 0.369, 1.0),
            Theme::Yellow => Color::new(0.984, 0.890, 0.420, 1.0),
            Theme::Green => Color::new(0.510, 0.847, 0.643, 1.0),
            Theme::Blue => Color::new(0.478, 0.714, 0.980, 1.0),
            Theme::Purple => Color::new(0.788, 0.612, 0.992, 1.0),
            Theme::Red => Color::new(0.949, 0.565, 0.502, 1.0),
        }
    }

    /// A darker accent for text and markers on the light palette.
    pub fn deep(self) -> Color {
        match self {
            Theme::Orange => Color::new(0.620, 0.337, 0.063, 1.0),
            Theme::Yellow => Color::new(0.596, 0.451, 0.024, 1.0),
            Theme::Green => Color::new(0.129, 0.447, 0.251, 1.0),
            Theme::Blue => Color::new(0.102, 0.337, 0.698, 1.0),
            Theme::Purple => Color::new(0.463, 0.220, 0.776, 1.0),
            Theme::Red => Color::new(0.722, 0.196, 0.157, 1.0),
        }
    }

    pub fn ink(self) -> Color {
        Color::new(0.055, 0.047, 0.071, 1.0)
    }
}

static KEEP_ON_TOP: AtomicBool = AtomicBool::new(false);

/// Whether the player asks the window manager to keep this window above
/// ordinary windows. The choice is saved with the other appearance settings.
pub fn keep_on_top() -> bool {
    KEEP_ON_TOP.load(Ordering::Relaxed)
}

pub fn set_keep_on_top(value: bool) {
    KEEP_ON_TOP.store(value, Ordering::Relaxed);
}

static THEME_INDEX: AtomicUsize = AtomicUsize::new(4);

pub fn theme() -> Theme {
    let index = THEME_INDEX.load(Ordering::Relaxed).min(THEMES.len() - 1);
    THEMES[index]
}

pub fn set_theme(value: Theme) {
    if let Some(index) = THEMES.iter().position(|t| *t == value) {
        THEME_INDEX.store(index, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------- preferences
/// Where the appearance choices live between launches. Following the XDG
/// convention keeps the file with the rest of the user's app settings.
pub fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("vidsync").join("settings.json"))
}

fn apply_saved_settings(text: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    if let Some(mode) = value.get("mode").and_then(|v| v.as_str()) {
        set_light_mode(mode.eq_ignore_ascii_case("light"));
    }
    if let Some(accent) = value.get("accent").and_then(|v| v.as_str()) {
        if let Some(theme) = THEMES
            .iter()
            .find(|theme| theme.label().eq_ignore_ascii_case(accent))
        {
            set_theme(*theme);
        }
    }
    if let Some(above) = value.get("keep_on_top").and_then(|v| v.as_bool()) {
        set_keep_on_top(above);
    }
}

/// Restore the last dark/light mode and accent colour. Missing or unreadable
/// settings simply keep the built-in defaults (dark and purple).
pub fn load_settings() {
    let Some(path) = settings_path() else {
        return;
    };
    if let Ok(text) = fs::read_to_string(path) {
        apply_saved_settings(&text);
    }
}

/// Write the current mode and accent colour so the next launch starts with them.
pub fn save_settings() {
    let Some(path) = settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let value = serde_json::json!({
        "mode": if light_mode() { "light" } else { "dark" },
        "accent": theme().label().to_lowercase(),
        "keep_on_top": keep_on_top(),
    });
    if let Ok(text) = serde_json::to_string_pretty(&value) {
        let _ = fs::write(path, text);
    }
}

pub fn accent() -> Color {
    theme().accent()
}

pub fn accent_bright() -> Color {
    if light_mode() {
        theme().deep()
    } else {
        theme().bright()
    }
}

pub fn accent_soft() -> Color {
    let c = accent();
    Color::new(c.r, c.g, c.b, 0.16)
}

pub fn accent_ink() -> Color {
    theme().ink()
}

pub struct Fonts {
    pub ui: Font,
    pub bold: Font,
    pub mono: Font,
    pub mono_bold: Font,
}

impl Fonts {
    pub fn load() -> Fonts {
        Fonts {
            ui: load_ttf_font_from_bytes(include_bytes!("../assets/fonts/DejaVuSans.ttf"))
                .expect("bundled font"),
            bold: load_ttf_font_from_bytes(include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf"))
                .expect("bundled font"),
            mono: load_ttf_font_from_bytes(include_bytes!("../assets/fonts/DejaVuSansMono.ttf"))
                .expect("bundled font"),
            mono_bold: load_ttf_font_from_bytes(include_bytes!(
                "../assets/fonts/DejaVuSansMono-Bold.ttf"
            ))
            .expect("bundled font"),
        }
    }
}

// ---------------------------------------------------------------- text
/// Draw text with its visual top-left corner at (x, y).
pub fn text_left(font: &Font, s: &str, x: f32, y: f32, size: u16, color: Color) {
    let d = measure_text(s, Some(font), size, 1.0);
    draw_text_ex(
        s,
        x,
        y + d.offset_y,
        TextParams {
            font: Some(font),
            font_size: size,
            color,
            ..Default::default()
        },
    );
}

pub fn text_center(font: &Font, s: &str, cx: f32, y: f32, size: u16, color: Color) {
    let d = measure_text(s, Some(font), size, 1.0);
    text_left(font, s, cx - d.width / 2.0, y, size, color);
}

pub fn text_right(font: &Font, s: &str, rx: f32, y: f32, size: u16, color: Color) {
    let d = measure_text(s, Some(font), size, 1.0);
    text_left(font, s, rx - d.width, y, size, color);
}

/// Draw text centred inside a rectangle.
pub fn text_in_rect(font: &Font, s: &str, r: Rect, size: u16, color: Color) {
    let d = measure_text(s, Some(font), size, 1.0);
    let x = r.x + (r.w - d.width) / 2.0;
    let top = r.y + (r.h - d.height) / 2.0;
    text_left(font, s, x, top, size, color);
}

pub fn text_width(font: &Font, s: &str, size: u16) -> f32 {
    measure_text(s, Some(font), size, 1.0).width
}

/// Shorten a string with an ellipsis so that it fits in max_w.
pub fn ellipsize(font: &Font, s: &str, size: u16, max_w: f32) -> String {
    if text_width(font, s, size) <= max_w {
        return s.to_string();
    }
    let ell = "…";
    let ell_w = text_width(font, ell, size);
    let mut out = String::new();
    for ch in s.chars() {
        let mut candidate = out.clone();
        candidate.push(ch);
        if text_width(font, &candidate, size) + ell_w > max_w {
            break;
        }
        out = candidate;
    }
    out.push('…');
    out
}

// ---------------------------------------------------------------- shapes
pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r <= 0.5 {
        draw_rectangle(x, y, w, h, color);
        return;
    }
    draw_rectangle(x + r, y, w - 2.0 * r, h, color);
    draw_rectangle(x, y + r, r, h - 2.0 * r, color);
    draw_rectangle(x + w - r, y + r, r, h - 2.0 * r, color);
    draw_circle(x + r, y + r, r, color);
    draw_circle(x + w - r, y + r, r, color);
    draw_circle(x + r, y + h - r, r, color);
    draw_circle(x + w - r, y + h - r, r, color);
}

pub fn mouse_in(r: Rect) -> bool {
    let (mx, my) = mouse_position();
    r.contains(vec2(mx, my))
}

pub fn clicked(r: Rect) -> bool {
    is_mouse_button_pressed(MouseButton::Left) && mouse_in(r)
}

pub enum BtnStyle {
    Primary,
    Ghost,
}

pub fn button(fonts: &Fonts, r: Rect, label: &str, style: BtnStyle, enabled: bool) -> bool {
    let hovered = enabled && mouse_in(r);
    let (bg, fg) = match style {
        BtnStyle::Primary => (
            if hovered { accent_bright() } else { accent() },
            accent_ink(),
        ),
        BtnStyle::Ghost => (
            if hovered { panel3() } else { panel2() },
            if enabled { text_color() } else { dim() },
        ),
    };
    rounded_rect(r.x, r.y, r.w, r.h, 9.0, bg);
    let font = &fonts.bold;
    text_in_rect(font, label, r, 13, fg);
    enabled && hovered && is_mouse_button_pressed(MouseButton::Left)
}

/// A small on/off switch. The knob slides to the right when it is on.
pub fn toggle_switch(r: Rect, on: bool, hovered: bool) {
    let track = if on {
        accent()
    } else if hovered {
        panel3()
    } else {
        line_color()
    };
    rounded_rect(r.x, r.y, r.w, r.h, r.h / 2.0, track);
    if hovered {
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.2, accent_bright());
    }
    let knob = (r.h / 2.0 - 2.0).max(2.0);
    let cx = r.x + if on { r.w - r.h / 2.0 } else { r.h / 2.0 };
    draw_circle(
        cx,
        r.y + r.h / 2.0,
        knob,
        if on {
            Color::new(0.98, 0.97, 0.94, 1.0)
        } else {
            muted()
        },
    );
}

// ---------------------------------------------------------------- icons
pub fn play_icon(cx: f32, cy: f32, size: f32, color: Color) {
    let h = size;
    let w = size * 0.86;
    draw_triangle(
        vec2(cx - w * 0.36, cy - h / 2.0),
        vec2(cx - w * 0.36, cy + h / 2.0),
        vec2(cx + w * 0.60, cy),
        color,
    );
}

pub fn folder_icon(x: f32, y: f32, s: f32, color: Color) {
    let h = s * 0.72;
    rounded_rect(x, y + s - h, s, h, 2.5, color);
    rounded_rect(x, y + s - h - s * 0.18, s * 0.55, s * 0.30, 2.0, color);
}

pub fn film_icon(x: f32, y: f32, w: f32, h: f32, color: Color) {
    rounded_rect(x, y, w, h, 8.0, color);
    let hole = bg();
    let n = 4;
    let hw = w * 0.045;
    for i in 0..n {
        let yy = y + 8.0 + i as f32 * ((h - 16.0) / (n - 1) as f32) - hw;
        rounded_rect(x + 7.0, yy, hw, hw, 1.2, hole);
        rounded_rect(x + w - 7.0 - hw, yy, hw, hw, 1.2, hole);
    }
    // Lens / play mark in the middle.
    play_icon(x + w * 0.53, y + h * 0.5, h * 0.22, hole);
}

pub fn dot(cx: f32, cy: f32, r: f32, color: Color) {
    draw_circle(cx, cy, r, color);
}

/// A small gear: eight teeth around a ring with a hollow centre.
pub fn gear_icon(cx: f32, cy: f32, r: f32, color: Color) {
    let tooth = (r * 0.28).max(1.4);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let inner = r * 0.62;
        let outer = r;
        draw_line(
            cx + a.cos() * inner,
            cy + a.sin() * inner,
            cx + a.cos() * outer,
            cy + a.sin() * outer,
            tooth,
            color,
        );
    }
    draw_circle_lines(cx, cy, r * 0.55, (r * 0.22).max(1.0), color);
    draw_circle(cx, cy, r * 0.17, color);
}

/// Four corner brackets, the usual "enter fullscreen" mark.
pub fn fullscreen_icon(cx: f32, cy: f32, r: f32, color: Color) {
    let arm = r * 0.82;
    let thick = (r * 0.24).max(1.2);
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let x = cx + sx * r;
        let y = cy + sy * r;
        draw_line(x, y, x - sx * arm, y, thick, color);
        draw_line(x, y, x, y - sy * arm, thick, color);
    }
}

// ---------------------------------------------------------------- time
pub fn fmt_time(seconds: f64, millis: bool) -> String {
    let t = seconds.max(0.0);
    let total_ms = (t * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total = total_ms / 1000;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if millis {
        format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
    } else {
        format!("{:02}:{:02}:{:02}", h, m, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_clocks() {
        assert_eq!(fmt_time(0.0, false), "00:00:00");
        assert_eq!(fmt_time(3723.0, false), "01:02:03");
        assert_eq!(fmt_time(3723.456, true), "01:02:03.456");
        assert_eq!(fmt_time(-5.0, false), "00:00:00");
    }

    #[test]
    fn light_mode_switches_palette() {
        set_light_mode(false);
        let dark_bg = bg();
        set_light_mode(true);
        assert!(light_mode());
        assert_ne!(bg(), dark_bg);
        set_light_mode(false);
        assert!(!light_mode());
        assert_eq!(bg(), dark_bg);
    }

    #[test]
    fn six_accent_colours_switch() {
        for t in THEMES {
            set_theme(t);
            assert_eq!(theme(), t);
            assert_eq!(accent(), t.accent());
        }
        set_theme(Theme::Purple);
        assert_eq!(theme(), Theme::Purple);
    }

    #[test]
    fn reads_saved_appearance() {
        set_light_mode(false);
        set_theme(Theme::Purple);
        set_keep_on_top(false);
        apply_saved_settings(r#"{"mode":"light","accent":"green","keep_on_top":true}"#);
        assert!(light_mode());
        assert_eq!(theme(), Theme::Green);
        assert!(keep_on_top());
        set_light_mode(false);
        set_theme(Theme::Purple);
        set_keep_on_top(false);
    }
}
