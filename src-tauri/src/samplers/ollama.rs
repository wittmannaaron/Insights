use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{async_runtime, AppHandle, Emitter, Runtime};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};

const PS_URL: &str = "http://127.0.0.1:11434/api/ps";
const POLL_INTERVAL_MS: u64 = 2000;

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct OllamaModel {
    pub name: String,
    pub model: String,
    pub size: u64,
    #[serde(default)]
    pub size_vram: u64,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Deserialize)]
struct PsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Serialize, Clone, Debug)]
pub struct OllamaPsSample {
    pub reachable: bool,
    pub models: Vec<OllamaModel>,
    pub ts_ms: u128,
}

#[derive(Serialize, Clone, Debug)]
pub struct OllamaRequestEvent {
    pub method: String,
    pub path: String,
    pub status: u16,
    pub duration_ms: f64,
    pub ts_ms: u128,
}

pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    let app_ps = app.clone();
    async_runtime::spawn(async move {
        ps_loop(app_ps).await;
    });
    async_runtime::spawn(async move {
        log_tail_loop(app).await;
    });
}

async fn ps_loop<R: Runtime>(app: AppHandle<R>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };
    loop {
        let now_ms = now_ms();
        let sample = match client.get(PS_URL).send().await {
            Ok(resp) => match resp.json::<PsResponse>().await {
                Ok(body) => OllamaPsSample {
                    reachable: true,
                    models: body.models,
                    ts_ms: now_ms,
                },
                Err(_) => OllamaPsSample {
                    reachable: true,
                    models: vec![],
                    ts_ms: now_ms,
                },
            },
            Err(_) => OllamaPsSample {
                reachable: false,
                models: vec![],
                ts_ms: now_ms,
            },
        };
        let _ = app.emit("metrics:ollama_ps", sample);
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

async fn log_tail_loop<R: Runtime>(app: AppHandle<R>) {
    let path = match dirs_log_path() {
        Some(p) => p,
        None => return,
    };
    let re = match gin_line_re() {
        Some(r) => r,
        None => return,
    };

    loop {
        match tokio::fs::File::open(&path).await {
            Ok(mut f) => {
                if f.seek(SeekFrom::End(0)).await.is_err() {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
                let mut reader = BufReader::new(f);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                        Ok(_) => {
                            if let Some(evt) = parse_gin_line(&re, line.trim_end()) {
                                let _ = app.emit("metrics:ollama_request", evt);
                            }
                        }
                        Err(_) => {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            break;
                        }
                    }
                }
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

fn dirs_log_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::PathBuf::from(home).join(".ollama/logs/server.log"))
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn gin_line_re() -> Option<Regex> {
    Regex::new(
        r#"\[GIN\]\s+\S+\s+-\s+\S+\s+\|\s+(\d{3})\s+\|\s+(\S+)\s+\|\s+\S+\s+\|\s+(\w+)\s+"([^"]+)""#,
    )
    .ok()
}

pub fn parse_gin_line(re: &Regex, line: &str) -> Option<OllamaRequestEvent> {
    let caps = re.captures(line)?;
    let status: u16 = caps.get(1)?.as_str().parse().ok()?;
    let dur_str = caps.get(2)?.as_str();
    let method = caps.get(3)?.as_str().to_string();
    let path = caps.get(4)?.as_str().to_string();
    let duration_ms = parse_duration_ms(dur_str)?;
    Some(OllamaRequestEvent {
        method,
        path,
        status,
        duration_ms,
        ts_ms: now_ms(),
    })
}

fn parse_duration_ms(s: &str) -> Option<f64> {
    // Handles "9.148875ms", "17.442302084s", "1m20.5s" (rare).
    let s = s.trim();
    if let Some(rest) = s.strip_suffix("ms") {
        rest.parse::<f64>().ok()
    } else if let Some(rest) = s.strip_suffix("µs").or_else(|| s.strip_suffix("us")) {
        rest.parse::<f64>().ok().map(|us| us / 1000.0)
    } else if let Some(rest) = s.strip_suffix("ns") {
        rest.parse::<f64>().ok().map(|ns| ns / 1_000_000.0)
    } else if let Some(rest) = s.strip_suffix('s') {
        rest.parse::<f64>().ok().map(|sec| sec * 1000.0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_gin_post() {
        let re = gin_line_re().unwrap();
        let line = r#"[GIN] 2026/05/05 - 19:25:15 | 200 | 17.442302084s |       127.0.0.1 | POST     "/v1/chat/completions""#;
        let evt = parse_gin_line(&re, line).unwrap();
        assert_eq!(evt.status, 200);
        assert_eq!(evt.method, "POST");
        assert_eq!(evt.path, "/v1/chat/completions");
        assert!((evt.duration_ms - 17442.302084).abs() < 1.0);
    }

    #[test]
    fn parses_gin_get_ms() {
        let re = gin_line_re().unwrap();
        let line = r#"[GIN] 2026/05/05 - 19:24:54 | 200 |    9.148875ms |       127.0.0.1 | GET      "/api/tags""#;
        let evt = parse_gin_line(&re, line).unwrap();
        assert_eq!(evt.method, "GET");
        assert!((evt.duration_ms - 9.148875).abs() < 0.01);
    }

    #[test]
    fn rejects_non_gin_line() {
        let re = gin_line_re().unwrap();
        assert!(parse_gin_line(&re, "time=2026 INFO foo").is_none());
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration_ms("100ms"), Some(100.0));
        assert_eq!(parse_duration_ms("1.5s"), Some(1500.0));
        assert_eq!(parse_duration_ms("500us"), Some(0.5));
        assert_eq!(parse_duration_ms("500µs"), Some(0.5));
    }
}
