"use strict";

const $ = (id) => document.getElementById(id);
const SPARK_LEN = 180;

let intervalMs = 2000;
let timer = null;
let lastData = null;
let sortKey = "cpu";
let sortDir = -1;

// ring buffers for charts
const hist = { cpu: [], mem: [], rx: [], tx: [] };

function push(buf, v) {
  buf.push(v);
  if (buf.length > SPARK_LEN) buf.shift();
}

/* ---------- formatting ---------- */

function fmtBytes(b, perSec) {
  if (b == null) return "-";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let i = 0;
  let v = b;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return (v >= 100 ? v.toFixed(0) : v >= 10 ? v.toFixed(1) : v.toFixed(v < 1 && v > 0 ? 2 : 1))
    + " " + units[i] + (perSec ? "/s" : "");
}

function fmtUptime(s) {
  const d = Math.floor(s / 86400), h = Math.floor(s % 86400 / 3600), m = Math.floor(s % 3600 / 60);
  return (d ? d + "d " : "") + h + "h " + m + "m";
}

function pctClass(p) {
  return p >= 90 ? "crit" : p >= 70 ? "warn" : "";
}

// chart time window label, e.g. "90s" or "6m"
function fmtWindow() {
  const s = Math.round(SPARK_LEN * intervalMs / 1000);
  return s < 120 ? s + "s" : Math.round(s / 60) + "m";
}

function setBar(el, pct) {
  el.style.width = Math.min(100, pct) + "%";
  el.className = "bar-fill " + pctClass(pct);
}

function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

/* ---------- charts: gridlines + gradient area + line ---------- */

function drawChart(canvas, series, max) {
  const ctx = canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth, h = canvas.clientHeight;
  if (canvas.width !== w * dpr) { canvas.width = w * dpr; canvas.height = h * dpr; }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);

  // gridlines at 25/50/75%
  ctx.strokeStyle = "#181e27";
  ctx.lineWidth = 1;
  for (const f of [0.25, 0.5, 0.75]) {
    const y = Math.round(h * f) + 0.5;
    ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
  }

  const peak = max || Math.max(1, ...series.map((s) => Math.max(...s.data)));
  const P = 3;
  const xy = (buf, i) => [
    (i / (SPARK_LEN - 1)) * w,
    h - P - (Math.min(buf[i], peak) / peak) * (h - 2 * P)
  ];

  for (const s of series) {
    if (s.data.length < 2) continue;
    // area fill
    if (s.fill) {
      const grad = ctx.createLinearGradient(0, 0, 0, h);
      grad.addColorStop(0, s.color + (s.alpha || "4d"));
      grad.addColorStop(1, s.color + "05");
      ctx.beginPath();
      ctx.moveTo(0, h);
      for (let i = 0; i < s.data.length; i++) { const [x, y] = xy(s.data, i); ctx.lineTo(x, y); }
      ctx.lineTo(xy(s.data, s.data.length - 1)[0], h);
      ctx.closePath();
      ctx.fillStyle = grad;
      ctx.fill();
    }
    // line
    ctx.beginPath();
    for (let i = 0; i < s.data.length; i++) {
      const [x, y] = xy(s.data, i);
      i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    }
    ctx.strokeStyle = s.color;
    ctx.lineWidth = s.width || 1.5;
    ctx.stroke();
  }
}

/* ---------- rendering ---------- */

