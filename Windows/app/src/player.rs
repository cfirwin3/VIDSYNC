use macroquad::prelude::*;
use serde_json::Value;
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;

pub const PLAY_FPS: f64 = 30.0;
const MAX_W: f64 = 1280.0;
const MAX_H: f64 = 720.0;

pub fn tool(name: &str) -> String {
    let env_name = format!("CUE_{}", name.to_uppercase());
    if let Ok(p) = std::env::var(&env_name) {
        if !p.trim().is_empty() {
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // On Windows the sidecar programs are ffmpeg.exe / ffprobe.exe.
            // Accept both a flat layout and a tools\ subfolder, which is handy
            // when the player is packaged as a folder or an installer payload.
            let file_name = if cfg!(windows) && !name.to_ascii_lowercase().ends_with(".exe") {
                format!("{name}.exe")
            } else {
                name.to_string()
            };
            for candidate in [dir.join(&file_name), dir.join("tools").join(&file_name)] {
                if candidate.is_file() {
                    return candidate.to_string_lossy().into_owned();
                }
            }
        }
    }
    if cfg!(windows) && !name.to_ascii_lowercase().ends_with(".exe") {
        // Command::new("ffmpeg.exe") uses the Windows executable search path,
        // so an ffmpeg installed with PATH keeps working too.
        return format!("{name}.exe");
    }
    name.to_string()
}

/// Build a child process for a media tool. Windows launches are hidden so a
/// GUI player never flashes a console window for each seek.
fn command(name: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(tool(name));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub path: PathBuf,
    pub name: String,
    pub duration: f64,
    pub frame_w: u32,
    pub frame_h: u32,
}

struct Decoder {
    child: Arc<Mutex<Child>>,
    rx: Receiver<Vec<u8>>,
}

impl Decoder {
    fn kill(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        self.kill();
    }
}

pub struct Player {
    pub media: Option<MediaInfo>,
    pub position: f64,
    pub playing: bool,
    pub loading: bool,
    pub has_frame: bool,
    pub status: String,
    texture: Option<Texture2D>,
    decoder: Option<Decoder>,
    pending: VecDeque<(u64, Vec<u8>)>,
    next_index: u64,
    stream_start: f64,
    preview_rx: Option<Receiver<Vec<u8>>>,
}

impl Player {
    pub fn new() -> Self {
        Player {
            media: None,
            position: 0.0,
            playing: false,
            loading: false,
            has_frame: false,
            status: String::new(),
            texture: None,
            decoder: None,
            pending: VecDeque::new(),
            next_index: 0,
            stream_start: 0.0,
            preview_rx: None,
        }
    }

    pub fn duration(&self) -> f64 {
        self.media.as_ref().map(|m| m.duration).unwrap_or(0.0)
    }

    pub fn texture(&self) -> Option<&Texture2D> {
        self.texture.as_ref()
    }

