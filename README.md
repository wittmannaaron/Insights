# Insights

Native macOS desktop app for live system telemetry — CPU, GPU, RAM, VRAM, and Ollama-specific metrics — with click-through to per-process drill-down. Tauri 2 shell, vanilla JS/CSS frontend.

Apple Silicon, macOS 13+. Public, MIT.

See [`docs/plans/2026-05-05-insights-design.md`](docs/plans/2026-05-05-insights-design.md) for the design.

## Status

M1 (skeleton) in progress. Tracker: design doc § Implementation milestones.

## Build (dev)

```bash
npm install
npm run tauri dev
```

## Build (release)

```bash
npm run tauri build
```
