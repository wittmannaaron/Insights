# Insights — Design Document

**Date:** 2026-05-05
**Target platform:** macOS 13+ (Apple Silicon, primary: M4)
**Status:** Approved, ready for implementation (M1 starting)

## Purpose

Native macOS desktop app showing live system telemetry (CPU, GPU, RAM, VRAM) plus Ollama-specific live metrics, with click-through to per-process drill-down. Built as a Tauri 2 shell with a vanilla JS/CSS frontend. Single-user, local-only tool.

## Key decisions (validated during brainstorming)

| Topic | Decision |
|---|---|
| Power (Watt) display | **Excluded** — `powermetrics` requires sudo. We show % utilization + RAM bytes only, no sudo needed. |
| Form factor | **Menubar tray icon + full window**. Tray popover shows mini-gauges; click opens main window. |
| Ollama telemetry | `/api/ps` polling **plus** log-tail of `~/.ollama/logs/server.log` for tokens/sec, prompt/completion counts, latency. |
| Update frequency | **Adaptive**: 1 Hz when window blurred / menubar idle, 4 Hz (250 ms) when main window focused. |
| Historical data | 60 s sparklines next to each gauge **plus** 5 min view tab. **In-memory only**, no SQLite/disk. |
| Frontend | **Vanilla JS + CSS**, no framework. SVG gauges, canvas sparklines. |
| GitHub repo | Public, no issues, no wiki, no projects, no discussions — plain code mirror. |

## Architecture

```
┌─────────────────────────────────────────┐
│  Tauri Shell (Rust)                     │
│  ├─ Tray icon + popover window          │
│  ├─ Main window (hidden by default)     │
│  └─ Sampler tasks (tokio)               │
│       ├─ CPU/RAM sampler (sysinfo)      │
│       ├─ GPU sampler (ioreg parser)     │
│       ├─ Process sampler (on-demand)    │
│       └─ Ollama sampler                 │
│            ├─ HTTP /api/ps poll         │
│            └─ Log tail server.log       │
│                                         │
│  IPC: Tauri events (push) + commands    │
└──────────────┬──────────────────────────┘
               │ window.__TAURI__.event
               ▼
┌─────────────────────────────────────────┐
│  Frontend (Vanilla JS + CSS)            │
│  ├─ SVG gauges                          │
│  ├─ Canvas sparklines                   │
│  ├─ Process drill-down modal            │
│  └─ Ollama panel (models + tps chart)   │
└─────────────────────────────────────────┘
```

## Data sources

### CPU + system RAM (no sudo)
- Crate: `sysinfo`
- Calls: `System::refresh_cpu_usage()`, `System::refresh_memory()`
- Provides: global CPU %, per-core %, used/total RAM, swap

### GPU + VRAM (Apple Silicon, no sudo)
- Primary: `ioreg -rw0 -c AGXAccelerator` parsed for `Device Utilization %` and in-use bytes
- Fallback: `system_profiler SPDisplaysDataType -json` for total VRAM
- Note: unified memory means VRAM is part of system RAM; reported separately for clarity
- `ioreg` subprocess spawned per sample (~5–10 ms overhead, acceptable)

### Per-process (on-demand only, on click)
- CPU/RAM: `sysinfo` process list, top 10 by metric
- GPU/VRAM per-process: best-effort heuristic via `ioreg` AGXCommandQueue → PID; if unavailable, modal shows "GPU per-process needs powermetrics (disabled)"

### Ollama
- HTTP poller: `GET http://localhost:11434/api/ps` every 2 s (loaded models, size_vram, expires_at)
- Log tail: tail `~/.ollama/logs/server.log` from end-of-file. Regex extracts `prompt_eval_count`, `eval_count`, `eval_duration` → tokens/sec computed

## Frontend layout

Main window ~720×520, dark theme:

```
┌─────────────────────────────────────────────┐
│  Insights                            ⚙  ✕   │
├─────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐                 │
│  │   CPU    │  │   GPU    │                 │
│  │   ◔ 47%  │  │   ◑ 82%  │                 │
│  │ ▁▂▃▅▇▅▃  │  │ ▂▃▅▇▆▄▃  │                 │
│  └──────────┘  └──────────┘                 │
│  ┌──────────┐  ┌──────────┐                 │
│  │   RAM    │  │  VRAM    │                 │
│  │ 24/64 GB │  │ 18/96 GB │                 │
│  └──────────┘  └──────────┘                 │
├─────────────────────────────────────────────┤
│  Ollama                                     │
│  ▸ qwen3:30b   18.2 GB   tps: 47.3          │
│  ┌──────── token/sec last 60s ─────────┐    │
│  └────────────────────────────────────┘    │
└─────────────────────────────────────────────┘
```

