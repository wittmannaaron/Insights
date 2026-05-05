# Insights

Native macOS desktop app for live system telemetry — CPU, GPU, RAM, VRAM, and Ollama-specific metrics — with click-through to per-process drill-down. Tauri 2 shell, vanilla JS/CSS frontend.

Apple Silicon, macOS 13+. Public, MIT.

See [`docs/plans/2026-05-05-insights-design.md`](docs/plans/2026-05-05-insights-design.md) for the design.

## Status

M1–M6b complete. Live CPU/GPU/RAM/VRAM gauges, sparklines, history view, process drilldown, Ollama panel with reverse-proxy-derived tokens/sec.

## Ollama tokens/sec

Insights ships a transparent reverse proxy on `127.0.0.1:11435` that forwards to the local Ollama server on `127.0.0.1:11434`. Inference responses (`/api/generate`, `/api/chat`, `/v1/chat/completions`) are inspected for token counts and durations to derive eval and prompt tokens/sec.

To see TPS in the panel, point your Ollama clients at the proxy:

```bash
export OLLAMA_HOST=http://127.0.0.1:11435
```

The proxy is streaming-transparent: clients still receive incremental NDJSON / SSE chunks unchanged. If port 11435 is busy the proxy disables itself — check the Tauri dev log for the bind error.

## Build (dev)

```bash
npm install
npm run tauri dev
```

## Build (release)

```bash
npm run tauri build
```
