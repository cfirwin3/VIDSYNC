//! Audacity 4.0 timecode over the accessibility bus.
//!
//! Audacity 4.0 is a Qt application and publishes its playback position as an
//! AT-SPI accessible value (`Playback position:00 h 00 m 11.037 s`). Reading
//! that value is independent of which window is on top, so VIDSYNC can follow
//! Audacity even while it is behind another window or minimised.
//!
//! A background thread discovers the Audacity accessible tree, caches the node
//! that holds the timecode, and polls it about thirty times a second. The UI
//! only reads a small snapshot, so a slow or missing Audacity never stalls the
//! video.

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const POLL: Duration = Duration::from_millis(33);
const REDISCOVER: Duration = Duration::from_millis(1500);

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub connected: bool,
    pub position: Option<f64>,
    /// True while the timecode has changed recently, i.e. Audacity is running.
    pub moving: bool,
    /// True when Audacity's timecode display uses the requested
    /// `00h00m00.000s` style rather than a bare colon clock.
    pub format_ok: bool,
    pub message: String,
}

struct TimecodeNode {
    conn: Connection,
    bus: String,
    path: String,
}

impl TimecodeNode {
    fn read(&self) -> Result<String, Box<dyn Error>> {
        let props = Proxy::new(
            &self.conn,
            self.bus.as_str(),
            self.path.as_str(),
            "org.freedesktop.DBus.Properties",
        )?;
        let value: OwnedValue = props.call("Get", &("org.a11y.atspi.Accessible", "Name"))?;
        Ok(String::try_from(value)?)
    }
}

pub struct AudacityLink {
    state: Arc<Mutex<Snapshot>>,
    stop: Arc<AtomicBool>,
}

