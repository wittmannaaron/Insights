use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::net::TcpListener;

const UPSTREAM: &str = "http://127.0.0.1:11434";
const LISTEN_ADDR: &str = "127.0.0.1:11435";

// Headers that must not be forwarded verbatim.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
];

#[derive(Clone)]
struct ProxyState<R: Runtime> {
    client: reqwest::Client,
    app: AppHandle<R>,
}

#[derive(Serialize, Clone, Debug)]
pub struct OllamaInferenceMetric {
    pub model: String,
    pub path: String,
    pub prompt_eval_count: Option<u64>,
    pub eval_count: Option<u64>,
    pub eval_duration_ns: Option<u64>,
    pub prompt_eval_duration_ns: Option<u64>,
    pub total_duration_ns: Option<u64>,
    pub eval_tps: Option<f64>,
    pub prompt_tps: Option<f64>,
    pub ts_ms: u128,
}

pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(app).await {
            eprintln!("[insights/proxy] exited: {e}");
        }
    });
}

async fn run<R: Runtime>(app: AppHandle<R>) -> std::io::Result<()> {
    let client = match reqwest::Client::builder()
        .pool_max_idle_per_host(0)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[insights/proxy] could not build http client: {e}");
            return Ok(());
        }
    };
    let state = Arc::new(ProxyState { client, app });
    let app_router: Router = Router::new()
        .fallback(any(forward::<R>))
        .with_state(state);

    let listener = match TcpListener::bind(LISTEN_ADDR).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[insights/proxy] cannot bind {LISTEN_ADDR}: {e} — proxy disabled");
            return Ok(());
        }
    };
    eprintln!("[insights/proxy] listening on {LISTEN_ADDR} -> {UPSTREAM}");
    axum::serve(listener, app_router).await
}

async fn forward<R: Runtime>(
    State(state): State<Arc<ProxyState<R>>>,
    req: Request,
) -> Response {
    let (parts, body) = req.into_parts();
    let method = parts.method.clone();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| parts.uri.path().to_string());
    let url = format!("{}{}", UPSTREAM, path_and_query);

    let body_bytes = match http_body_util::BodyExt::collect(body).await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_GATEWAY, format!("read body: {e}")),
    };

    let mut req_builder = state
        .client
        .request(method.clone(), &url)
        .body(body_bytes.to_vec());
    for (name, value) in parts.headers.iter() {
        if HOP_BY_HOP
            .iter()
            .any(|h| h.eq_ignore_ascii_case(name.as_str()))
        {
            continue;
        }
        req_builder = req_builder.header(name.as_str(), value);
    }

    let upstream_resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            return error_response(StatusCode::BAD_GATEWAY, format!("upstream {url}: {e}"))
        }
    };

    let status = upstream_resp.status();
    let upstream_headers = upstream_resp.headers().clone();
    let mut downstream_headers = HeaderMap::new();
    for (name, value) in upstream_headers.iter() {
        if HOP_BY_HOP
            .iter()
            .any(|h| h.eq_ignore_ascii_case(name.as_str()))
        {
            continue;
        }
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            downstream_headers.append(n, v);
        }
    }

    let path_for_parse = parts.uri.path().to_string();
    let app = state.app.clone();
    let mut upstream_stream = upstream_resp.bytes_stream();

    let body_stream = async_stream::stream! {
        let mut buf: Vec<u8> = Vec::new();
        let mut emitted = false;
        while let Some(chunk_res) = upstream_stream.next().await {
            match chunk_res {
                Ok(chunk) => {
                    if !emitted {
                        buf.extend_from_slice(&chunk);
                        if let Some(metric) = extract_metric(&buf, &path_for_parse) {
                            let _ = app.emit("metrics:ollama_tps", metric);
                            emitted = true;
                            buf.clear();
                            buf.shrink_to_fit();
                        } else if buf.len() > 4 * 1024 * 1024 {
                            // Cap inspect buffer to 4 MB; stop trying.
                            emitted = true;
                            buf.clear();
                            buf.shrink_to_fit();
                        }
                    }
                    yield Ok::<_, std::io::Error>(chunk);
                }
                Err(e) => {
                    yield Err(std::io::Error::new(std::io::ErrorKind::Other, e));
                    return;
                }
            }
        }
    };

    let body = Body::from_stream(body_stream);
    let mut response = Response::builder().status(status);
    if let Some(headers) = response.headers_mut() {
        *headers = downstream_headers;
    }
    response.body(body).unwrap_or_else(|_| {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "build response".into())
    })
}

fn error_response(code: StatusCode, msg: String) -> Response {
    Response::builder()
        .status(code)
        .body(Body::from(msg))
        .unwrap()
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[derive(Deserialize)]
struct OllamaDoneEvent {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    done: Option<bool>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    eval_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_duration: Option<u64>,
    #[serde(default)]
    total_duration: Option<u64>,
}

#[derive(Deserialize)]
struct OpenAIChatResponse {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Deserialize)]
struct OpenAIUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

