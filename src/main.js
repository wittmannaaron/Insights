import { createGauge } from "./gauge.js";

const ACCENTS = {
  cpu: "#4ea3ff",
  gpu: "#b48cff",
  ram: "#6ce3a3",
  vram: "#ffb572",
};

const gauges = {};

function mountGauges() {
  for (const card of document.querySelectorAll(".card")) {
    const metric = card.dataset.metric;
    const slot = card.querySelector(".gauge-slot");
    const gauge = createGauge({ accent: ACCENTS[metric] });
    slot.appendChild(gauge.el);
    gauges[metric] = { gauge, sub: card.querySelector(".sub") };
  }
}

function fmtBytes(b) {
  const gb = b / 1024 / 1024 / 1024;
  return `${gb.toFixed(1)} GB`;
}

async function wireEvents() {
  if (!window.__TAURI__) {
    console.warn("Not running inside Tauri — events disabled (browser preview).");
    return;
  }
  const { listen } = window.__TAURI__.event;

  await listen("metrics:cpu", (e) => {
    const { usage_pct } = e.payload;
    gauges.cpu.gauge.set(usage_pct);
    gauges.cpu.sub.textContent = `${usage_pct.toFixed(1)}%`;
  });

  await listen("metrics:ram", (e) => {
    const { used_bytes, total_bytes, used_pct } = e.payload;
    gauges.ram.gauge.set(used_pct);
    gauges.ram.sub.textContent = `${fmtBytes(used_bytes)} / ${fmtBytes(total_bytes)}`;
  });
}

window.addEventListener("DOMContentLoaded", async () => {
  mountGauges();
  await wireEvents();
});
