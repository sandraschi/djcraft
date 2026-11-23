#![forbid(unsafe_code)]
//! Built-in opt-in LAN Multi-Touch Remote PWA server for iPad / Tablet control.
//!
//! Zero external HTTP crate dependencies: uses `std::net::TcpListener` on a background thread
//! with clean atomic shutdown, serving `touch_remote.html`, `/manifest.json`, `/api/state`,
//! and `/api/cmd`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const TOUCH_HTML: &str = include_str!("touch_remote.html");
const TOUCH_MANIFEST: &str = r##"{"name":"DJCraft Touch","short_name":"DJCraft","display":"standalone","orientation":"landscape","background_color":"#080a10","theme_color":"#38bdf8"}"##;

#[derive(Debug, Clone, Default, Serialize)]
pub struct TouchCueSnapshot {
    pub set: bool,
    pub label: String,
    pub color: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TouchDeckSnapshot {
    pub title: String,
    pub bpm: f64,
    pub playing: bool,
    pub loop4: bool,
    pub volume: f64,
    pub filter: f64,
    pub eq_low: f64,
    pub cues: [TouchCueSnapshot; 4],
}

#[derive(Debug, Clone, Serialize)]
pub struct TouchTrackRow {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub bpm: f64,
    pub key: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TouchRemoteSnapshot {
    pub deck_a: TouchDeckSnapshot,
    pub deck_b: TouchDeckSnapshot,
    pub crossfader: f64,
    pub master_volume: f64,
    pub master_muted: bool,
    pub fx1_on: bool,
    pub fx2_on: bool,
    pub seq_on: bool,
    pub tracks: Vec<TouchTrackRow>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum TouchRemoteAction {
    PlayPause { deck: String },
    Cue { deck: String },
    Sync,
    ToggleMute,
    ToggleLoop4 { deck: String },
    Beatjump { deck: String, beats: f64 },
    HotCue { deck: String, slot: usize },
    SetVolume { deck: String, value: f64 },
    SetMaster { value: f64 },
    SetCrossfader { value: f64 },
    SetFilter { deck: String, value: f64 },
    SetEqLow { deck: String, value: f64 },
    ToggleFx { unit: usize },
    SetFxWet { unit: usize, value: f64 },
    TriggerSample { bank: usize, slot: usize },
    ToggleSequencer,
    LoadTrack { deck: String, track_id: i64 },
}

pub struct TouchRemoteServer {
    #[cfg_attr(not(test), allow(dead_code))]
    pub port: u16,
    pub lan_url: String,
    pub pin_protected: bool,
    stop: Arc<AtomicBool>,
    pub snapshot: Arc<Mutex<TouchRemoteSnapshot>>,
    pub rx: Receiver<TouchRemoteAction>,
}

impl TouchRemoteServer {
    /// Start the opt-in LAN HTTP server bound to `0.0.0.0:<port>`.
    /// If `pin` is non-empty, `/api/state` and `/api/cmd` require `X-DJCraft-Pin: <pin>`.
    pub fn start(port: u16, pin: String) -> Result<Self, String> {
        let listener = TcpListener::bind(("0.0.0.0", port))
            .map_err(|e| format!("Could not bind Touch Remote on :{port}: {e}"))?;
        let actual_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        let _ = listener.set_nonblocking(true);

        let clean_pin = pin.trim().to_string();
        let pin_protected = !clean_pin.is_empty();
        let pin_arc = Arc::new(clean_pin);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_bg = Arc::clone(&stop);
        let snapshot = Arc::new(Mutex::new(TouchRemoteSnapshot::default()));
        let snap_bg = Arc::clone(&snapshot);
        let (tx, rx) = mpsc::channel::<TouchRemoteAction>();

        std::thread::Builder::new()
            .name("djcraft-touch-remote".to_string())
            .spawn(move || {
                while !stop_bg.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let snap = Arc::clone(&snap_bg);
                            let cmd_tx = tx.clone();
                            let req_pin = Arc::clone(&pin_arc);
                            let _ = std::thread::spawn(move || {
                                handle_client(stream, &snap, &cmd_tx, &req_pin);
                            });
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(25));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn Touch Remote thread: {e}"))?;

        let ip = detect_lan_ipv4().unwrap_or_else(|| "127.0.0.1".to_string());
        let lan_url = format!("http://{ip}:{actual_port}");
        Ok(Self {
            port: actual_port,
            lan_url,
            pin_protected,
            stop,
            snapshot,
            rx,
        })
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for TouchRemoteServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Pure-OS routing table lookup for this machine's primary LAN IPv4 (no packets transmitted).
pub fn detect_lan_ipv4() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let addr = sock.local_addr().ok()?;
    Some(addr.ip().to_string())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn handle_client(
    mut stream: TcpStream,
    snapshot: &Arc<Mutex<TouchRemoteSnapshot>>,
    tx: &Sender<TouchRemoteAction>,
    required_pin: &str,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });

    let mut req_line = String::new();
    if reader.read_line(&mut req_line).is_err() || req_line.is_empty() {
        return;
    }
    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1].split('?').next().unwrap_or("/");

    let mut content_len: usize = 0;
    let mut provided_pin = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse().unwrap_or(0).min(65536);
            } else if k.eq_ignore_ascii_case("x-djcraft-pin") {
                provided_pin = v.trim().to_string();
            }
        }
    }

