// Lightweight modal that lists top processes for a given metric kind.

const TITLES = {
  cpu: "Top CPU processes",
  gpu: "Top GPU processes",
  ram: "Top RAM processes",
  vram: "Top VRAM processes",
};

const ACCENTS = {
  cpu: "#4ea3ff",
  gpu: "#b48cff",
  ram: "#6ce3a3",
  vram: "#ffb572",
};

let overlay = null;

function ensureOverlay() {
  if (overlay) return overlay;
  overlay = document.createElement("div");
  overlay.className = "modal-overlay";
  overlay.hidden = true;
  overlay.innerHTML = `
    <div class="modal" role="dialog" aria-modal="true">
      <header class="modal-head">
        <h2 class="modal-title">—</h2>
        <button class="modal-close" aria-label="Close">×</button>
      </header>
      <div class="modal-note" hidden></div>
      <div class="modal-body"></div>
    </div>
  `;
  document.body.appendChild(overlay);

  overlay.addEventListener("click", (e) => {
    if (e.target === overlay) close();
  });
  overlay.querySelector(".modal-close").addEventListener("click", close);
  document.addEventListener("keydown", (e) => {
    if (!overlay.hidden && e.key === "Escape") close();
  });
  return overlay;
}

function close() {
  if (overlay) overlay.hidden = true;
}

function renderRows(processes, accent) {
  if (!processes.length) return "";
  const max = Math.max(...processes.map((p) => p.value), 1);
  return processes
    .map((p) => {
      const w = Math.max(2, Math.min(100, (p.value / max) * 100));
      const safeName = String(p.name).replace(/[<>&"]/g, (c) =>
        ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;" }[c])
      );
      return `
        <li class="proc-row">
          <span class="proc-bar" style="--w:${w}%; --c:${accent}"></span>
          <span class="proc-name" title="PID ${p.pid}">${safeName}</span>
          <span class="proc-pid">${p.pid}</span>
          <span class="proc-val">${p.display}</span>
        </li>
      `;
    })
    .join("");
}

export async function showProcessModal(kind) {
  ensureOverlay();
  overlay.hidden = false;
  overlay.querySelector(".modal-title").textContent = TITLES[kind] || kind;
  const noteEl = overlay.querySelector(".modal-note");
  const body = overlay.querySelector(".modal-body");
  noteEl.hidden = true;
  noteEl.textContent = "";
  body.innerHTML = `<div class="modal-loading">Loading…</div>`;

  if (!window.__TAURI__) {
    body.innerHTML = "";
    noteEl.hidden = false;
    noteEl.textContent = "Browser preview — process data is only available inside the Tauri app.";
    return;
  }

  try {
    const { invoke } = window.__TAURI__.core;
    const result = await invoke("get_top_processes", { kind, limit: 10 });
    if (result.note) {
      noteEl.hidden = false;
      noteEl.textContent = result.note;
    }
    body.innerHTML = `<ul class="proc-list">${renderRows(result.processes, ACCENTS[kind])}</ul>`;
  } catch (err) {
    body.innerHTML = `<div class="modal-error">Failed: ${err}</div>`;
  }
}
