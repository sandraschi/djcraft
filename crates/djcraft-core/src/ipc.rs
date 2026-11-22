#![forbid(unsafe_code)]
//! Loopback JSON-over-TCP IPC for live remote control of a running `djcraft.exe` GUI.
//!
//! `djcraft.exe` binds `127.0.0.1:0` (ephemeral port) and writes `%LOCALAPPDATA%\djcraft\live.port`.
//! `djcraft-cli` and `djcraft-mcp` connect to that port to dispatch `Command`s, load audio files,
//! and query live 4-deck + mixer + audio state.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use djcraft_audio::DeckId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Command;

pub fn parse_deck(s: &str) -> Result<DeckId, String> {
    match s.trim().to_lowercase().as_str() {
        "a" | "1" | "decka" | "deck_a" => Ok(DeckId::DeckA),
        "b" | "2" | "deckb" | "deck_b" => Ok(DeckId::DeckB),
        "c" | "3" | "deckc" | "deck_c" => Ok(DeckId::DeckC),
        "d" | "4" | "deckd" | "deck_d" => Ok(DeckId::DeckD),
        other => Err(format!("Invalid deck '{other}' (expected a/b/c/d or 1..4)")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum IpcRequest {
    Command { command: Command },
    LoadFile { deck: DeckId, path: String },
    LiveStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

impl IpcResponse {
    pub fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: Value::Null,
            error: Some(msg.into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LivePortInfo {
    port: u16,
    pid: u32,
}

pub fn live_port_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)?;
    Some(base.join("djcraft").join("live.port"))
}

pub type IpcReceiver = mpsc::Receiver<(IpcRequest, mpsc::SyncSender<IpcResponse>)>;

/// Start a loopback TCP IPC server on `127.0.0.1:0` and record its port in `live.port`.
pub fn start_ipc_server(wake: impl Fn() + Send + 'static) -> Result<(IpcReceiver, u16), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("IPC bind failed: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("IPC local_addr failed: {e}"))?
        .port();

    if let Some(path) = live_port_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let info = LivePortInfo {
            port,
            pid: std::process::id(),
        };
        if let Ok(json) = serde_json::to_string(&info) {
            let _ = std::fs::write(&path, json);
        }
    }

    let (tx, rx) = mpsc::channel::<(IpcRequest, mpsc::SyncSender<IpcResponse>)>();

    std::thread::spawn(move || {
        for stream_res in listener.incoming() {
            let Ok(mut stream) = stream_res else {
                break;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

            let mut line = String::new();
            let read_ok = {
                let mut reader = BufReader::new(&stream);
                reader.read_line(&mut line).is_ok()
            };
            if !read_ok || line.trim().is_empty() {
                continue;
            }

            let resp = match serde_json::from_str::<IpcRequest>(line.trim()) {
                Ok(req) => {
                    let (reply_tx, reply_rx) = mpsc::sync_channel::<IpcResponse>(1);
                    if tx.send((req, reply_tx)).is_err() {
                        IpcResponse::err("DJCraft GUI event loop closed")
                    } else {
                        wake();
                        match reply_rx.recv_timeout(Duration::from_secs(4)) {
                            Ok(r) => r,
                            Err(_) => {
                                IpcResponse::err("DJCraft GUI timed out handling IPC request")
                            }
                        }
                    }
                }
                Err(e) => IpcResponse::err(format!("Invalid IPC request JSON: {e}")),
            };

            if let Ok(json_resp) = serde_json::to_string(&resp) {
                let _ = writeln!(stream, "{json_resp}");
                let _ = stream.flush();
            }
        }
    });

    Ok((rx, port))
}

/// Send an IPC request to the running `djcraft.exe` GUI via `live.port`.
pub fn send_ipc_request(req: &IpcRequest) -> Result<IpcResponse, String> {
    let path =
        live_port_path().ok_or_else(|| "Cannot resolve config dir for live.port".to_string())?;
    let raw = std::fs::read_to_string(&path).map_err(|_| {
        format!(
            "DJCraft GUI is not running (missing {}). Start djcraft.exe first.",
            path.display()
        )
    })?;
    let info: LivePortInfo = serde_json::from_str(&raw)
        .map_err(|e| format!("Corrupt live.port {}: {e}", path.display()))?;
    send_ipc_request_to_port(info.port, req)
}

pub fn send_ipc_request_to_port(port: u16, req: &IpcRequest) -> Result<IpcResponse, String> {
    let addr: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .map_err(|e| format!("Invalid socket addr: {e}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(1)).map_err(|e| {
        format!(
            "Cannot connect to running DJCraft GUI on 127.0.0.1:{port} ({e}). Is djcraft.exe running?"
        )
    })?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let payload =
        serde_json::to_string(req).map_err(|e| format!("Serialize IPC request failed: {e}"))?;
    writeln!(stream, "{payload}").map_err(|e| format!("IPC write failed: {e}"))?;
    stream
        .flush()
        .map_err(|e| format!("IPC flush failed: {e}"))?;

    let mut line = String::new();
    let mut reader = BufReader::new(&stream);
    reader
        .read_line(&mut line)
        .map_err(|e| format!("IPC read failed: {e}"))?;
    serde_json::from_str::<IpcResponse>(line.trim())
        .map_err(|e| format!("Invalid IPC response JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_roundtrip_over_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut line = String::new();
                let _ = BufReader::new(&stream).read_line(&mut line);
                let req: IpcRequest = serde_json::from_str(line.trim()).expect("parse req");
                let resp = match req {
                    IpcRequest::LiveStatus => IpcResponse::ok(serde_json::json!({"alive": true})),
                    _ => IpcResponse::err("unexpected"),
                };
                let _ = writeln!(stream, "{}", serde_json::to_string(&resp).expect("ser"));
            }
        });

        let resp = send_ipc_request_to_port(port, &IpcRequest::LiveStatus).expect("send");
        assert!(resp.ok);
        assert_eq!(resp.data.get("alive").and_then(|v| v.as_bool()), Some(true));
        let _ = handle.join();
    }
}