/// Extract token-count metrics if the buffer contains the final inference
/// statistics. Supports two response shapes:
///
/// - **NDJSON streaming** (`/api/generate`, `/api/chat`): the last NDJSON
///   object with `"done": true` carries `eval_count`, `eval_duration`, etc.
/// - **OpenAI JSON** (`/v1/chat/completions` non-stream): a single JSON
///   document with a `usage` block.
pub fn extract_metric(buf: &[u8], path: &str) -> Option<OllamaInferenceMetric> {
    if path == "/api/generate" || path == "/api/chat" {
        return extract_ndjson_metric(buf, path);
    }
    if path == "/v1/chat/completions" || path == "/v1/completions" {
        return extract_openai_metric(buf, path);
    }
    None
}

fn extract_ndjson_metric(buf: &[u8], path: &str) -> Option<OllamaInferenceMetric> {
    // Scan from the end for the most recent newline-delimited JSON line that
    // parses with `done: true`.
    let mut end = buf.len();
    while end > 0 {
        let slice_end = end;
        // find prior newline
        let start = buf[..slice_end]
            .iter()
            .rposition(|&b| b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line = &buf[start..slice_end].trim_ascii_end();
        if !line.is_empty() {
            if let Ok(evt) = serde_json::from_slice::<OllamaDoneEvent>(line) {
                if evt.done == Some(true) && evt.eval_count.is_some() {
                    let eval_tps = evt
                        .eval_count
                        .zip(evt.eval_duration)
                        .filter(|(_, d)| *d > 0)
                        .map(|(c, d)| c as f64 / (d as f64 / 1_000_000_000.0));
                    let prompt_tps = evt
                        .prompt_eval_count
                        .zip(evt.prompt_eval_duration)
                        .filter(|(_, d)| *d > 0)
                        .map(|(c, d)| c as f64 / (d as f64 / 1_000_000_000.0));
                    return Some(OllamaInferenceMetric {
                        model: evt.model.unwrap_or_default(),
                        path: path.to_string(),
                        prompt_eval_count: evt.prompt_eval_count,
                        eval_count: evt.eval_count,
                        eval_duration_ns: evt.eval_duration,
                        prompt_eval_duration_ns: evt.prompt_eval_duration,
                        total_duration_ns: evt.total_duration,
                        eval_tps,
                        prompt_tps,
                        ts_ms: now_ms(),
                    });
                }
            }
        }
        if start == 0 {
            break;
        }
        end = start - 1;
    }
    None
}

fn extract_openai_metric(buf: &[u8], path: &str) -> Option<OllamaInferenceMetric> {
    let resp = serde_json::from_slice::<OpenAIChatResponse>(buf).ok()?;
    let usage = resp.usage?;
    Some(OllamaInferenceMetric {
        model: resp.model.unwrap_or_default(),
        path: path.to_string(),
        prompt_eval_count: usage.prompt_tokens,
        eval_count: usage.completion_tokens,
        eval_duration_ns: None,
        prompt_eval_duration_ns: None,
        total_duration_ns: None,
        eval_tps: None,
        prompt_tps: None,
        ts_ms: now_ms(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndjson_done_with_eval_counts() {
        let body = br#"{"model":"qwen3:30b","created_at":"2026-05-05T00:00:00Z","response":"hello","done":false}
{"model":"qwen3:30b","created_at":"2026-05-05T00:00:00Z","response":" world","done":false}
{"model":"qwen3:30b","created_at":"2026-05-05T00:00:00Z","done":true,"prompt_eval_count":12,"prompt_eval_duration":50000000,"eval_count":47,"eval_duration":1000000000,"total_duration":1080000000}
"#;
        let m = extract_metric(body, "/api/chat").expect("metric");
        assert_eq!(m.model, "qwen3:30b");
        assert_eq!(m.eval_count, Some(47));
        assert_eq!(m.prompt_eval_count, Some(12));
        // 47 tokens / 1.0 s = 47 tps
        assert!((m.eval_tps.unwrap() - 47.0).abs() < 0.001);
        // 12 tokens / 0.05 s = 240 tps
        assert!((m.prompt_tps.unwrap() - 240.0).abs() < 0.01);
    }

    #[test]
    fn openai_usage_block() {
        let body = br#"{"id":"chatcmpl-1","object":"chat.completion","model":"qwen3:30b","choices":[{"message":{"role":"assistant","content":"hi"}}],"usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17}}"#;
        let m = extract_metric(body, "/v1/chat/completions").expect("metric");
        assert_eq!(m.model, "qwen3:30b");
        assert_eq!(m.eval_count, Some(5));
        assert_eq!(m.prompt_eval_count, Some(12));
        assert!(m.eval_tps.is_none());
    }

    #[test]
    fn ndjson_without_done_returns_none() {
        let body = br#"{"model":"qwen3","done":false,"response":"x"}
"#;
        assert!(extract_metric(body, "/api/chat").is_none());
    }

    #[test]
    fn unrelated_path_returns_none() {
        assert!(extract_metric(b"{}", "/api/tags").is_none());
    }
}
