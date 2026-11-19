/* Copyright (C) 2026 NetSalmon. SPDX-License-Identifier: AGPL-3.0-only */

const UNIVERSES = [
  ["all", "全部互动"],
  ["mutual", "双向圈"],
  ["inflow", "流入圈"],
  ["outflow", "流出圈"],
  ["reply", "回复圈"],
  ["quote", "引用圈"],
  ["mention", "提及圈"],
  ["repost", "转发圈"],
  ["top50", "核心50"],
  ["top100", "核心100"],
  ["d7", "近7日"],
  ["d30", "近30日"],
];

const COLORS = [
  ["d1", "1日变化", false],
  ["d3", "3日变化", false],
  ["d5", "5日变化", false],
  ["d10", "10日变化", false],
  ["d20", "20日变化", false],
  ["d60", "60日变化", false],
  ["ytd", "年初至今", false],
  ["flow", "净流入占比", true],
];

const AREAS = [
  ["score", "互动分"],
  ["count", "互动次数"],
  ["heat", "互动热度"],
  ["ratio", "热度比"],
];

const LEVELS = [
  [1, "一级关系"],
  [2, "二级关系"],
  [3, "三级关系"],
];

const DENSITIES = [
  ["1", "疏"],
  ["2", "标准"],
  ["3", "密"],
  ["4", "很密"],
];

let suppressClick = false;

const state = {
  circle: null,
  universe: "all",
  colorKey: "flow",
  baseStatus: "等待生成",
  areaKey: "score",
  scaleDensity: 2,
  level: 1,
  focus: null,
  watchOn: false,
  watch: new Set(),
  tree: null,
  layoutKey: "",
  view: { scale: 1, x: 0, y: 0 },
  hover: null,
  drag: null,
  stats: null,
};

const $ = (id) => document.getElementById(id);
const canvas = $("treemap");
const ctx = canvas.getContext("2d");

function boot() {
  fillSelect($("colorKey"), colorOptions(), "flow");
  fillSelect($("areaKey"), AREAS, "score");
  fillSelect($("level"), LEVELS, "1");
  const savedDensity = Number(localStorage.getItem("xstar-scale-density"));
  state.scaleDensity = DENSITIES.some(([value]) => Number(value) === savedDensity) ? savedDensity : 2;
  fillSelect($("scaleDensity"), DENSITIES, String(state.scaleDensity));
  const ul = $("universes");
  for (const [value, label] of UNIVERSES) {
    const li = document.createElement("li");
    li.dataset.value = value;
    li.textContent = label;
    if (value === "all") li.classList.add("on");
    li.addEventListener("click", () => {
      state.universe = value;
      state.focus = null;
      resetView();
      markUniverse();
      render();
    });
    ul.appendChild(li);
  }
  paintLegend();
  const saved = localStorage.getItem("xstar-watch") || "";
  $("watchList").value = saved;
  state.watch = parseWatch(saved);
  $("watchOn").checked = localStorage.getItem("xstar-watch-on") === "1";
  state.watchOn = $("watchOn").checked;

  $("searchForm").addEventListener("submit", (ev) => {
    ev.preventDefault();
    generate($("username").value);
  });
  $("demoBtn").addEventListener("click", () => loadDemo());
  $("emptyDemo").addEventListener("click", () => loadDemo());
  $("colorKey").addEventListener("change", () => {
    state.colorKey = $("colorKey").value;
    paintLegend();
    render();
  });
  $("scaleDensity").addEventListener("change", () => {
    state.scaleDensity = Number($("scaleDensity").value);
    localStorage.setItem("xstar-scale-density", String(state.scaleDensity));
    paintLegend();
    render();
  });
  $("areaKey").addEventListener("change", () => {
    state.areaKey = $("areaKey").value;
    resetView();
    render();
  });
  $("level").addEventListener("change", () => {
    state.level = Number($("level").value);
    state.focus = null;
    resetView();
    render();
  });
  $("backBtn").addEventListener("click", (ev) => {
    ev.preventDefault();
    state.focus = null;
    resetView();
    render();
  });
  $("watchOn").addEventListener("change", () => {
    state.watchOn = $("watchOn").checked;
    localStorage.setItem("xstar-watch-on", state.watchOn ? "1" : "0");
    draw();
  });
  $("watchList").addEventListener("input", () => {
    state.watch = parseWatch($("watchList").value);
    localStorage.setItem("xstar-watch", $("watchList").value);
    draw();
  });
  $("lookupBtn").addEventListener("click", () => openId($("lookupId").value));
  $("lookupId").addEventListener("keydown", (ev) => {
    if (ev.key === "Enter") openId($("lookupId").value);
  });
  $("feedbackBtn").addEventListener("click", (ev) => {
    ev.preventDefault();
    openModal("feedbackModal");
  });
  $("boardBtn").addEventListener("click", async (ev) => {
    ev.preventDefault();
    await showBoard();
  });
  document.querySelectorAll("[data-close]").forEach((node) => {
    node.addEventListener("click", () => closeModal(node.dataset.close));
  });
  document.querySelectorAll(".modal").forEach((modal) => {
    modal.addEventListener("click", (ev) => {
      if (ev.target === modal) modal.hidden = true;
    });
  });
  window.addEventListener("keydown", (ev) => {
    if (ev.key !== "Escape") return;
    for (const modal of document.querySelectorAll(".modal")) modal.hidden = true;
  });
  $("fbSubmit").addEventListener("click", sendFeedback);
  $("fullBtn").addEventListener("click", (ev) => {
    ev.preventDefault();
    if (document.fullscreenElement) document.exitFullscreen();
    else document.documentElement.requestFullscreen().catch(() => {});
  });
  document.addEventListener("fullscreenchange", () => {
    $("fullBtn").textContent = document.fullscreenElement ? "退出全屏" : "全屏";
    $("fullBtn").classList.toggle("exitfullscreen", Boolean(document.fullscreenElement));
  });
  $("optBtn").addEventListener("click", () => {
    document.body.classList.toggle("drawer-on");
    requestAnimationFrame(resize);
  });
  $("copyId").addEventListener("click", copyId);
  $("openHome").addEventListener("click", () => {
    const name = state.circle && state.circle.username;
    if (!name || name === "demo") return;
    window.open(`https://x.com/${name}`, "_blank", "noopener");
  });

  canvas.addEventListener("mousemove", onMove);
  canvas.addEventListener("mouseleave", () => {
    state.hover = null;
    hideFloaters();
  });
  canvas.addEventListener("mousedown", (ev) => {
    state.drag = { x: ev.clientX, y: ev.clientY, ox: state.view.x, oy: state.view.y, moved: false };
  });
  window.addEventListener("mouseup", () => {
    if (state.drag && state.drag.moved) suppressClick = true;
    state.drag = null;
  });
  canvas.addEventListener("dblclick", onDouble);
  canvas.addEventListener("click", onClick);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  window.addEventListener("resize", resize);
  window.addEventListener("popstate", () => {
    const id = idFromPath();
    if (id) loadId(id, false);
  });

  resize();
  refreshStats();
  const initial = idFromPath();
  if (initial) loadId(initial, false);
  else render();
}

