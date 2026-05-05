# Insights

Native macOS desktop app for live system telemetry — CPU, GPU, RAM, VRAM, and Ollama-specific metrics — with click-through to per-process drill-down. Tauri 2 shell, vanilla JS/CSS frontend.

Apple Silicon, macOS 13+. Public, MIT.

See [`docs/plans/2026-05-05-insights-design.md`](docs/plans/2026-05-05-insights-design.md) for the design.

## Status

v0.1 complete (M1–M8). Live CPU/GPU/RAM/VRAM gauges with 60 s sparklines and 5 min history view, click-to-drill process modal, Ollama panel with reverse-proxy-derived tokens/sec, tray popover with mini-gauges, adaptive sampling (250 ms focused / 1000 ms idle).

## Ollama tokens/sec

Insights ships a transparent reverse proxy that takes over the default Ollama port `127.0.0.1:11434` and forwards to a relocated Ollama on `127.0.0.1:11435`. Inference responses (`/api/generate`, `/api/chat`, `/v1/chat/completions`) are inspected for token counts and durations to derive eval and prompt tokens/sec.

After this swap **every Ollama client works unchanged** — they all hit `:11434` by default and get the live token-per-second instrumentation for free.

### One-time setup

Run the helper script once. It edits `~/Library/LaunchAgents/environment.plist`, sets `OLLAMA_HOST=127.0.0.1:11435` in the launchctl `gui/<uid>` domain, restarts the LaunchAgent and `Ollama.app`:

```bash
bash scripts/configure-ollama-port.sh
```

Verify:

```bash
launchctl getenv OLLAMA_HOST    # → 127.0.0.1:11435
curl -s http://127.0.0.1:11435/api/tags | head -c 80   # Ollama on 11435
curl -s http://127.0.0.1:11434/api/tags | head -c 80   # Insights proxy on 11434
```

To revert (restore default port for Ollama):

```bash
bash scripts/configure-ollama-port.sh --revert
```

### Auto-start

Insights enables a macOS LaunchAgent on first run via `tauri-plugin-autostart`, so the proxy comes back up after every login. Disable in `System Settings → General → Login Items` if not wanted.

### Notes

- Proxy is streaming-transparent: NDJSON / SSE chunks pass through untouched, only inspected.
- If port 11434 is already taken (e.g. Ollama still bound there because the setup script hasn't run), the proxy logs the bind error and disables itself; the rest of the app keeps working.
- Remove any leftover `export OLLAMA_HOST=...` from `.zshrc` / `.bashrc` from earlier setups.

## Build (dev)

```bash
npm install
npm run tauri dev
```

## Build (release)

```bash
npm run tauri build
```

Outputs `src-tauri/target/release/bundle/macos/Insights.app` and a DMG installer at `src-tauri/target/release/bundle/dmg/Insights_<version>_aarch64.dmg`.

## Limitations

- **Per-process GPU usage** requires `powermetrics` (sudo). The drilldown modal shows a notice instead. By design — see the design doc.
- **Watt readings** also require `powermetrics`. Excluded for the same reason.
- **Ollama tokens/sec** only appears when clients are pointed at the proxy on `127.0.0.1:11435`. Unproxied requests still show as load in the request-rate sparkline but contribute no TPS data.