Tabs: **Live** (default) / **History 5 min** (300-point sparklines for all 4 metrics).

Tray popover (~280×200): four mini-gauges, no sparklines. Click a gauge → main window opens with that tab pre-selected.

Color thresholds: <60 % normal, 60–85 % warn (yellow), >85 % hot (red). Gauge color shifts with value.
Per-metric accent: CPU = blue, GPU = purple, RAM = green, VRAM = orange.

## Rust crate layout

```
src-tauri/src/
├── main.rs              # Tauri setup, tray, window mgmt
├── samplers/
│   ├── mod.rs           # Sampler trait + spawn loop
│   ├── cpu.rs
│   ├── gpu.rs
│   ├── processes.rs
│   └── ollama.rs
├── state.rs             # Shared AppState (Arc<Mutex<…>>)
├── ipc.rs               # #[tauri::command] handlers
└── history.rs           # Ring buffer per metric (300 pts = 5 min @ 1 Hz)
```

### Sampler trait

```rust
#[async_trait]
trait Sampler: Send + Sync {
    fn name(&self) -> &'static str;
    async fn sample(&mut self) -> Result<serde_json::Value>;
    fn interval(&self, focused: bool) -> Duration;
}
```

### IPC commands (frontend → Rust)

| Command | Payload | Returns |
|---|---|---|
| `get_top_processes` | `{kind: "cpu"\|"ram"\|"gpu"\|"vram"}` | `Vec<ProcessInfo>` |
| `get_history` | `{metric: str, range: "60s"\|"5m"}` | `Vec<{ts, value}>` |
| `get_ollama_recent` | `{limit: u32}` | `Vec<RequestRecord>` |
| `set_focus_state` | `{focused: bool}` | `()` |
| `open_main_window` | `{tab?: str}` | `()` |

### IPC events (Rust → frontend, push)

- `metrics:cpu`, `metrics:gpu`, `metrics:ram`, `metrics:vram`, `metrics:ollama`
- `ollama:request_complete` `{model, tps, prompt_tokens, completion_tokens}`

## Dependencies

**Cargo:**
- `tauri = { version = "2", features = ["tray-icon"] }`
- `sysinfo = "0.32"`
- `tokio = { version = "1", features = ["full"] }`
- `reqwest = { version = "0.12", features = ["json"] }`
- `serde`, `serde_json`, `anyhow`, `regex`, `async-trait`

**Frontend (package.json):** only `@tauri-apps/api` + `@tauri-apps/cli`. No additional framework.

## Test strategy

- **Rust unit tests:** `ioreg` output parser (fixtures in `tests/fixtures/ioreg_*.txt`), Ollama log regex (fixture).
- **Integration:** sampler loop with mock source, asserts events emitted.
- **Manual smoke:** dev build, observe gauges under load (`yes > /dev/null`, an Ollama inference).
- No E2E framework — overhead too high for a single-user tool.

## Implementation milestones

1. **M1 — Skeleton:** Tauri app boots, empty main window, tray icon present.
2. **M2 — CPU + RAM sampler** + one live SVG gauge. Verify under load.
3. **M3 — GPU + VRAM sampler** (ioreg parser + tests). Verify GPU spike during Ollama inference.
4. **M4 — Sparklines + History 60 s / 5 min view.**
5. **M5 — Process drill-down modals** (CPU/RAM first; GPU per-process if feasible).
6. **M6 — Ollama panel:** `/api/ps` + log tail + tokens/sec chart.
7. **M7 — Menubar popover** + adaptive sampling frequency.
8. **M8 — Polish:** theme, threshold colors, DMG build.

## Risks

- `ioreg` output format may vary across macOS versions → defensive parser, fall back to 0 instead of crash.
- Ollama log format can change → keep regex isolated, easy to update.
- GPU per-process on Apple Silicon without sudo is unreliable → user-visible notice in modal instead of fake data.