function render(d) {
  // header
  $("host-info").textContent =
    `${d.host.hostname} · ${d.host.os} · ${d.host.kernel} · ${d.host.arch} · ${d.host.cpu_count} cores · up ${fmtUptime(d.host.uptime_secs)}`;

  // alert chip
  $("alerts").hidden = !d.alerts.length;
  if (d.alerts.length) {
    $("alerts").textContent =
      "▲ " + d.alerts.map((a) => `${a.metric} ${a.value}% > ${a.threshold}%`).join(" · ");
  }

  // cpu
  const cpu = d.cpu.total_pct;
  $("cpu-total").textContent = cpu.toFixed(1) + "%";
  $("cpu-total").className = "head-val " + pctClass(cpu);
  $("load").textContent = "load " + d.cpu.load_avg.map((l) => l.toFixed(2)).join(" ");
  push(hist.cpu, cpu);
  $("cpu-max").textContent = "100% · " + fmtWindow();
  drawChart($("cpu-chart"), [{ data: hist.cpu, color: "#58a6ff", fill: true }], 100);
  $("cores").innerHTML = d.cpu.per_core_pct.map((p, i) =>
    `<div class="core"><span class="core-idx">${i}</span>` +
    `<div class="bar"><div class="bar-fill ${pctClass(p)}" style="width:${Math.min(100, p)}%"></div></div>` +
    `<span class="core-pct">${p.toFixed(0)}%</span></div>`
  ).join("");
  $("cpu-model").textContent = d.host.cpu_model +
    ` · ${d.host.cpu_count} cores` +
    (d.host.cpu_freq_mhz ? ` · ${(d.host.cpu_freq_mhz / 1000).toFixed(1)} GHz` : "") +
    ` · ${d.host.arch}`;

  // memory
  const memPct = d.memory.total ? (d.memory.used / d.memory.total) * 100 : 0;
  $("mem-label").textContent = `${fmtBytes(d.memory.used)} / ${fmtBytes(d.memory.total)} · ${memPct.toFixed(0)}%`;
  push(hist.mem, memPct);
  $("mem-max").textContent = "100% · " + fmtWindow();
  drawChart($("mem-chart"), [{ data: hist.mem, color: "#3fb950", fill: true }], 100);
  if (d.memory.swap_total) {
    setBar($("swap-bar"), (d.memory.swap_used / d.memory.swap_total) * 100);
    $("swap-bar").classList.add("dim");
    $("swap-label").textContent = `${fmtBytes(d.memory.swap_used)} / ${fmtBytes(d.memory.swap_total)}`;
  } else {
    $("swap-label").textContent = "no swap";
  }

  // network
  const rx = d.net.reduce((a, n) => a + n.rx_bps, 0);
  const tx = d.net.reduce((a, n) => a + n.tx_bps, 0);
  $("net-rx").textContent = "↓ " + fmtBytes(rx, 1);
  $("net-tx").textContent = "↑ " + fmtBytes(tx, 1);
  push(hist.rx, rx);
  push(hist.tx, tx);
  const netMax = Math.max(2e5, ...hist.rx, ...hist.tx) * 1.08;
  $("net-max").textContent = fmtBytes(netMax, 1) + " · " + fmtWindow();
  drawChart($("net-chart"), [
    { data: hist.rx, color: "#58a6ff", fill: true, alpha: "38" },
    { data: hist.tx, color: "#d29922", fill: false, width: 1.2 }
  ], netMax);
  const ifaces = d.net.filter((n) => n.rx_total + n.tx_total > 0)
    .sort((a, b) => (b.rx_bps + b.tx_bps) - (a.rx_bps + a.tx_bps)).slice(0, 4);
  $("net-table").innerHTML =
    `<div class="gt-head"><span>iface</span><span class="num">rx/s</span><span class="num">tx/s</span><span class="num">total</span></div>` +
    ifaces.map((n) =>
      `<div class="gt-row"><span>${esc(n.iface)}</span>` +
      `<span class="num">${fmtBytes(n.rx_bps, 1)}</span><span class="num">${fmtBytes(n.tx_bps, 1)}</span>` +
      `<span class="num dim">${fmtBytes(n.rx_total + n.tx_total)}</span></div>`
    ).join("");

  // disks
  $("disks").innerHTML = d.disks.map((dk) => {
    const pct = dk.total ? (dk.used / dk.total) * 100 : 0;
    return `<div class="stack-row"><div class="label">` +
      `<b>${esc(dk.mount)}</b><span class="fs">${esc(dk.fs)}</span><span class="spacer"></span>` +
      `<span class="val">${fmtBytes(dk.used).replace(/ [KMGT]?i?B$/, "")} / ${fmtBytes(dk.total)} · ${pct.toFixed(0)}%</span></div>` +
      `<div class="bar"><div class="bar-fill ${pctClass(pct)}" style="width:${pct}%"></div></div></div>`;
  }).join("");

  // disk io (linux only — placeholder elsewhere)
  $("diskio-table").hidden = !d.disk_io;
  $("diskio-na").hidden = !!d.disk_io;
  if (d.disk_io) {
    $("diskio-table").innerHTML =
      `<div class="gt-head"><span>device</span><span class="num">read/s</span><span class="num">write/s</span></div>` +
      d.disk_io.map((io) =>
        `<div class="gt-row"><span>${esc(io.device)}</span>` +
        `<span class="num rx">${fmtBytes(io.read_bps, 1)}</span><span class="num tx">${fmtBytes(io.write_bps, 1)}</span></div>`
      ).join("");
  }

  // connections (linux only — placeholder elsewhere)
  $("conns-body").hidden = !d.connections;
  $("conns-na").hidden = !!d.connections;
  if (d.connections) {
    $("conn-est").textContent = d.connections.established;
    $("conn-tw").textContent = d.connections.time_wait;
    $("listen-ports").innerHTML = d.connections.listening.length
      ? d.connections.listening.map((p) => `<span class="port">${esc(p)}</span>`).join("")
      : `<span class="subline">none</span>`;
  }

  // sensors — full list, scrolls within a fixed height
  $("panel-sensors").hidden = !d.sensors;
  if (d.sensors) {
    $("temps").innerHTML = d.sensors.temps.map((t) => {
      const crit = t.critical_c || 100;
      const pct = Math.min(100, (t.temp_c / crit) * 100);
      return `<div class="sensor-row"><span class="s-label">${esc(t.label)}</span>` +
        `<div class="bar"><div class="bar-fill ${pctClass(pct)}" style="width:${pct}%"></div></div>` +
        `<span class="s-val">${t.temp_c.toFixed(1)}°${t.critical_c ? " / " + t.critical_c.toFixed(0) + "°C" : ""}</span></div>`;
    }).join("");
    $("fans").textContent = d.sensors.fans.length
      ? d.sensors.fans.map((f) => `${f.label} ${f.rpm} rpm`).join(" · ")
      : "";
  }

  // processes — full list from server, sorted client-side
  $("proc-total").textContent = `${d.processes.total} total · ${d.processes.running} running` +
    (d.processes.zombie ? ` · ${d.processes.zombie} zombie` : " · 0 zombie");
  const procs = [...d.processes.list].sort((a, b) => {
    const va = sortKey === "cpu" ? a.cpu_pct : sortKey === "mem" ? a.mem_bytes : a[sortKey];
    const vb = sortKey === "cpu" ? b.cpu_pct : sortKey === "mem" ? b.mem_bytes : b[sortKey];
    return (sortKey === "name" ? String(va).localeCompare(String(vb)) : va - vb) * sortDir;
  });
  for (const k of ["pid", "name", "cpu", "mem"]) {
    $("arr-" + k).textContent = sortKey === k ? (sortDir > 0 ? " ▴" : " ▾") : "";
  }
  $("proc-rows").innerHTML = procs.map((p) =>
    `<div class="gt-row"><span class="num dim">${p.pid}</span><span title="${esc(p.name)}">${esc(p.name)}</span>` +
    `<span class="${p.state === "running" ? "st-r" : p.state === "zombie" ? "st-z" : "dim"}">${esc(p.state)}</span>` +
    `<div class="cpu-cell"><div class="bar"><div class="bar-fill ${pctClass(p.cpu_pct)}" style="width:${Math.min(100, p.cpu_pct)}%"></div></div>` +
    `<span class="cpu-num">${p.cpu_pct.toFixed(1)}</span></div>` +
    `<span class="num">${fmtBytes(p.mem_bytes)}</span></div>`
  ).join("");

  // docker
  $("panel-docker").hidden = !d.docker;
  if (d.docker) {
    const running = d.docker.containers.filter((c) => c.state === "running").length;
    $("docker-total").textContent = `${d.docker.containers.length} containers · ${running} running`;
    $("docker-rows").innerHTML = d.docker.containers.map((c) =>
      `<div class="gt-row"><span title="${esc(c.name)}">${esc(c.name)}</span>` +
      `<span class="dim" title="${esc(c.image)}">${esc(c.image)}</span>` +
      `<span class="state-${esc(c.state)}">${esc(c.state)}</span>` +
      `<span class="num">${c.state === "running" ? c.cpu_pct.toFixed(1) : "-"}</span>` +
      `<span class="num dim">${c.state === "running" ? fmtBytes(c.mem_bytes) + " / " + fmtBytes(c.mem_limit) : "-"}</span></div>`
    ).join("");
  }
}