    pub fn aspect(&self) -> f32 {
        self.media
            .as_ref()
            .map(|m| m.frame_w as f32 / m.frame_h.max(1) as f32)
            .unwrap_or(16.0 / 9.0)
    }

    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let info = probe(path)?;
        self.stop_decoder();
        self.preview_rx = None;
        self.media = Some(info);
        self.position = 0.0;
        self.playing = false;
        self.loading = true;
        self.has_frame = false;
        self.texture = None;
        self.pending.clear();
        self.next_index = 0;
        self.status.clear();
        self.request_preview();
        Ok(())
    }

    fn start_stream(&mut self) {
        self.preview_rx = None;
        let media = match self.media.as_ref() {
            Some(m) => m,
            None => return,
        };
        let frame_bytes = (media.frame_w * media.frame_h * 4) as usize;
        let filter = format!(
            "scale={}:{}:flags=bicubic,setsar=1,format=rgba",
            media.frame_w, media.frame_h
        );
        let mut cmd = command("ffmpeg");
        cmd.arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-nostdin")
            .arg("-ss")
            .arg(format!("{:.3}", self.position))
            .arg("-i")
            .arg(&media.path)
            .arg("-an")
            .arg("-sn")
            .arg("-dn")
            .arg("-vf")
            .arg(filter)
            .arg("-r")
            .arg("30")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("rgba")
            .arg("pipe:1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        match cmd.spawn() {
            Ok(mut child) => {
                let stdout = match child.stdout.take() {
                    Some(s) => s,
                    None => {
                        let _ = child.kill();
                        let _ = child.wait();
                        self.status = "Could not read video output".to_string();
                        return;
                    }
                };
                let child = Arc::new(Mutex::new(child));
                let (tx, rx) = sync_channel::<Vec<u8>>(8);
                thread::spawn(move || {
                    let mut stdout = stdout;
                    loop {
                        let mut buf = vec![0u8; frame_bytes];
                        match stdout.read_exact(&mut buf) {
                            Ok(()) => {
                                if tx.send(buf).is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
                self.decoder = Some(Decoder { child, rx });
                self.stream_start = self.position;
                self.next_index = 0;
                self.playing = true;
                self.loading = true;
                self.status.clear();
            }
            Err(e) => {
                self.playing = false;
                self.status = format!("Could not start ffmpeg: {e}");
            }
        }
    }

    /// Follow an external clock (Audacity's timecode). When the clock moves,
    /// keep the decoder running from a nearby position; when it jumps, restart
    /// the decode stream at the new position.
    pub fn follow(&mut self, target: f64) {
        if self.media.is_none() {
            return;
        }
        let dur = self.duration();
        let t = if dur > 0.0 {
            target.clamp(0.0, dur)
        } else {
            target.max(0.0)
        };
        // Audacity keeps running past the end of a short clip. Park on the
        // last frame instead of respawning ffmpeg every timecode tick.
        if dur > 0.0 && t >= dur - 0.001 {
            let changed = (self.position - dur).abs() > 0.02;
            if self.playing {
                self.stop_decoder();
                self.pending.clear();
                self.playing = false;
            }
            self.position = dur;
            if changed || !self.has_frame {
                self.request_preview();
            }
            return;
        }
        let drift = t - self.position;
        let far = drift.abs() > 0.4;
        let behind_stream = t < self.stream_start - 0.05;
        if !self.playing || far || behind_stream {
            self.stop_decoder();
            self.pending.clear();
            self.position = t;
            self.start_stream();
        } else {
            self.position = t;
        }
    }

    /// Hold the picture on an external clock position while Audacity is
    /// paused. The decoder is stopped; only a still frame is fetched.
    pub fn hold(&mut self, target: f64) {
        if self.media.is_none() {
            return;
        }
        let dur = self.duration();
        let t = if dur > 0.0 {
            target.clamp(0.0, dur)
        } else {
            target.max(0.0)
        };
        let changed = (self.position - t).abs() > 0.02;
        if self.playing {
            self.stop_decoder();
            self.pending.clear();
            self.playing = false;
        }
        self.position = t;
        if changed {
            self.request_preview();
        }
    }

    fn stop_decoder(&mut self) {
        if let Some(mut d) = self.decoder.take() {
            d.kill();
        }
    }

    fn request_preview(&mut self) {
        self.preview_rx = None;
        let media = match self.media.as_ref() {
            Some(m) => m,
            None => return,
        };
        self.loading = true;
        let frame_bytes = (media.frame_w * media.frame_h * 4) as usize;
        let filter = format!(
            "scale={}:{}:flags=bicubic,setsar=1,format=rgba",
            media.frame_w, media.frame_h
        );
        let path = media.path.clone();
        let at = self.position;
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        thread::spawn(move || {
            let mut cmd = command("ffmpeg");
            cmd.arg("-hide_banner")
                .arg("-loglevel")
                .arg("error")
                .arg("-nostdin")
                .arg("-ss")
                .arg(format!("{:.3}", at))
                .arg("-i")
                .arg(&path)
                .arg("-an")
                .arg("-sn")
                .arg("-dn")
                .arg("-vf")
                .arg(filter)
                .arg("-frames:v")
                .arg("1")
                .arg("-f")
                .arg("rawvideo")
                .arg("-pix_fmt")
                .arg("rgba")
                .arg("pipe:1")
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            if let Ok(mut child) = cmd.spawn() {
                if let Some(mut out) = child.stdout.take() {
                    let mut buf = vec![0u8; frame_bytes];
                    if out.read_exact(&mut buf).is_ok() {
                        let _ = tx.send(buf);
                    }
                }
                let _ = child.wait();
            }
        });
        self.preview_rx = Some(rx);
    }

    fn upload(&mut self, bytes: &[u8]) {
        let (w, h) = match self.media.as_ref() {
            Some(m) => (m.frame_w, m.frame_h),
            None => return,
        };
        if bytes.len() < (w * h * 4) as usize {
            return;
        }
        let needs_new = match self.texture.as_ref() {
            Some(t) => {
                let s = t.size();
                s.x as u32 != w || s.y as u32 != h
            }
            None => true,
        };
        if needs_new {
            let tex = Texture2D::from_rgba8(w as u16, h as u16, &bytes[..(w * h * 4) as usize]);
            tex.set_filter(FilterMode::Linear);
            self.texture = Some(tex);
        } else if let Some(tex) = self.texture.as_ref() {
            tex.update_from_bytes(w, h, &bytes[..(w * h * 4) as usize]);
        }
        self.has_frame = true;
        self.loading = false;
    }

    pub fn update(&mut self, dt: f64) {
        if let Some(rx) = self.preview_rx.as_ref() {
            if let Ok(bytes) = rx.try_recv() {
                self.preview_rx = None;
                self.upload(&bytes);
            }
        }

        if !self.playing {
            return;
        }

        let dur = self.duration();
        if dur > 0.0 && self.position >= dur {
            self.position = dur;
            self.playing = false;
            self.stop_decoder();
            self.pending.clear();
            return;
        }

        self.position += dt;
        if dur > 0.0 && self.position > dur {
            self.position = dur;
        }

        let mut disconnected = false;
        if let Some(decoder) = self.decoder.as_ref() {
            // Keep the *earliest* frames, not the newest. ffmpeg decodes as
            // fast as it can, so draining everything would throw away the
            // frames the clock is actually at and leave only frames from
            // seconds in the future. A small queue lets the decoder run just
            // ahead of the clock and then blocks it on its channel.
            while self.pending.len() < 12 {
                match decoder.rx.try_recv() {
                    Ok(bytes) => {
                        self.pending.push_back((self.next_index, bytes));
                        self.next_index += 1;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        // Show the newest frame whose timestamp has been reached.
        loop {
            let due = match self.pending.front() {
                Some((idx, _)) => {
                    self.stream_start + *idx as f64 / PLAY_FPS <= self.position + 0.045
                }
                None => false,
            };
            if !due {
                break;
            }
            if let Some((_, bytes)) = self.pending.pop_front() {
                self.upload(&bytes);
            }
        }

        if disconnected && self.pending.is_empty() {
            self.playing = false;
            if dur > 0.0 {
                self.position = dur;
            }
            self.stop_decoder();
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.preview_rx = None;
        self.stop_decoder();
    }
}

// ---------------------------------------------------------------- probing
fn ratio(text: &str) -> Option<f64> {
    let mut it = text.split(':');
    let a: f64 = it.next()?.trim().parse().ok()?;
    let b: f64 = it.next()?.trim().parse().ok()?;
    if b.abs() < f64::EPSILON || a <= 0.0 {
        None
    } else {
        Some(a / b)
    }
}

fn number(v: &Value, key: &str) -> Option<f64> {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn rotation_of(stream: &Value) -> f64 {
    if let Some(list) = stream.get("side_data_list").and_then(|v| v.as_array()) {
        for item in list {
            if let Some(r) = number(item, "rotation") {
                return r;
            }
        }
    }
    if let Some(tags) = stream.get("tags") {
        if let Some(r) = number(tags, "rotate") {
            return r;
        }
    }
    0.0
}

pub fn probe(path: &Path) -> Result<MediaInfo, String> {
    if !path.is_file() {
        return Err(format!(
            "That file could not be found: {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    let output = command("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()
        .map_err(|e| {
            format!("Could not start ffprobe ({e}). Install ffmpeg or bundle it with the player.")
        })?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let first = err
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("unknown error");
        return Err(format!("This file could not be read: {}", first.trim()));
    }
    let root: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Unexpected ffprobe output".to_string())?;

    let streams = root
        .get("streams")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "No streams found in this file".to_string())?;
    let stream = streams
        .iter()
        .find(|s| {
            s.get("codec_type").and_then(|v| v.as_str()) == Some("video")
                && s.get("disposition")
                    .and_then(|d| d.get("attached_pic"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    != 1
        })
        .ok_or_else(|| "No video track was found in this file".to_string())?;

    let width = stream.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let height = stream.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err("The video track has an unknown size".to_string());
    }

    let sar = stream
        .get("sample_aspect_ratio")
        .and_then(|v| v.as_str())
        .and_then(ratio)
        .unwrap_or(1.0);
    let rotation = rotation_of(stream);
    let (mut vw, mut vh) = (width as f64 * sar, height as f64);
    if rotation.abs() % 180.0 > 45.0 {
        std::mem::swap(&mut vw, &mut vh);
    }
    let scale = (MAX_W / vw).min(MAX_H / vh).min(1.0);
    let mut frame_w = ((vw * scale) / 2.0).round().max(1.0) as u32 * 2;
    let mut frame_h = ((vh * scale) / 2.0).round().max(1.0) as u32 * 2;
    frame_w = frame_w.max(2);
    frame_h = frame_h.max(2);

    let format_duration = root
        .get("format")
        .and_then(|f| number(f, "duration"))
        .unwrap_or(0.0);
    let stream_duration = number(stream, "duration").unwrap_or(0.0);
    let duration = if format_duration > 0.0 {
        format_duration
    } else {
        stream_duration
    };

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "video".to_string());

    Ok(MediaInfo {
        path: path.to_path_buf(),
        name,
        duration,
        frame_w,
        frame_h,
    })
}
