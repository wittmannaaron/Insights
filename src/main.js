import { createGauge } from "./gauge.js";
import { createSparkline } from "./sparkline.js";
import { showProcessModal } from "./modal.js";

const ACCENTS = {
  cpu: "#4ea3ff",
  gpu: "#b48cff",
  ram: "#6ce3a3",
  vram: "#ffb572",
};

const METRICS = ["cpu", "gpu", "ram", "vram"];

// Per-metric: gauge, card sparkline (60s), history sparkline (300pts/5min), sub elements.
const view = {};

function mount() {
  for (const metric of METRICS) {
    const card = document.querySelector(`.view-live .card[data-metric="${metric}"]`);
    const gauge = createGauge({ accent: ACCENTS[metric] });
    card.querySelector(".gauge-slot").appendChild(gauge.el);

    const cardSpark = createSparkline({ capacity: 60, color: ACCENTS[metric], height: 28 });
    card.querySelector(".spark-slot").appendChild(cardSpark.el);

    const histRow = document.querySelector(`.view-history .history-row[data-metric="${metric}"]`);
    const histSpark = createSparkline({ capacity: 300, color: ACCENTS[metric], height: 56 });
    histRow.querySelector(".history-spark").appendChild(histSpark.el);

    view[metric] = {
      gauge,
      cardSpark,
      histSpark,
      cardSub: card.querySelector(".sub"),
      histSub: histRow.querySelector(".history-head .sub"),
    };
  }
}

function fmtBytes(b) {
  return `${(b / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function pushPct(metric, pct, subText) {
  const v = view[metric];
  v.gauge.set(pct);
  v.cardSpark.push(pct);
  v.histSpark.push(pct);
  v.cardSub.textContent = subText;
  v.histSub.textContent = subText;
}

function setupTabs() {
  const tabs = document.querySelectorAll(".tab");
  const liveView = document.querySelector(".view-live");
  const historyView = document.querySelector(".view-history");
  tabs.forEach((btn) => {
    btn.addEventListener("click", () => {
      tabs.forEach((b) => b.classList.toggle("active", b === btn));
      const isLive = btn.dataset.tab === "live";
      liveView.hidden = !isLive;
      historyView.hidden = isLive;
      // Force redraw of currently visible sparklines (canvas size may have changed)
      for (const m of METRICS) {
        if (isLive) view[m].cardSpark.redraw();
        else view[m].histSpark.redraw();
      }
    });
  });
}

async function wireEvents() {
  if (!window.__TAURI__) {
    console.warn("Not running inside Tauri — events disabled (browser preview).");
    return;
  }
  const { listen } = window.__TAURI__.event;

  await listen("metrics:cpu", (e) => {
    const { usage_pct } = e.payload;
    pushPct("cpu", usage_pct, `${usage_pct.toFixed(1)}%`);
  });
  await listen("metrics:ram", (e) => {
    const { used_bytes, total_bytes, used_pct } = e.payload;
    pushPct("ram", used_pct, `${fmtBytes(used_bytes)} / ${fmtBytes(total_bytes)}`);
  });
  await listen("metrics:gpu", (e) => {
    const { usage_pct } = e.payload;
    pushPct("gpu", usage_pct, `${usage_pct.toFixed(1)}%`);
  });
  await listen("metrics:vram", (e) => {
    const { used_bytes, total_bytes, used_pct } = e.payload;
    pushPct("vram", used_pct, `${fmtBytes(used_bytes)} / ${fmtBytes(total_bytes)}`);
  });
}

function setupClickToDrill() {
  for (const card of document.querySelectorAll(".view-live .card")) {
    card.addEventListener("click", () => {
      showProcessModal(card.dataset.metric);
    });
    card.style.cursor = "pointer";
  }
  for (const row of document.querySelectorAll(".view-history .history-row")) {
    row.addEventListener("click", () => {
      showProcessModal(row.dataset.metric);
    });
    row.style.cursor = "pointer";
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  mount();
  setupTabs();
  setupClickToDrill();
  await wireEvents();
});