function colorOptions() {
  const opts = [["", "——互动变化——", true]];
  for (const [value, label, flow] of COLORS) {
    if (flow) opts.push(["", "——资金流向——", true]);
    opts.push([value, label, false]);
  }
  return opts;
}

function fillSelect(select, options, current) {
  select.replaceChildren();
  for (const [value, label, disabled] of options) {
    const opt = document.createElement("option");
    opt.value = value;
    opt.textContent = label;
    opt.disabled = Boolean(disabled);
    if (value === String(current)) opt.selected = true;
    select.appendChild(opt);
  }
}

function snapTick(value) {
  if (!(value > 0)) return 0;
  const exp = Math.floor(Math.log10(value));
  const base = 10 ** exp;
  const fraction = value / base;
  const nice = fraction < 1.5 ? 1 : fraction < 3.5 ? 2 : fraction < 7.5 ? 5 : 10;
  const snapped = nice * base;
  const clean = value >= 10 ? Math.round(value) : Math.round(value * 2) / 2;
  if (clean > 0 && clean < 100 && Math.abs(clean - value) / value < 0.08) return clean;
  return snapped;
}

function colorSpan() {
  const base = state.colorKey === "flow" ? 20 : 4;
  const tighten = [2, 1, 0.5, 0.25][state.scaleDensity - 1] ?? 1;
  return base * tighten;
}

function legendTicks() {
  const span = colorSpan();
  const bands = [
    [1, 5],
    [0.25, 0.5, 1, 2, 5],
    [0.1, 0.25, 0.5, 1, 2, 4],
    [0.05, 0.1, 0.25, 0.5, 1, 2, 3],
  ][state.scaleDensity - 1] ?? [0.25, 0.5, 1, 2, 5];
  const values = new Set([0, -100, 100]);
  for (const mult of bands) {
    const tick = snapTick(span * mult);
    if (tick <= 0 || tick >= 100) continue;
    values.add(tick);
    values.add(-tick);
  }
  return [...values].sort((a, b) => a - b);
}

function paintLegend() {
  const ul = $("legend");
  ul.replaceChildren();
  for (const value of legendTicks()) {
    const li = document.createElement("li");
    li.textContent = value > 0 ? `+${value}` : String(value);
    li.style.background = colorFor(value);
    ul.appendChild(li);
  }
}

function parseWatch(raw) {
  return new Set(
    raw
      .split(/[\s,，]+/)
      .map((s) => s.trim().replace(/^@/, "").toLowerCase())
      .filter(Boolean),
  );
}

function idFromPath() {
  const match = location.pathname.match(/^\/c\/([A-Za-z0-9]{4,8})$/);
  return match ? match[1].toLowerCase() : "";
}

async function generate(raw) {
  const username = raw.trim().replace(/^@/, "");
  if (!/^[A-Za-z0-9_]{1,15}$/.test(username)) {
    setStatus("用户名是 1–15 位字母、数字或下划线");
    return;
  }
  await runLoad("/api/circles", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ username, persist: $("persist").checked }),
  });
}

async function loadDemo() {
  await runLoad("/api/demo");
}

async function openId(raw) {
  const id = raw.trim().toLowerCase();
  if (!/^[a-z0-9]{8}$/.test(id)) {
    setStatus("圈子 ID 是 8 位");
    return;
  }
  await loadId(id, true);
}