impl AudacityLink {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(Snapshot {
            connected: false,
            position: None,
            moving: false,
            format_ok: false,
            message: "looking for Audacity…".to_string(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_state = Arc::clone(&state);
        let worker_stop = Arc::clone(&stop);
        thread::spawn(move || poll_loop(worker_state, worker_stop));
        AudacityLink { state, stop }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for AudacityLink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn set_state(state: &Mutex<Snapshot>, next: Snapshot) {
    if let Ok(mut current) = state.lock() {
        *current = next;
    }
}

fn poll_loop(state: Arc<Mutex<Snapshot>>, stop: Arc<AtomicBool>) {
    let mut node: Option<TimecodeNode> = None;
    let mut next_discover = Instant::now();
    let mut last_position: Option<f64> = None;
    let mut last_change: Option<Instant> = None;

    while !stop.load(Ordering::Relaxed) {
        if Instant::now() >= next_discover {
            match discover() {
                Ok(found) => {
                    node = Some(found);
                    next_discover = Instant::now() + REDISCOVER * 8;
                }
                Err(message) => {
                    node = None;
                    last_position = None;
                    last_change = None;
                    next_discover = Instant::now() + Duration::from_secs(1);
                    set_state(
                        &state,
                        Snapshot {
                            connected: false,
                            position: None,
                            moving: false,
                            format_ok: false,
                            message,
                        },
                    );
                }
            }
        }

        if let Some(current) = node.as_ref() {
            match current.read() {
                Ok(raw) => {
                    if let Some(position) = parse_timecode(&raw) {
                        let changed = last_position
                            .map(|previous| (position - previous).abs() > 0.0005)
                            .unwrap_or(false);
                        if changed {
                            last_change = Some(Instant::now());
                        }
                        last_position = Some(position);
                        let moving = last_change
                            .map(|at| at.elapsed() < Duration::from_millis(220))
                            .unwrap_or(false);
                        let format_ok = timecode_uses_hms(&raw);
                        let message = if format_ok {
                            "timecode live".to_string()
                        } else {
                            "set Audacity to 00h00m00.000s".to_string()
                        };
                        set_state(
                            &state,
                            Snapshot {
                                connected: true,
                                position: Some(position),
                                moving,
                                format_ok,
                                message,
                            },
                        );
                    } else {
                        // The node was found but its text is not a timecode we
                        // understand. Re-discover soon in case Audacity swapped
                        // the timecode display.
                        node = None;
                        last_position = None;
                        last_change = None;
                        set_state(
                            &state,
                            Snapshot {
                                connected: false,
                                position: None,
                                moving: false,
                                format_ok: false,
                                message: "timecode format not recognised".to_string(),
                            },
                        );
                    }
                }
                Err(_) => {
                    node = None;
                    last_position = None;
                    last_change = None;
                    set_state(
                        &state,
                        Snapshot {
                            connected: false,
                            position: None,
                            moving: false,
                            format_ok: false,
                            message: "Audacity closed".to_string(),
                        },
                    );
                }
            }
        }

        thread::sleep(POLL);
    }
}

/// Audacity 4.0's timecode can be shown as `00h00m00.000s` or as a colon
/// clock. The person asked for the dot to turn green only for the hour /
/// minute / second display, so that choice is reported separately from the
/// value itself.
pub fn timecode_uses_hms(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    let clock = lower.rsplit(':').next().unwrap_or(lower.as_str());
    clock.contains('h') && clock.contains('m') && clock.contains('s')
}

fn discover() -> Result<TimecodeNode, String> {
    discover_inner().map_err(|e| friendly(e.to_string()))
}

fn friendly(message: String) -> String {
    if message.contains("org.a11y") || message.contains("Accessibility") {
        "accessibility bus unavailable".to_string()
    } else if message.contains("not found") || message.contains("No such") {
        "Audacity not found".to_string()
    } else {
        "waiting for Audacity".to_string()
    }
}

fn discover_inner() -> Result<TimecodeNode, Box<dyn Error>> {
    let session = Connection::session()?;

    // Make sure the desktop accessibility bridge is active. Audacity only
    // builds its accessible tree while this is switched on.
    if let Ok(props) = Proxy::new(
        &session,
        "org.a11y.Bus",
        "/org/a11y/bus",
        "org.freedesktop.DBus.Properties",
    ) {
        let enabled = zbus::zvariant::Value::Bool(true);
        let _: Result<(), _> = props.call("Set", &("org.a11y.Status", "IsEnabled", enabled));
    }

    let bus = Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus")?;
    let address: String = bus.call("GetAddress", &())?;
    let conn = zbus::blocking::connection::Builder::address(address.as_str())?.build()?;

    let root = Proxy::new(
        &conn,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/accessible/root",
        "org.a11y.atspi.Accessible",
    )?;
    let apps: Vec<(String, OwnedObjectPath)> = root.call("GetChildren", &())?;

    for (app_bus, app_path) in apps {
        let name = get_string(&conn, &app_bus, app_path.as_str(), "Name").unwrap_or_default();
        if !name.to_ascii_lowercase().contains("audacity") {
            continue;
        }
        let count = get_i32(&conn, &app_bus, app_path.as_str(), "ChildCount").unwrap_or(0);
        if count <= 0 {
            continue;
        }
        if let Some(path) = find_timecode(&conn, &app_bus, app_path.as_str(), 0)? {
            return Ok(TimecodeNode {
                conn,
                bus: app_bus,
                path,
            });
        }
    }

    Err("Audacity 4.0 timecode was not found".into())
}

/// Recognise the accessible name that carries Audacity's position. The
/// English label is checked first; the fallback accepts a short name that is a
/// clock, so a translated Audacity still works.
fn looks_like_timecode(name: &str) -> bool {
    let text = name.trim();
    if text.is_empty() || text.len() > 96 || parse_timecode(text).is_none() {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    lower.contains("position")
        || lower.contains("timecode")
        || lower.contains(':')
        || timecode_uses_hms(text)
}

fn find_timecode(
    conn: &Connection,
    bus: &str,
    path: &str,
    depth: usize,
) -> Result<Option<String>, Box<dyn Error>> {
    if depth > 14 {
        return Ok(None);
    }
    let accessible = Proxy::new(conn, bus, path, "org.a11y.atspi.Accessible")?;
    let name = get_string(conn, bus, path, "Name").unwrap_or_default();
    if looks_like_timecode(&name) {
        return Ok(Some(path.to_string()));
    }

    let count = get_i32(conn, bus, path, "ChildCount").unwrap_or(0);
    if count <= 0 {
        return Ok(None);
    }
    let children: Vec<(String, OwnedObjectPath)> = accessible.call("GetChildren", &())?;
    for (_, child) in children {
        if let Some(found) = find_timecode(conn, bus, child.as_str(), depth + 1)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

fn get_prop(
    conn: &Connection,
    bus: &str,
    path: &str,
    property: &str,
) -> Result<OwnedValue, Box<dyn Error>> {
    let props = Proxy::new(conn, bus, path, "org.freedesktop.DBus.Properties")?;
    Ok(props.call("Get", &("org.a11y.atspi.Accessible", property))?)
}

fn get_string(
    conn: &Connection,
    bus: &str,
    path: &str,
    property: &str,
) -> Result<String, Box<dyn Error>> {
    Ok(String::try_from(get_prop(conn, bus, path, property)?)?)
}

fn get_i32(
    conn: &Connection,
    bus: &str,
    path: &str,
    property: &str,
) -> Result<i32, Box<dyn Error>> {
    Ok(i32::try_from(get_prop(conn, bus, path, property)?)?)
}

/// Parse the text Audacity exposes for its playback position.
///
/// The display can be `00h00m00.000s`, `00 h 00 m 11.037 s`, or a colon clock
/// such as `00:00:11.037`; all describe the same hours:minutes:seconds.mmm
/// value. The readable prefix is removed before the number is read.
pub fn parse_timecode(raw: &str) -> Option<f64> {
    let mut text = raw.trim();
    let first_lower = text.to_ascii_lowercase();
    if first_lower.contains('h') && first_lower.contains('m') && first_lower.contains('s') {
        // An accessible name may be labelled (`Playback position:` in any
        // language) and may use spaces around the units. Remove the label and
        // anything before the first digit, but never touch a colon clock such
        // as `00:00:11.037`.
        if let Some((prefix, rest)) = text.split_once(':') {
            let prefix = prefix.to_ascii_lowercase();
            let rest_lower = rest.to_ascii_lowercase();
            if prefix.contains("position")
                || prefix.contains("timecode")
                || (rest_lower.contains('h')
                    && rest_lower.contains('m')
                    && rest_lower.contains('s'))
            {
                text = rest;
            }
        }
        if !text.starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(index) = text.find(|c: char| c.is_ascii_digit()) {
                text = &text[index..];
            }
        }
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains('h') || lower.contains('m') || lower.contains('s') {
        let mut total = 0.0;
        let mut start = 0usize;
        let mut parts = 0;
        let mut seconds = 0.0;
        for (i, ch) in lower.char_indices() {
            let unit = match ch {
                'h' => 'h',
                'm' => 'm',
                's' => 's',
                _ => continue,
            };
            let number: f64 = text[start..i].trim().parse().ok()?;
            if !number.is_finite() || number < 0.0 {
                return None;
            }
            match unit {
                'h' => total += number * 3600.0,
                'm' => total += number * 60.0,
                's' => seconds = number,
                _ => {}
            }
            parts += 1;
            start = i + ch.len_utf8();
        }
        if parts == 0 {
            return None;
        }
        total += seconds;
        return Some(total.max(0.0));
    }

    // Fall back to the colon clock used by the rest of the interface.
    parse_clock(text)
}

fn parse_clock(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for (i, raw) in parts.iter().enumerate() {
        let value: f64 = raw.trim().parse().ok()?;
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        total += value
            * match parts.len() - i {
                1 => 1.0,
                2 => 60.0,
                3 => 3600.0,
                _ => return None,
            };
    }
    if total.is_finite() {
        Some(total)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_audacity_spaced_timecode() {
        assert_eq!(
            parse_timecode("Playback position:00 h 00 m 11.037 s"),
            Some(11.037)
        );
        assert_eq!(
            parse_timecode("Playback position:01 h 02 m 03.004 s"),
            Some(3723.004)
        );
    }

    #[test]
    fn reads_compact_timecode() {
        assert_eq!(parse_timecode("00h00m00.000s"), Some(0.0));
        assert_eq!(parse_timecode("02h03m04.500s"), Some(7384.5));
        assert_eq!(parse_timecode("00:01:02.250"), Some(62.25));
    }

    #[test]
    fn rejects_text_without_a_clock() {
        assert_eq!(parse_timecode("Playback position:"), None);
        assert_eq!(parse_timecode("Audacity"), None);
    }

    #[test]
    fn finds_a_timecode_even_when_the_label_is_translated() {
        assert!(looks_like_timecode("Playback position:00 h 00 m 11.037 s"));
        assert!(looks_like_timecode("Reproducción:00 h 00 m 11.037 s"));
        assert!(looks_like_timecode("00:00:11.037"));
        assert!(!looks_like_timecode("Playback position:"));
        assert!(!looks_like_timecode("Timeline"));
    }

    #[test]
    fn recognises_the_hour_minute_second_display() {
        assert!(timecode_uses_hms("Playback position:00 h 00 m 11.037 s"));
        assert!(timecode_uses_hms("00h00m11.037s"));
        assert!(!timecode_uses_hms("00:00:11.037"));
    }
}
