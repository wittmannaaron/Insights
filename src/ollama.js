import { createSparkline } from "./sparkline.js";

function fmtBytes(b) {
  const gb = b / 1024 / 1024 / 1024;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  const mb = b / 1024 / 1024;
  return `${mb.toFixed(0)} MB`;
}

function fmtMs(ms) {
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)} s`;
  return `${ms.toFixed(0)} ms`;
}

export function mountOllamaPanel() {
  const panel = document.querySelector(".ollama-panel");
  if (!panel) return;
  const statusEl = panel.querySelector(".ollama-status");
  const modelsEl = panel.querySelector(".ollama-models");
  const activityHead = panel.querySelector(".ollama-activity-stats");
  const sparkSlot = panel.querySelector(".ollama-activity-spark");

  // 60-bucket sparkline of requests-per-second.
  const spark = createSparkline({ capacity: 60, color: "#7ee0d0", height: 32 });
  sparkSlot.appendChild(spark.el);

  // Request log: keep last 60 seconds of completed requests.
  const requestLog = []; // { ts_ms, duration_ms, path }

  let buckets = new Array(60).fill(0);
  let bucketStart = Math.floor(Date.now() / 1000);

  function rotateBuckets() {
    const nowSec = Math.floor(Date.now() / 1000);
    const advance = nowSec - bucketStart;
    if (advance <= 0) return;
    if (advance >= 60) {
      buckets.fill(0);
    } else {
      for (let i = 0; i < advance; i++) buckets.push(0);
      buckets = buckets.slice(-60);
    }
    bucketStart = nowSec;
  }

  function tick() {
    rotateBuckets();
    // Re-render sparkline from buckets snapshot.
    spark.clear();
    for (const v of buckets) spark.push(v);

    // Drop log entries older than 60 s.
    const cutoff = Date.now() - 60_000;
    while (requestLog.length && requestLog[0].ts_ms < cutoff) requestLog.shift();

    if (requestLog.length === 0) {
      activityHead.textContent = "no requests";
    } else {
      const total = requestLog.length;
      const avg = requestLog.reduce((a, b) => a + b.duration_ms, 0) / total;
      const last = requestLog[requestLog.length - 1];
      activityHead.textContent = `${total} req · avg ${fmtMs(avg)} · last ${last.path} ${fmtMs(last.duration_ms)}`;
    }
  }

  setInterval(tick, 1000);

  function renderModels(models) {
    if (!models.length) {
      modelsEl.innerHTML = `<li class="ollama-empty">No models loaded.</li>`;
      return;
    }
    modelsEl.innerHTML = models
      .map((m) => {
        const safe = String(m.name).replace(/[<>&"]/g, (c) =>
          ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;" }[c])
        );
        const expires = m.expires_at
          ? new Date(m.expires_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
          : "—";
        return `
          <li class="ollama-model">
            <span class="model-name">${safe}</span>
            <span class="model-vram">${fmtBytes(m.size_vram || m.size)}</span>
            <span class="model-expires" title="Unloads at">${expires}</span>
          </li>
        `;
      })
      .join("");
  }

  if (!window.__TAURI__) {
    statusEl.textContent = "browser preview";
    panel.dataset.reachable = "unknown";
    renderModels([]);
    return;
  }

  const { listen } = window.__TAURI__.event;

  listen("metrics:ollama_ps", (e) => {
    const { reachable, models } = e.payload;
    if (reachable) {
      panel.dataset.reachable = "yes";
      statusEl.textContent = `${models.length} model${models.length === 1 ? "" : "s"} loaded`;
    } else {
      panel.dataset.reachable = "no";
      statusEl.textContent = "offline";
    }
    renderModels(models);
  });

  listen("metrics:ollama_request", (e) => {
    const evt = e.payload;
    requestLog.push({
      ts_ms: evt.ts_ms,
      duration_ms: evt.duration_ms,
      path: evt.path,
    });
    rotateBuckets();
    buckets[buckets.length - 1] += 1;
  });
}