async function loadId(id, push) {
  await runLoad(`/api/circles/${id}`, null, push);
}

async function runLoad(url, options, push = true) {
  const stop = beginLoad();
  try {
    const res = await fetch(url, options);
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body.error || `请求失败 ${res.status}`);
    applyCircle(body, push);
    refreshStats();
  } catch (err) {
    setStatus(err.message || "生成失败");
  } finally {
    stop();
    if (!state.circle) $("empty").hidden = false;
  }
}

function beginLoad() {
  $("loading").hidden = false;
  $("empty").hidden = true;
  $("go").disabled = true;
  const items = [...$("stages").querySelectorAll("li")];
  $("stages").hidden = false;
  items.forEach((li) => li.classList.remove("on"));
  let step = 0;
  items[0].classList.add("on");
  const timer = setInterval(() => {
    step = Math.min(step + 1, items.length - 1);
    items[step].classList.add("on");
  }, 650);
  return () => {
    clearInterval(timer);
    $("stages").hidden = true;
    $("loading").hidden = true;
    $("go").disabled = false;
  };
}

function applyCircle(circle, push) {
  state.circle = circle;
  state.focus = null;
  state.universe = "all";
  markUniverse();
  resetView();
  paintOwner(circle);
  preloadAvatars(circle.users);
  $("username").value = circle.username === "demo" ? "" : circle.username;
  $("lookupId").value = circle.id === "demo" ? "" : circle.id;
  const when = new Date(circle.createdAt * 1000);
  const stamp = `${when.getMonth() + 1}-${when.getDate()} ${pad(when.getHours())}:${pad(when.getMinutes())}`;
  const tag = circle.source === "demo" ? "示例盘面" : `@${circle.username}`;
  state.baseStatus = `${stamp}\n${tag} · ${circle.users.length}人`;
  setStatus(state.baseStatus);
  $("copyId").classList.toggle("disabled", circle.id === "demo");
  $("openHome").classList.toggle("disabled", !circle.username || circle.username === "demo");
  if (push) {
    const path = `/c/${circle.id}`;
    if (location.pathname !== path) history.pushState({ id: circle.id }, "", path);
  }
  $("empty").hidden = true;
  document.body.classList.remove("drawer-on");
  render();
}

function setStatus(text) {
  $("quotetime").textContent = text;
}

function pad(n) {
  return String(n).padStart(2, "0");
}

function markUniverse() {
  for (const li of $("universes").children) {
    li.classList.toggle("on", li.dataset.value === state.universe);
  }
}

function resetView() {
  state.view = { scale: 1, x: 0, y: 0 };
  state.layoutKey = "";
}

function filteredUsers() {
  const circle = state.circle;
  if (!circle) return [];
  let users = circle.users.slice();
  switch (state.universe) {
    case "mutual":
      users = users.filter((u) => u.sector1 === "双向核心");
      break;
    case "inflow":
      users = users.filter((u) => u.sector1 === "流入为主");
      break;
    case "outflow":
      users = users.filter((u) => u.sector1 === "流出为主");
      break;
    case "reply":
      users = users.filter((u) => u.sector2 === "回复");
      break;
    case "quote":
      users = users.filter((u) => u.sector2 === "引用");
      break;
    case "mention":
      users = users.filter((u) => u.sector2 === "提及");
      break;
    case "repost":
      users = users.filter((u) => u.sector2 === "转发");
      break;
    case "top50":
      users = users.slice(0, 50);
      break;
    case "top100":
      users = users.slice(0, 100);
      break;
    case "d7":
      users = users.filter((u) => u.recencyDays <= 7);
      break;
    case "d30":
      users = users.filter((u) => u.recencyDays <= 30);
      break;
    default:
      break;
  }
  return users;
}

function groupName(user) {
  if (state.level === 1) return user.sector1;
  if (state.level === 2) return `${user.sector1}·${user.sector2}`;
  return `${user.sector2}·${user.sector3}`;
}

function areaOf(user) {
  if (state.areaKey === "count") return Math.max(user.count, 0.01);
  if (state.areaKey === "heat") return Math.max(user.heat, 0.01);
  if (state.areaKey === "ratio") return Math.max(user.heatRatio, 0.01);
  return Math.max(user.score, 0.01);
}

function metricOf(user) {
  return Number(user.zdf[state.colorKey] || 0);
}

function buildTree(users) {
  const focused = state.focus
    ? users.filter((u) => groupName(u) === state.focus)
    : users;
  if (state.focus) {
    return {
      depth: 0,
      children: focused.map((u) => leaf(u)),
    };
  }
  const groups = new Map();
  for (const user of focused) {
    const name = groupName(user);
    if (!groups.has(name)) groups.set(name, []);
    groups.get(name).push(user);
  }
  const children = [...groups.entries()]
    .map(([name, list]) => ({
      depth: 1,
      name,
      value: list.reduce((sum, user) => sum + areaOf(user), 0),
      children: list.map((u) => leaf(u)),
    }))
    .sort((a, b) => b.value - a.value);
  return { depth: 0, children };
}

