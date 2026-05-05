// Minimal SVG arc gauge. 0..max -> arc fill, color shifts on thresholds.
// Pure DOM, no framework.

const NS = "http://www.w3.org/2000/svg";

const ARC_START_DEG = 135;
const ARC_END_DEG = 405; // 270deg sweep
const RADIUS = 52;
const STROKE = 10;
const SIZE = 140;

function polar(cx, cy, r, deg) {
  const rad = (deg * Math.PI) / 180;
  return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
}

function arcPath(cx, cy, r, startDeg, endDeg) {
  const [x1, y1] = polar(cx, cy, r, startDeg);
  const [x2, y2] = polar(cx, cy, r, endDeg);
  const large = endDeg - startDeg > 180 ? 1 : 0;
  return `M ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2}`;
}

function thresholdColor(pct, accent) {
  if (pct >= 85) return "#ff5d6c";
  if (pct >= 60) return "#ffc857";
  return accent;
}

export function createGauge({ accent = "#4ea3ff" } = {}) {
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${SIZE} ${SIZE}`);
  svg.setAttribute("class", "gauge");
  svg.setAttribute("width", SIZE);
  svg.setAttribute("height", SIZE);

  const cx = SIZE / 2;
  const cy = SIZE / 2;
  const fullSweep = ARC_END_DEG - ARC_START_DEG;

  const track = document.createElementNS(NS, "path");
  track.setAttribute("d", arcPath(cx, cy, RADIUS, ARC_START_DEG, ARC_END_DEG));
  track.setAttribute("fill", "none");
  track.setAttribute("stroke", "#33333f");
  track.setAttribute("stroke-width", STROKE);
  track.setAttribute("stroke-linecap", "round");
  svg.appendChild(track);

  const fill = document.createElementNS(NS, "path");
  fill.setAttribute("fill", "none");
  fill.setAttribute("stroke", accent);
  fill.setAttribute("stroke-width", STROKE);
  fill.setAttribute("stroke-linecap", "round");
  fill.style.transition = "stroke 200ms ease, d 200ms ease";
  svg.appendChild(fill);

  const text = document.createElementNS(NS, "text");
  text.setAttribute("x", cx);
  text.setAttribute("y", cy + 6);
  text.setAttribute("text-anchor", "middle");
  text.setAttribute("class", "gauge-text");
  text.textContent = "—";
  svg.appendChild(text);

  function set(pct, label) {
    const clamped = Math.max(0, Math.min(100, pct));
    const endDeg = ARC_START_DEG + (clamped / 100) * fullSweep;
    if (clamped > 0) {
      fill.setAttribute("d", arcPath(cx, cy, RADIUS, ARC_START_DEG, endDeg));
    } else {
      fill.removeAttribute("d");
    }
    fill.setAttribute("stroke", thresholdColor(clamped, accent));
    text.textContent = label ?? `${clamped.toFixed(0)}%`;
  }

  return { el: svg, set };
}
