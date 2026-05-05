const ACCENTS = {
  cpu: "#4ea3ff",
  gpu: "#b48cff",
  ram: "#6ce3a3",
  vram: "#ffb572",
};

function fmtPct(p) {
  return `${p.toFixed(0)}%`;
}

function setMini(metric, pct) {
  const card = document.querySelector(`.mini[data-metric="${metric}"]`);
  if (!card) return;
  const val = card.querySelector(".mini-val");
  val.textContent = fmtPct(pct);
  card.style.setProperty("--accent", ACCENTS[metric]);
  card.style.setProperty("--fill", `${Math.max(0, Math.min(100, pct))}%`);
  card.classList.toggle("warn", pct >= 60 && pct < 85);
  card.classList.toggle("hot", pct >= 85);
}

window.addEventListener("DOMContentLoaded", async () => {
  document.querySelector(".popover-open").addEventListener("click", async () => {
    if (!window.__TAURI__) return;
    const { getAllWebviewWindows } = window.__TAURI__.webviewWindow;
    const wins = await getAllWebviewWindows();
    const main = wins.find((w) => w.label === "main");
    if (main) {
      await main.show();
      await main.setFocus();
    }
    const popover = wins.find((w) => w.label === "popover");
    if (popover) await popover.hide();
  });

  if (!window.__TAURI__) return;
  const { listen } = window.__TAURI__.event;

  await listen("metrics:cpu", (e) => setMini("cpu", e.payload.usage_pct));
  await listen("metrics:gpu", (e) => setMini("gpu", e.payload.usage_pct));
  await listen("metrics:ram", (e) => setMini("ram", e.payload.used_pct));
  await listen("metrics:vram", (e) => setMini("vram", e.payload.used_pct));
  await listen("metrics:ollama_tps", (e) => {
    const el = document.querySelector(".popover-tps-val");
    const m = e.payload;
    if (m.eval_tps != null) {
      el.textContent = `${m.eval_tps.toFixed(0)} tps · ${m.model}`;
    }
  });
});