function leaf(user) {
  return {
    depth: 2,
    name: user.name || user.handle,
    value: areaOf(user),
    user,
  };
}

function layoutNode(node, x, y, w, h) {
  node.x0 = x;
  node.y0 = y;
  node.x1 = x + w;
  node.y1 = y + h;
  if (!node.children || !node.children.length || w <= 0 || h <= 0) return;
  const pad = 1;
  const top = node.depth >= 1 ? 16 : 1;
  const ix = x + pad;
  const iy = y + top;
  const iw = Math.max(0, w - pad * 2);
  const ih = Math.max(0, h - top - pad);
  squarify(node.children, ix, iy, iw, ih);
  for (const child of node.children) {
    layoutNode(child, child.x0, child.y0, child.x1 - child.x0, child.y1 - child.y0);
  }
}

function squarify(nodes, x, y, w, h) {
  const total = nodes.reduce((sum, node) => sum + node.value, 0);
  if (!(total > 0) || w <= 1 || h <= 1) {
    for (const node of nodes) {
      node.x0 = x;
      node.y0 = y;
      node.x1 = x;
      node.y1 = y;
    }
    return;
  }
  const items = nodes
    .map((node) => ({ node, area: (node.value / total) * w * h }))
    .sort((a, b) => b.area - a.area);
  placeRows(items, { x, y, w, h });
}

function placeRows(items, rect) {
  if (!items.length || rect.w <= 0 || rect.h <= 0) return;
  let row = [];
  let i = 0;
  while (i < items.length) {
    const next = items[i];
    if (!row.length || worst(row, rect) >= worst(row.concat(next), rect)) {
      row.push(next);
      i += 1;
    } else {
      rect = placeRow(row, rect);
      row = [];
    }
  }
  if (row.length) placeRow(row, rect);
}

function worst(row, rect) {
  const sum = row.reduce((s, item) => s + item.area, 0);
  const side = Math.min(rect.w, rect.h);
  if (side <= 0 || sum <= 0) return Infinity;
  let rmax = 0;
  let rmin = Infinity;
  for (const item of row) {
    rmax = Math.max(rmax, item.area);
    rmin = Math.min(rmin, item.area);
  }
  const s2 = side * side;
  return Math.max((s2 * rmax) / (sum * sum), (sum * sum) / (s2 * rmin));
}

function placeRow(row, rect) {
  const sum = row.reduce((s, item) => s + item.area, 0);
  if (rect.w >= rect.h) {
    const dw = Math.min(rect.w, sum / Math.max(rect.h, 0.01));
    let y = rect.y;
    for (const item of row) {
      const dh = item.area / Math.max(dw, 0.01);
      setBox(item.node, rect.x, y, dw, Math.min(dh, rect.y + rect.h - y));
      y += dh;
    }
    return { x: rect.x + dw, y: rect.y, w: Math.max(0, rect.w - dw), h: rect.h };
  }
  const dh = Math.min(rect.h, sum / Math.max(rect.w, 0.01));
  let x = rect.x;
  for (const item of row) {
    const dw = item.area / Math.max(dh, 0.01);
    setBox(item.node, x, rect.y, Math.min(dw, rect.x + rect.w - x), dh);
    x += dw;
  }
  return { x: rect.x, y: rect.y + dh, w: rect.w, h: Math.max(0, rect.h - dh) };
}

function setBox(node, x, y, w, h) {
  node.x0 = x;
  node.y0 = y;
  node.x1 = x + Math.max(0, w);
  node.y1 = y + Math.max(0, h);
}

function resize() {
  state.layoutKey = "";
  render();
}

function chartSize() {
  const box = $("mainchart").getBoundingClientRect();
  return { w: Math.max(10, box.width), h: Math.max(10, box.height) };
}

function render() {
  const { w, h } = chartSize();
  const users = filteredUsers();
  const key = [
    state.circle && state.circle.id,
    state.universe,
    state.areaKey,
    state.level,
    state.focus,
    users.length,
    Math.round(w),
    Math.round(h),
  ].join("|");
  if (key !== state.layoutKey) {
    state.tree = buildTree(users);
    layoutNode(state.tree, 0, 0, w, h);
    state.layoutKey = key;
  }
  draw();
  updateSide(users);
  $("backBtn").classList.toggle("is-hidden", !state.focus);
  if (state.baseStatus) {
    setStatus(state.focus ? `${state.baseStatus}\n${state.focus}` : state.baseStatus);
  }
  $("empty").hidden = Boolean(state.circle) || !$("loading").hidden;
}

