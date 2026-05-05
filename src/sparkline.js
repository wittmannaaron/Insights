// Canvas-based sparkline. Ring-buffer of N samples, redraws on push.
// Drawing happens inside requestAnimationFrame to coalesce rapid updates.

export function createSparkline({ capacity = 60, color = "#4ea3ff", height = 28 } = {}) {
  const canvas = document.createElement("canvas");
  canvas.className = "sparkline";
  canvas.style.height = `${height}px`;
  canvas.style.width = "100%";

  const ring = new Float32Array(capacity);
  let writeIdx = 0;
  let count = 0;
  let dirty = false;
  let rafId = null;

  function push(value) {
    ring[writeIdx] = value;
    writeIdx = (writeIdx + 1) % capacity;
    if (count < capacity) count++;
    if (!dirty) {
      dirty = true;
      rafId = requestAnimationFrame(redraw);
    }
  }

  function redraw() {
    rafId = null;
    dirty = false;
    const dpr = window.devicePixelRatio || 1;
    const cssW = canvas.clientWidth || 200;
    const cssH = canvas.clientHeight || height;
    const w = Math.floor(cssW * dpr);
    const h = Math.floor(cssH * dpr);
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    const ctx = canvas.getContext("2d");
    ctx.clearRect(0, 0, w, h);
    if (count < 2) return;

    const pad = 2 * dpr;
    const innerW = w - pad * 2;
    const innerH = h - pad * 2;

    // Linearize ring
    const start = (writeIdx - count + capacity) % capacity;
    let min = Infinity;
    let max = -Infinity;
    for (let i = 0; i < count; i++) {
      const v = ring[(start + i) % capacity];
      if (v < min) min = v;
      if (v > max) max = v;
    }
    if (max - min < 1) {
      max = min + 1;
    }

    // Fill area
    ctx.beginPath();
    for (let i = 0; i < count; i++) {
      const v = ring[(start + i) % capacity];
      const x = pad + (i / (capacity - 1)) * innerW;
      const y = pad + innerH - ((v - min) / (max - min)) * innerH;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    const last = ring[(start + count - 1) % capacity];
    const lastX = pad + ((count - 1) / (capacity - 1)) * innerW;
    ctx.lineTo(lastX, pad + innerH);
    ctx.lineTo(pad, pad + innerH);
    ctx.closePath();
    ctx.fillStyle = color + "22";
    ctx.fill();

    // Stroke line
    ctx.beginPath();
    for (let i = 0; i < count; i++) {
      const v = ring[(start + i) % capacity];
      const x = pad + (i / (capacity - 1)) * innerW;
      const y = pad + innerH - ((v - min) / (max - min)) * innerH;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.lineWidth = 1.5 * dpr;
    ctx.strokeStyle = color;
    ctx.lineJoin = "round";
    ctx.stroke();
  }

  function clear() {
    writeIdx = 0;
    count = 0;
    if (rafId) cancelAnimationFrame(rafId);
    const ctx = canvas.getContext("2d");
    ctx.clearRect(0, 0, canvas.width, canvas.height);
  }

  return {
    el: canvas,
    push,
    redraw,
    clear,
    get length() {
      return count;
    },
  };
}

// Shared ring buffer (across views). Used by both 60s card sparklines
// and 5min history view (different capacities, different instances).
export class RingHistory {
  constructor(capacity) {
    this.capacity = capacity;
    this.buf = new Float32Array(capacity);
    this.idx = 0;
    this.count = 0;
  }
  push(v) {
    this.buf[this.idx] = v;
    this.idx = (this.idx + 1) % this.capacity;
    if (this.count < this.capacity) this.count++;
  }
  toArray() {
    const out = new Array(this.count);
    const start = (this.idx - this.count + this.capacity) % this.capacity;
    for (let i = 0; i < this.count; i++) out[i] = this.buf[(start + i) % this.capacity];
    return out;
  }
}
