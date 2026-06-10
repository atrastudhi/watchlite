"use strict";

const $ = (id) => document.getElementById(id);
const SPARK_LEN = 60;

let intervalMs = 2000;
let timer = null;
let procTab = "cpu";

// ring buffers for sparklines
const hist = { cpu: [], mem: [], rx: [], tx: [] };

function push(buf, v) {
  buf.push(v);
  if (buf.length > SPARK_LEN) buf.shift();
}

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

function setGauge(el, pct) {
  el.style.width = Math.min(100, pct) + "%";
  el.className = "gauge-fill " + pctClass(pct);
}

function drawSpark(canvas, series, colors, max) {
  const ctx = canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth, h = canvas.clientHeight;
  if (canvas.width !== w * dpr) { canvas.width = w * dpr; canvas.height = h * dpr; }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  const peak = max || Math.max(1, ...series.flat());
  series.forEach((buf, si) => {
    if (buf.length < 2) return;
    ctx.beginPath();
    buf.forEach((v, i) => {
      const x = (i / (SPARK_LEN - 1)) * w;
      const y = h - (Math.min(v, peak) / peak) * (h - 2) - 1;
      i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    });
    ctx.strokeStyle = colors[si];
    ctx.lineWidth = 1.5;
    ctx.stroke();
  });
}

function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

function render(d) {
  // header
  $("host-info").textContent =
    `${d.host.hostname} · ${d.host.os} · ${d.host.kernel} · ${d.host.arch} · up ${fmtUptime(d.host.uptime_secs)}`;

  // cpu
  const cpu = d.cpu.total_pct;
  $("cpu-total").textContent = cpu.toFixed(1) + "%";
  setGauge($("cpu-bar"), cpu);
  push(hist.cpu, cpu);
  drawSpark($("cpu-spark"), [hist.cpu], ["#58a6ff"], 100);
  $("cores").innerHTML = d.cpu.per_core_pct.map((p, i) =>
    `<div class="core">${i}<div class="gauge"><div class="gauge-fill ${pctClass(p)}" style="width:${Math.min(100, p)}%"></div></div></div>`
  ).join("");
  $("load").textContent = `load ${d.cpu.load_avg.map((l) => l.toFixed(2)).join(" / ")} · ${d.host.cpu_count} cores`;

  // memory
  const memPct = d.memory.total ? (d.memory.used / d.memory.total) * 100 : 0;
  $("mem-label").textContent = `${fmtBytes(d.memory.used)} / ${fmtBytes(d.memory.total)} (${memPct.toFixed(0)}%)`;
  setGauge($("mem-bar"), memPct);
  push(hist.mem, memPct);
  drawSpark($("mem-spark"), [hist.mem], ["#3fb950"], 100);
  $("swap").textContent = d.memory.swap_total
    ? `swap ${fmtBytes(d.memory.swap_used)} / ${fmtBytes(d.memory.swap_total)}`
    : "no swap";

  // network
  const rx = d.net.reduce((a, n) => a + n.rx_bps, 0);
  const tx = d.net.reduce((a, n) => a + n.tx_bps, 0);
  $("net-total").textContent = `↓ ${fmtBytes(rx, 1)}  ↑ ${fmtBytes(tx, 1)}`;
  push(hist.rx, rx);
  push(hist.tx, tx);
  drawSpark($("net-spark"), [hist.rx, hist.tx], ["#58a6ff", "#d29922"]);
  const ifaces = d.net.filter((n) => n.rx_total + n.tx_total > 0)
    .sort((a, b) => (b.rx_bps + b.tx_bps) - (a.rx_bps + a.tx_bps)).slice(0, 8);
  $("net-table").innerHTML =
    `<tr><th>iface</th><th class="num">rx/s</th><th class="num">tx/s</th><th class="num">rx total</th><th class="num">tx total</th></tr>` +
    ifaces.map((n) =>
      `<tr><td>${esc(n.iface)}</td><td class="num">${fmtBytes(n.rx_bps, 1)}</td><td class="num">${fmtBytes(n.tx_bps, 1)}</td>` +
      `<td class="num">${fmtBytes(n.rx_total)}</td><td class="num">${fmtBytes(n.tx_total)}</td></tr>`
    ).join("");

  // disks
  $("disks").innerHTML = d.disks.map((dk) => {
    const pct = dk.total ? (dk.used / dk.total) * 100 : 0;
    return `<div class="disk-row"><div class="label"><span><b>${esc(dk.mount)}</b> ${esc(dk.fs)}</span>` +
      `<span>${fmtBytes(dk.used)} / ${fmtBytes(dk.total)} (${pct.toFixed(0)}%)</span></div>` +
      `<div class="gauge small"><div class="gauge-fill ${pctClass(pct)}" style="width:${pct}%"></div></div></div>`;
  }).join("");

  // disk io (linux only)
  $("panel-diskio").hidden = !d.disk_io;
  if (d.disk_io) {
    $("diskio-table").innerHTML =
      `<tr><th>device</th><th class="num">read/s</th><th class="num">write/s</th></tr>` +
      d.disk_io.map((io) =>
        `<tr><td>${esc(io.device)}</td><td class="num">${fmtBytes(io.read_bps, 1)}</td><td class="num">${fmtBytes(io.write_bps, 1)}</td></tr>`
      ).join("");
  }

  // processes
  $("proc-total").textContent = d.processes.total + " total";
  const list = procTab === "cpu" ? d.processes.top_cpu : d.processes.top_mem;
  $("proc-table").innerHTML =
    `<tr><th class="num">pid</th><th>name</th><th class="num">cpu% (1 core)</th><th class="num">mem</th></tr>` +
    list.map((p) =>
      `<tr><td class="num">${p.pid}</td><td>${esc(p.name)}</td>` +
      `<td class="num">${p.cpu_pct.toFixed(1)}</td><td class="num">${fmtBytes(p.mem_bytes)}</td></tr>`
    ).join("");

  // docker
  $("panel-docker").hidden = !d.docker;
  if (d.docker) {
    $("docker-table").innerHTML =
      `<tr><th>name</th><th>image</th><th>state</th><th class="num">cpu%</th><th class="num">mem</th><th class="num">limit</th></tr>` +
      d.docker.containers.map((c) =>
        `<tr><td>${esc(c.name)}</td><td>${esc(c.image)}</td>` +
        `<td class="state-${esc(c.state)}">${esc(c.state)}</td>` +
        `<td class="num">${c.state === "running" ? c.cpu_pct.toFixed(1) : "-"}</td>` +
        `<td class="num">${c.state === "running" ? fmtBytes(c.mem_bytes) : "-"}</td>` +
        `<td class="num">${c.state === "running" ? fmtBytes(c.mem_limit) : "-"}</td></tr>`
      ).join("");
  }
}

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
    render(d);
  } catch {
    $("conn").className = "conn-dot lost";
  }
}

$("tab-cpu").onclick = () => { procTab = "cpu"; $("tab-cpu").classList.add("active"); $("tab-mem").classList.remove("active"); refresh(); };
$("tab-mem").onclick = () => { procTab = "mem"; $("tab-mem").classList.add("active"); $("tab-cpu").classList.remove("active"); refresh(); };

refresh();
timer = setInterval(refresh, intervalMs);