function draw() {
  const { w, h } = chartSize();
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(h * dpr);
  canvas.style.width = `${w}px`;
  canvas.style.height = `${h}px`;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = "#262931";
  ctx.fillRect(0, 0, w, h);
  ctx.save();
  ctx.translate(state.view.x, state.view.y);
  ctx.scale(state.view.scale, state.view.scale);
  const groups = [];
  const leaves = [];
  walk(state.tree, groups, leaves);
  ctx.textBaseline = "alphabetic";
  ctx.textAlign = "left";
  for (const group of groups) {
    const gw = group.x1 - group.x0;
    if (gw < 8 || group.y1 - group.y0 < 8) continue;
    ctx.fillStyle = "#fff";
    ctx.font = `${gw < 48 ? 9 : 13}px sans-serif`;
    ctx.save();
    ctx.beginPath();
    ctx.rect(group.x0, group.y0, gw, 16);
    ctx.clip();
    ctx.fillText(group.name, group.x0 + 2, group.y0 + 12);
    ctx.restore();
  }
  for (const leafNode of leaves) {
    const user = leafNode.user;
    const bw = leafNode.x1 - leafNode.x0;
    const bh = leafNode.y1 - leafNode.y0;
    if (bw < 1 || bh < 1) continue;
    ctx.fillStyle = colorFor(metricOf(user));
    ctx.fillRect(leafNode.x0, leafNode.y0, bw, bh);
    if (state.watchOn && state.watch.has(user.handle)) {
      ctx.strokeStyle = "#FFD614";
      ctx.lineWidth = 3;
      ctx.strokeRect(leafNode.x0 + 1.5, leafNode.y0 + 1.5, bw - 3, bh - 3);
    }
    const size = labelSize(bw, bh, (user.name || user.handle).length);
    const textH = size > 0 ? size * 2.05 : 0;
    let avatarR = avatarRadius(bw, bh);
    if (avatarR > 0 && avatarR * 2 + 4 + textH > bh - 4) {
      avatarR = Math.floor((bh - 4 - textH) / 2);
      if (avatarR < 8) avatarR = 0;
    }
    if (size > 0 || avatarR > 0) {
      ctx.save();
      ctx.beginPath();
      ctx.rect(leafNode.x0 + 1, leafNode.y0 + 1, Math.max(0, bw - 2), Math.max(0, bh - 2));
      ctx.clip();
      const gap = avatarR > 0 && size > 0 ? 3 : 0;
      const block = avatarR * 2 + gap + textH;
      let top = leafNode.y0 + Math.max(2, (bh - block) / 2);
      const cx = leafNode.x0 + bw / 2;
      if (avatarR > 0) {
        drawAvatar(ctx, user, cx, top + avatarR, avatarR);
        top += avatarR * 2 + gap;
      }
      if (size > 0) {
        ctx.fillStyle = "#fff";
        ctx.textAlign = "center";
        ctx.textBaseline = "top";
        ctx.font = `${size}px sans-serif`;
        ctx.fillText(user.name || user.handle, cx, top);
        ctx.font = `${Math.max(8, size * 0.9)}px sans-serif`;
        ctx.fillText(fmtPct(metricOf(user)), cx, top + size * 1.05);
      }
      ctx.restore();
    }
  }
  ctx.restore();
  if (!leaves.length && state.circle) {
    ctx.fillStyle = "#c5c8d4";
    ctx.font = "16px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText("这个范围里没有互动", w / 2, h / 2);
  }
}

const avatarCache = new Map();
let avatarFrame = 0;

function avatarSrc(user) {
  if (!user || !user.avatar) return "";
  return `/api/avatar?u=${encodeURIComponent(user.avatar)}`;
}

function preloadAvatars(users) {
  for (const user of users) ensureAvatar(user);
}

function ensureAvatar(user) {
  const src = avatarSrc(user);
  if (!src) return null;
  let record = avatarCache.get(src);
  if (!record) {
    const img = new Image();
    record = { status: "loading", img };
    img.onload = () => {
      record.status = "ok";
      scheduleAvatarRedraw();
    };
    img.onerror = () => {
      record.status = "err";
    };
    img.src = src;
    avatarCache.set(src, record);
  }
  return record.status === "ok" ? record.img : null;
}

function scheduleAvatarRedraw() {
  cancelAnimationFrame(avatarFrame);
  avatarFrame = requestAnimationFrame(() => draw());
}

function paintOwner(circle) {
  const img = $("ownerAvatar");
  if (!circle || !circle.avatar || circle.username === "demo") {
    img.hidden = true;
    img.removeAttribute("src");
    return;
  }
  img.alt = circle.displayName || circle.username;
  img.src = avatarSrc(circle);
  img.hidden = false;
  img.onerror = () => {
    img.hidden = true;
  };
}

function initialOf(user) {
  const text = (user.name || user.handle || "?").trim();
  return [...text][0] || "?";
}

function initialColor(user) {
  let hash = 0;
  for (const char of user.handle || user.name || "?") {
    hash = (hash * 33 + char.charCodeAt(0)) >>> 0;
  }
  return `hsl(${hash % 360} 46% 38%)`;
}

function avatarRadius(w, h) {
  const side = Math.min(w, h);
  if (side < 48 || w < 36) return 0;
  if (side >= 110) return Math.min(26, side * 0.16);
  if (side >= 72) return Math.min(16, side * 0.18);
  return 11;
}