/* ---------- refresh loop ---------- */

async function refresh() {
  try {
    const res = await fetch("/api/stats");
    if (!res.ok) throw new Error(res.status);
    const d = await res.json();
    $("conn").className = "conn-dot ok";
    if (d.warming_up) return;
    const ms = Math.max(500, d.interval_secs * 1000);
    if (ms !== intervalMs) {
      intervalMs = ms;
      clearInterval(timer);
      timer = setInterval(refresh, intervalMs);
    }
    lastData = d;
    render(d);
  } catch {
    $("conn").className = "conn-dot lost";
  }
}

// sortable process headers
$("proc-head").addEventListener("click", (e) => {
  const th = e.target.closest("[data-sort]");
  if (!th) return;
  const key = th.dataset.sort;
  sortDir = sortKey === key ? -sortDir : (key === "name" ? 1 : -1);
  sortKey = key;
  if (lastData) render(lastData);
});

// clock
function tickClock() { $("clock").textContent = new Date().toLocaleTimeString(); }
tickClock();
setInterval(tickClock, 1000);

// redraw charts on resize
let resizeT = null;
window.addEventListener("resize", () => {
  clearTimeout(resizeT);
  resizeT = setTimeout(() => { if (lastData) render(lastData); }, 150);
});

// Seed charts from server-side history so they survive page reloads.
async function seedHistory() {
  try {
    const res = await fetch("/api/history");
    const h = await res.json();
    const pts = h.points || [];
    const stride = Math.max(1, Math.ceil(pts.length / SPARK_LEN));
    for (let i = 0; i < pts.length; i += stride) {
      push(hist.cpu, pts[i].cpu);
      push(hist.mem, pts[i].mem);
      push(hist.rx, pts[i].rx);
      push(hist.tx, pts[i].tx);
    }
  } catch { /* no history yet — charts fill in live */ }
}

seedHistory().then(refresh);
timer = setInterval(refresh, intervalMs);