    let pin_ok = required_pin.is_empty()
        || constant_time_eq(provided_pin.as_bytes(), required_pin.as_bytes());

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            write_response(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                TOUCH_HTML.as_bytes(),
            );
        }
        ("GET", "/manifest.json") => {
            write_response(
                &mut stream,
                "200 OK",
                "application/manifest+json",
                TOUCH_MANIFEST.as_bytes(),
            );
        }
        ("GET", "/api/state") => {
            if !pin_ok {
                write_response(
                    &mut stream,
                    "401 Unauthorized",
                    "application/json",
                    br#"{"ok":false,"auth_required":true}"#,
                );
                return;
            }
            let body = snapshot
                .lock()
                .ok()
                .and_then(|s| serde_json::to_vec(&*s).ok())
                .unwrap_or_else(|| b"{}".to_vec());
            write_response(&mut stream, "200 OK", "application/json", &body);
        }
        ("POST", "/api/cmd") => {
            if !pin_ok {
                write_response(
                    &mut stream,
                    "401 Unauthorized",
                    "application/json",
                    br#"{"ok":false,"auth_required":true}"#,
                );
                return;
            }
            let mut buf = vec![0u8; content_len];
            if content_len > 0 && reader.read_exact(&mut buf).is_err() {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "application/json",
                    br#"{"ok":false}"#,
                );
                return;
            }
            match serde_json::from_slice::<TouchRemoteAction>(&buf) {
                Ok(act) => {
                    let _ = tx.send(act);
                    write_response(&mut stream, "200 OK", "application/json", br#"{"ok":true}"#);
                }
                Err(e) => {
                    let msg = serde_json::to_vec(&Value::Object({
                        let mut m = serde_json::Map::new();
                        m.insert("ok".into(), Value::Bool(false));
                        m.insert("error".into(), Value::String(e.to_string()));
                        m
                    }))
                    .unwrap_or_else(|_| br#"{"ok":false}"#.to_vec());
                    write_response(&mut stream, "400 Bad Request", "application/json", &msg);
                }
            }
        }
        _ => {
            write_response(&mut stream, "404 Not Found", "text/plain", b"Not Found");
        }
    }
}

fn write_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_roundtrip(port: u16, req: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        s.write_all(req.as_bytes()).expect("write");
        let mut out = String::new();
        s.read_to_string(&mut out).expect("read");
        out
    }

    #[test]
    fn test_touch_remote_http_get_and_post_cmd() {
        let srv = TouchRemoteServer::start(0, String::new()).expect("bind ephemeral port");

        let html = http_roundtrip(srv.port, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(html.contains("200 OK"));
        assert!(html.contains("DJCraft Touch Remote"));

        let state_json = http_roundtrip(
            srv.port,
            "GET /api/state HTTP/1.1\r\nHost: localhost\r\n\r\n",
        );
        assert!(state_json.contains("200 OK"));
        assert!(state_json.contains("crossfader"));

        let body = r#"{"cmd":"set_crossfader","value":0.42}"#;
        let post_req = format!(
            "POST /api/cmd HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let resp = http_roundtrip(srv.port, &post_req);
        assert!(resp.contains("200 OK"));

        let act = srv
            .rx
            .recv_timeout(Duration::from_secs(2))
            .expect("received action");
        match act {
            TouchRemoteAction::SetCrossfader { value } => assert!((value - 0.42).abs() < 1e-6),
            other => panic!("unexpected action: {other:?}"),
        }

        // Verify optional PIN gate rejects unauthenticated requests and accepts matching header
        let pin_srv =
            TouchRemoteServer::start(0, "4829".to_string()).expect("bind pin-protected port");
        let unauth = http_roundtrip(
            pin_srv.port,
            "GET /api/state HTTP/1.1\r\nHost: localhost\r\n\r\n",
        );
        assert!(unauth.contains("401 Unauthorized"));
        let auth = http_roundtrip(
            pin_srv.port,
            "GET /api/state HTTP/1.1\r\nHost: localhost\r\nX-DJCraft-Pin: 4829\r\n\r\n",
        );
        assert!(auth.contains("200 OK"));
    }
}