function drawAvatar(ctx, user, x, y, radius) {
  const img = ensureAvatar(user);
  ctx.save();
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.clip();
  if (img) {
    const scale = Math.max((radius * 2) / img.width, (radius * 2) / img.height);
    const dw = img.width * scale;
    const dh = img.height * scale;
    ctx.drawImage(img, x - dw / 2, y - dh / 2, dw, dh);
  } else {
    ctx.fillStyle = initialColor(user);
    ctx.fillRect(x - radius, y - radius, radius * 2, radius * 2);
    ctx.fillStyle = "#fff";
    ctx.font = `700 ${Math.max(9, Math.round(radius))}px sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(initialOf(user), x, y + 0.5);
  }
  ctx.restore();
  ctx.beginPath();
  ctx.arc(x, y, radius - 0.5, 0, Math.PI * 2);
  ctx.strokeStyle = "rgba(0,0,0,0.35)";
  ctx.lineWidth = 1;
  ctx.stroke();
}

function avatarNode(user, className) {
  const src = avatarSrc(user);
  if (src) {
    const img = document.createElement("img");
    img.className = className;
    img.alt = "";
    img.src = src;
    img.addEventListener("error", () => {
      img.replaceWith(fallbackAvatar(user, className));
    });
    return img;
  }
  return fallbackAvatar(user, className);
}

function fallbackAvatar(user, className) {
  const node = document.createElement("span");
  node.className = `${className} avatar-fallback`;
  node.textContent = initialOf(user);
  node.style.background = initialColor(user);
  return node;
}

function labelSize(w, h, nameLength) {
  const slack = nameLength > 4 ? 12 : 0;
  const c = w - slack;
  if (c >= 180 && h >= 120) return 28;
  if (c >= 160 && h >= 100) return 26;
  if (c >= 140 && h >= 90) return 24;
  if (c >= 120 && h >= 80) return 20;
  if (c >= 100 && h >= 70) return 18;
  if (c >= 80 && h >= 60) return 16;
  if (c >= 60 && h >= 50) return 14;
  if (c >= 50 && h >= 40) return 12;
  if (c >= 45 && h >= 30) return 11;
  if (c >= 40 && h >= 25) return 10;
  if (c >= 35 && h >= 22) return 9;
  if (c >= 32 && h >= 20) return 8;
  return 0;
}

function walk(node, groups, leaves) {
  if (!node) return;
  if (node.user) leaves.push(node);
  else if (node.depth === 1) groups.push(node);
  if (node.children) {
    for (const child of node.children) walk(child, groups, leaves);
  }
}

function colorFor(value) {
  const n = Number(value) || 0;
  const span = colorSpan();
  const sign = Math.sign(n);
  const mag = Math.min(Math.abs(n), 100);
  let strength;
  if (mag <= span) strength = Math.sqrt(mag / span) * 0.78;
  else strength = 0.78 + 0.22 * Math.sqrt((mag - span) / (100 - span));
  if (strength < 0.02) return "#3c4150";
  const hue = sign > 0 ? 356 + strength * 8 : 150 - strength * 16;
  const sat = 30 + strength * 52;
  const light = 18 + strength * 30;
  return `hsl(${hue.toFixed(1)} ${sat.toFixed(1)}% ${light.toFixed(1)}%)`;
}

function updateSide(users) {
  let up = 0;
  let down = 0;
  let flat = 0;
  let score = 0;
  for (const user of users) {
    const metric = metricOf(user);
    if (metric > 0.05) up += 1;
    else if (metric < -0.05) down += 1;
    else flat += 1;
    score += areaOf(user);
  }
  $("upN").textContent = state.circle ? String(up) : "-";
  $("downN").textContent = state.circle ? String(down) : "-";
  $("flatN").textContent = state.circle ? String(flat) : "-";
  $("scoreN").textContent = state.circle ? fmtNum(state.areaKey === "score" ? score : users.reduce((s, u) => s + u.score, 0)) : "-";
  const market = state.circle && state.circle.market;
  const change = market ? market[state.colorKey] : null;
  const node = $("chgN");
  if (change == null || state.colorKey === "flow") {
    node.textContent = market ? fmtPct(market.flow) : "-";
    node.className = !market ? "" : market.flow >= 0 ? "price_up" : "price_down";
  } else {
    node.textContent = fmtPct(change);
    node.className = change > 0.05 ? "price_up" : change < -0.05 ? "price_down" : "";
  }
  paintHeader(up, down, score);
}

function paintHeader(up, down, score) {
  const box = $("topzs");
  box.replaceChildren();
  const stats = state.stats;
  const items = [
    ["流入", state.circle ? up : "-", up ? "up" : ""],
    ["流出", state.circle ? down : "-", down ? "down" : ""],
    ["互动分", state.circle ? fmtNum(state.areaKey === "score" ? score : state.circle.market.totalScore) : "-", ""],
    ["圈子", stats ? stats.circles : "—", ""],
    ["今日", stats ? stats.today : "—", ""],
  ];
  for (const [label, value, cls] of items) {
    const item = document.createElement("div");
    item.className = "item";
    const cap = document.createElement("div");
    cap.textContent = label;
    const strong = document.createElement("b");
    strong.textContent = value;
    if (cls) strong.className = cls;
    item.append(cap, strong);
    box.appendChild(item);
  }
}

function fmtPct(value) {
  const n = Number(value) || 0;
  const digits = Math.abs(n) >= 100 ? 0 : Math.abs(n) >= 10 ? 1 : 2;
  const text = `${n > 0 ? "+" : ""}${n.toFixed(digits)}%`;
  return text;
}

function fmtNum(value) {
  const n = Number(value) || 0;
  if (n >= 100000000) return `${(n / 100000000).toFixed(2)}亿`;
  if (n >= 10000) return `${(n / 10000).toFixed(2)}万`;
  if (n >= 100) return String(Math.round(n));
  return n.toFixed(n >= 10 ? 1 : 2);
}

function worldPoint(ev) {
  const rect = canvas.getBoundingClientRect();
  const px = ev.clientX - rect.left;
  const py = ev.clientY - rect.top;
  return {
    px,
    py,
    x: (px - state.view.x) / state.view.scale,
    y: (py - state.view.y) / state.view.scale,
  };
}

function hit(ev) {
  if (!state.tree) return null;
  const point = worldPoint(ev);
  const groups = [];
  const leaves = [];
  walk(state.tree, groups, leaves);
  for (let i = leaves.length - 1; i >= 0; i -= 1) {
    const leafNode = leaves[i];
    if (point.x >= leafNode.x0 && point.x <= leafNode.x1 && point.y >= leafNode.y0 && point.y <= leafNode.y1) {
      return { kind: "leaf", node: leafNode, point };
    }
  }
  for (const group of groups) {
    if (point.x >= group.x0 && point.x <= group.x1 && point.y >= group.y0 && point.y <= group.y0 + 16) {
      return { kind: "group", node: group, point };
    }
  }
  return { kind: "empty", point };
}

function onMove(ev) {
  if (state.drag) {
    const dx = ev.clientX - state.drag.x;
    const dy = ev.clientY - state.drag.y;
    if (Math.abs(dx) + Math.abs(dy) > 4) {
      state.drag.moved = true;
      state.view.x = state.drag.ox + dx;
      state.view.y = state.drag.oy + dy;
      draw();
      positionFloaters();
    }
    return;
  }
  const found = hit(ev);
  state.hover = found;
  if (!found || found.kind === "empty") {
    hideFloaters();
    return;
  }
  if (found.kind === "group") {
    $("stockinfo").style.display = "none";
    const band = $("bktitlebgd");
    const { node } = found;
    band.style.display = "block";
    band.style.left = `${node.x0 * state.view.scale + state.view.x}px`;
    band.style.top = `${node.y0 * state.view.scale + state.view.y}px`;
    band.style.width = `${(node.x1 - node.x0) * state.view.scale}px`;
    band.style.height = `${16 * state.view.scale}px`;
    return;
  }
  $("bktitlebgd").style.display = "none";
  showTip(found.node, ev);
}

function onClick(ev) {
  if (suppressClick) {
    suppressClick = false;
    return;
  }
  const found = hit(ev);
  if (found && found.kind === "group") {
    state.focus = found.node.name;
    resetView();
    render();
  }
}

function onDouble(ev) {
  const found = hit(ev);
  if (!found || found.kind !== "leaf") return;
  const handle = found.node.user.handle;
  window.open(`https://x.com/${handle}`, "_blank", "noopener");
}

function onWheel(ev) {
  ev.preventDefault();
  const point = worldPoint(ev);
  const factor = ev.deltaY < 0 ? 1.12 : 1 / 1.12;
  const next = Math.min(8, Math.max(1, state.view.scale * factor));
  state.view.x = point.px - point.x * next;
  state.view.y = point.py - point.y * next;
  state.view.scale = next;
  if (next === 1) state.view = { scale: 1, x: 0, y: 0 };
  draw();
  positionFloaters();
}

function hideFloaters() {
  $("stockinfo").style.display = "none";
  $("bktitlebgd").style.display = "none";
}

function positionFloaters() {
  if (!state.hover || state.hover.kind !== "leaf") return;
  placeTip(state.hover.point.px, state.hover.point.py);
}

function showTip(leafNode, ev) {
  const user = leafNode.user;
  const metric = metricOf(user);
  const info = $("stockinfo");
  info.replaceChildren();
  const title = document.createElement("div");
  title.className = "si_t si_person";
  const who = document.createElement("div");
  who.className = "si_who";
  const link = document.createElement("a");
  link.href = `https://x.com/${user.handle}`;
  link.target = "_blank";
  link.rel = "noopener";
  link.textContent = user.name || user.handle;
  const sub = document.createElement("span");
  sub.className = "si_sub";
  sub.textContent = `@${user.handle}`;
  who.append(link, sub);
  title.append(avatarNode(user, "avatar-lg"), who);
  const banner = document.createElement("div");
  banner.className = `si_binfo ${metric > 0.05 ? "si_zd_up" : metric < -0.05 ? "si_zd_down" : "si_zd_flat"}`;
  const name = document.createElement("span");
  name.textContent = groupName(user);
  const pct = document.createElement("span");
  pct.textContent = fmtPct(metric);
  banner.append(name, pct);
  const spark = document.createElement("canvas");
  spark.className = "spark";
  spark.width = 680;
  spark.height = 92;
  drawSpark(spark, user.series || []);
  const table = document.createElement("table");
  const peers = siblingLeaves(leafNode).slice(0, 8);
  for (const peer of peers) {
    const tr = document.createElement("tr");
    if (peer.user.handle === user.handle) tr.className = "on";
    const tdName = document.createElement("td");
    tdName.appendChild(avatarNode(peer.user, "avatar-sm"));
    const who = document.createElement("span");
    who.className = "who";
    who.textContent = peer.user.name || peer.user.handle;
    who.addEventListener("click", () => {
      window.open(`https://x.com/${peer.user.handle}`, "_blank", "noopener");
    });
    tdName.appendChild(who);
    const tdPct = document.createElement("td");
    tdPct.className = "pct";
    tdPct.textContent = fmtPct(metricOf(peer.user));
    tdPct.style.color = metricOf(peer.user) >= 0 ? "#ff1c1c" : "#00a83a";
    const tdArea = document.createElement("td");
    tdArea.className = "num";
    tdArea.textContent = fmtNum(areaOf(peer.user));
    tr.append(tdName, tdPct, tdArea);
    table.appendChild(tr);
  }
  const foot = document.createElement("div");
  foot.className = "si_t";
  foot.textContent = `流入 ${fmtNum(user.inScore)} · 流出 ${fmtNum(user.outScore)} · ${user.count} 次 · 热度 ${fmtNum(user.heat)}`;
  info.append(title, banner, spark, table, foot);
  info.style.display = "block";
  const point = worldPoint(ev);
  placeTip(point.px, point.py);
}

function siblingLeaves(leafNode) {
  const groups = [];
  const leaves = [];
  walk(state.tree, groups, leaves);
  const parentName = groupName(leafNode.user);
  const same = state.focus ? leaves : leaves.filter((item) => groupName(item.user) === parentName);
  return same.sort((a, b) => b.value - a.value);
}

function placeTip(px, py) {
  const info = $("stockinfo");
  const bounds = $("mainchart").getBoundingClientRect();
  let left = px + 14;
  let top = py + 14;
  const w = info.offsetWidth || 340;
  const h = info.offsetHeight || 220;
  if (left + w > bounds.width - 8) left = px - w - 14;
  if (top + h > bounds.height - 8) top = Math.max(8, bounds.height - h - 8);
  info.style.left = `${Math.max(8, left)}px`;
  info.style.top = `${Math.max(8, top)}px`;
}

function drawSpark(canvasNode, series) {
  const g = canvasNode.getContext("2d");
  const w = canvasNode.width;
  const h = canvasNode.height;
  g.clearRect(0, 0, w, h);
  const days = series.slice(0, 30).reverse();
  const max = Math.max(1, ...days);
  const bw = w / Math.max(days.length, 1);
  days.forEach((value, i) => {
    const bh = (value / max) * (h - 16);
    const prev = i === 0 ? value : days[i - 1];
    g.fillStyle = value >= prev ? "#f63538" : "#30cc5a";
    g.fillRect(i * bw + 1, h - 8 - bh, Math.max(1, bw - 2), bh);
  });
}

async function refreshStats() {
  try {
    const res = await fetch("/api/stats");
    if (!res.ok) return;
    state.stats = await res.json();
    if (state.circle) updateSide(filteredUsers());
    else paintHeader(0, 0, 0);
  } catch {
    /* stats are decorative */
  }
}

async function showBoard() {
  openModal("boardModal");
  const list = $("boardList");
  list.textContent = "加载中…";
  try {
    const res = await fetch("/api/leaderboard");
    const items = await res.json();
    list.replaceChildren();
    if (!items.length) {
      const empty = document.createElement("div");
      empty.className = "boardempty";
      empty.textContent = "还没有长期保存的星图。生成时勾选「长期保存」。";
      list.appendChild(empty);
      return;
    }
    for (const item of items) {
      const button = document.createElement("button");
      button.type = "button";
      const id = document.createElement("span");
      id.textContent = item.id;
      const name = document.createElement("span");
      name.textContent = `${item.displayName} @${item.username}`;
      const count = document.createElement("span");
      count.className = "muted";
      count.textContent = `${item.userCount} 人`;
      const when = document.createElement("span");
      when.className = "muted";
      const date = new Date(item.createdAt * 1000);
      when.textContent = `${date.getMonth() + 1}-${date.getDate()}`;
      button.append(id, name, count, when);
      button.addEventListener("click", () => {
        closeModal("boardModal");
        loadId(item.id, true);
      });
      list.appendChild(button);
    }
  } catch (err) {
    list.textContent = err.message || "排行榜加载失败";
  }
}

function openModal(id) {
  $(id).hidden = false;
}

function closeModal(id) {
  $(id).hidden = true;
}

async function sendFeedback() {
  const text = $("fbText").value.trim();
  const note = $("fbNote");
  if (text.length < 2) {
    note.textContent = "写两个字以上再提交";
    note.style.color = "#c0392b";
    return;
  }
  const res = await fetch("/api/feedback", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ text, contact: $("fbContact").value.trim() }),
  });
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    note.style.color = "#c0392b";
    note.textContent = body.error || "提交失败";
    return;
  }
  note.style.color = "#0a7a32";
  note.textContent = "已保存，谢谢。";
  $("fbText").value = "";
}

async function copyId() {
  const id = state.circle && state.circle.id;
  if (!id || id === "demo") return;
  try {
    await navigator.clipboard.writeText(id);
    setStatus(`圈子 ID ${id} 已复制`);
  } catch {
    setStatus(`圈子 ID ${id}`);
  }
}

boot();
