// Chronologie déterministe : tout l'état visuel est une fonction pure de `frame` (pas de CSS transitions ni de timers).
const FPS = 30;
const TOTAL = 48; // secondes
const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const seg = (t, a, b) => clamp((t - a) / (b - a));
const E = {
  out3: (t) => 1 - Math.pow(1 - t, 3),
  out4: (t) => 1 - Math.pow(1 - t, 4),
  io3: (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2),
  back: (t) => { const c1 = 1.70158, c3 = c1 + 1; return 1 + c3 * Math.pow(t - 1, 3) + c1 * Math.pow(t - 1, 2); },
  expo: (t) => (t >= 1 ? 1 : 1 - Math.pow(2, -10 * t)),
};
const $ = (id) => document.getElementById(id);
const lerp = (a, b, t) => a + (b - a) * t;
function put(el, { x = 0, y = 0, s = 1, r = 0, o = 1 } = {}) {
  el.style.transform = `translate(${x}px,${y}px) rotate(${r}deg) scale(${s})`;
  el.style.opacity = o;
}
// Apparition standard : glisse de `dy` px en fondu.
function rise(el, t, a, d = 0.6, dy = 40) {
  const e = E.out4(seg(t, a, a + d));
  put(el, { y: (1 - e) * dy, o: e });
}

const STARTS = [0, 4.0, 8.0, 12.5, 18.0, 24.0, 30.0, 34.0, 39.5, 43.5];
const scenes = STARTS.map((_, i) => $("s" + (i + 1)));
const WIPE = 0.75;

// Mise en page dépendant des polices : mesurée une fois.
let layout = null;
function splitLetters(el, text) {
  el.textContent = "";
  return [...text].map((ch) => {
    const s = document.createElement("span");
    s.textContent = ch;
    s.style.display = "inline-block";
    s.style.willChange = "transform,opacity";
    el.appendChild(s);
    return s;
  });
}
function measure(el, text) {
  const probe = el.cloneNode(false);
  probe.removeAttribute("id");
  probe.style.cssText += ";position:absolute;visibility:hidden;left:-9999px";
  probe.textContent = text;
  document.body.appendChild(probe);
  const w = probe.getBoundingClientRect().width;
  probe.remove();
  return w;
}
function init() {
  const w1 = measure($("s1word"), "OpenPics");
  const letters1 = splitLetters($("s1word"), "OpenPics");
  const w10 = measure($("s10word"), "OpenPics");
  const letters10 = splitLetters($("s10word"), "OpenPics");
  layout = { w1, letters1, w10, letters10 };
}

function s1(t) {
  const L = layout;
  const total = 240 + 56 + L.w1;
  const x0 = (1920 - total) / 2;
  const shift = E.io3(seg(t, 1.9, 2.7));
  const iconX = lerp(960 - 120, x0, shift);
  const pop = E.back(seg(t, 0, 0.8));
  put($("s1icon"), { x: iconX, y: 540 - 120, s: lerp(0.5, 1, pop), o: seg(t, 0, 0.3) });
  $("i-frame").style.strokeDashoffset = 130 * (1 - E.io3(seg(t, 0.6, 1.4)));
  const sun = E.back(seg(t, 1.2, 1.6));
  $("i-sun").style.transformOrigin = "26px 27px";
  $("i-sun").style.transform = `scale(${sun})`;
  const hill = E.out3(seg(t, 1.3, 1.9));
  $("i-hill").style.opacity = hill;
  $("i-hill").style.transform = `translateY(${(1 - hill) * 8}px)`;
  const wx = x0 + 240 + 56;
  put($("s1word"), { x: wx, y: 540 - 80 });
  L.letters1.forEach((el, i) => {
    const e = E.out4(seg(t, 2.15 + i * 0.05, 2.15 + i * 0.05 + 0.55));
    put(el, { y: (1 - e) * 60, o: e });
  });
  put($("s1by"), { x: wx + 10, y: 540 + 96, o: E.out3(seg(t, 3.0, 3.6)) });
  const g = $("s1glow");
  put(g, { x: 510 + (iconX - 840) * 0.4, y: 90, s: 1 + Math.sin(t * 1.6) * 0.04, o: E.out3(seg(t, 0, 1.6)) * 0.5 });
  put($("s1grid"), { y: -((t * 18) % 96), o: 0.07 });
}

function s2(t) {
  const lines = $("s2lines").querySelectorAll(".mask > span");
  lines.forEach((el, i) => {
    const e = E.out4(seg(t, 0.3 + i * 0.28, 0.3 + i * 0.28 + 0.7));
    put(el, { y: (1 - e) * 230, o: e > 0 ? 1 : 0 });
  });
  rise($("s2sub"), t, 1.9, 0.7, 30);
  const c1 = E.back(seg(t, 0.5, 1.4));
  put($("s2card"), { x: (1 - c1) * 380, y: Math.sin(t * 1.5) * 8, r: lerp(10, -3, c1), o: seg(t, 0.5, 0.8) });
  const c2 = E.back(seg(t, 0.95, 1.8));
  put($("s2card2"), { x: (1 - c2) * 380, y: Math.sin(t * 1.5 + 1.2) * 7, r: lerp(8, 3, c2), o: seg(t, 0.95, 1.25) });
}

function s3(t) {
  rise($("s3title"), t, 0.2, 0.7, 30);
  const e = E.out4(seg(t, 0.1, 1.25));
  const win = $("s3win");
  const z = E.io3(seg(t, 2.1, 3.3));
  win.style.transform = `translateY(${(1 - e) * 900}px) scale(${lerp(0.94, 1, e) * lerp(1, 1.5, z)})`;
  win.style.opacity = seg(t, 0.1, 0.4);
  const c = E.back(seg(t, 2.9, 3.5));
  put($("s3chip"), { s: lerp(0.8, 1, c), o: seg(t, 2.9, 3.2), y: (1 - c) * 20 });
}

function s4(t) {
  rise($("s4pill"), t, 0.1, 0.5, 24);
  rise($("s4h"), t, 0.25, 0.7, 40);
  rise($("s4p"), t, 0.55, 0.7, 30);
  const e = E.out4(seg(t, 0.2, 1.0));
  put($("s4card"), { x: (1 - e) * 140, o: e });
  const pos = t < 2.6 ? lerp(1, 0.08, E.io3(seg(t, 1.2, 2.6))) : lerp(0.08, 0.55, E.io3(seg(t, 3.3, 4.2)));
  $("s4before").style.width = pos * 800 + "px";
  $("s4bar").style.left = pos * 800 + "px";
  $("s4knob").style.left = pos * 800 + "px";
  $("s4lb1").style.opacity = clamp((pos - 0.18) * 6);
  $("s4lb2").style.opacity = clamp((0.82 - pos) * 6);
  const p = E.back(seg(t, 1.4, 2.0));
  put($("s4panel"), { s: lerp(0.9, 1, p), y: (1 - p) * 30, o: seg(t, 1.4, 1.7) });
  ["s4r1", "s4r2", "s4r3"].forEach((id, i) => rise($(id), t, 1.7 + i * 0.18, 0.5, 18));
}

function s5(t) {
  rise($("s5pill"), t, 0.1, 0.5, 24);
  rise($("s5h"), t, 0.25, 0.7, 40);
  rise($("s5p"), t, 0.55, 0.7, 30);
  const e = E.out4(seg(t, 0.2, 1.0));
  put($("s5card"), { x: (1 - e) * 140, o: e });
  const r = E.io3(seg(t, 2.4, 3.5)) * 760;
  $("s5after").style.clipPath = r > 0 ? `circle(${r}px at 50% 58%)` : "circle(0px at 50% 58%)";
  const c = E.back(seg(t, 0.9, 1.5));
  put($("s5chip"), { s: lerp(0.85, 1, c), o: seg(t, 0.9, 1.2) });
  $("s5chiptxt").textContent = t < 2.4 ? "Sélection du sujet" : "Arrière-plan supprimé · masque réversible";
}

function s6(t) {
  rise($("s6pill"), t, 0.1, 0.5, 24);
  rise($("s6h"), t, 0.25, 0.7, 40);
  rise($("s6p"), t, 0.55, 0.7, 30);
  const e = E.out4(seg(t, 0.2, 1.0));
  put($("s6card"), { x: (1 - e) * 140, o: e });
  const labels = ["Fond", "Cercles et contour", "Bouton arrondi", "Titre", "Texte"];
  let step = 0;
  for (let i = 0; i < 5; i++) {
    const a = 1.0 + i * 0.85;
    const o = i === 0 ? 1 : seg(t, a, a + 0.3);
    const pop = i === 0 ? 1 : E.out3(seg(t, a, a + 0.5));
    const img = $("s6i" + i);
    img.style.opacity = o;
    img.style.transform = `scale(${lerp(1.04, 1, pop)})`;
    if (t >= a) step = i;
  }
  const a = 1.0 + step * 0.85;
  const c = E.back(seg(t, a, a + 0.4));
  put($("s6chip"), { s: lerp(0.94, 1, c), o: seg(t, 0.8, 1.1) });
  $("s6chiptxt").textContent = labels[step];
}

function s7(t) {
  rise($("s7pill"), t, 0.1, 0.5, 24);
  rise($("s7h"), t, 0.25, 0.7, 40);
  rise($("s7p"), t, 0.55, 0.7, 30);
  const e = E.out4(seg(t, 0.2, 1.0));
  put($("s7card"), { x: (1 - e) * 140, o: e });
  const e2 = E.out4(seg(t, 0.5, 1.2));
  put($("s7ui"), { x: (1 - e2) * 140, o: e2 });
  const p = E.io3(seg(t, 1.1, 3.1));
  const idx = Math.min(5, Math.floor(p * 5.999));
  for (let i = 0; i < 6; i++) $("s7i" + i).style.opacity = i === idx ? 1 : 0;
  const track = 740;
  $("s7fill").style.width = p * track + "px";
  $("s7kn").style.left = p * track + "px";
  $("s7val").textContent = Math.round(3 + p * 33);
}

function s8(t) {
  rise($("s8h"), t, 0.1, 0.7, 40);
  [...$("s8grid").children].forEach((el, i) => {
    const a = 0.45 + i * 0.14;
    const e = E.back(seg(t, a, a + 0.55));
    put(el, { s: lerp(0.86, 1, e), y: (1 - e) * 36, o: seg(t, a, a + 0.25) });
  });
}

function s9(t) {
  [...$("s9lines").children].forEach((el, i) => {
    const a = 0.2 + i * 0.5;
    const e = E.expo(seg(t, a, a + 0.7));
    put(el, { x: (1 - e) * -140, s: lerp(1.04, 1, e), o: seg(t, a, a + 0.25) });
  });
}

function s10(t) {
  const L = layout;
  const total = 200 + 44 + L.w10;
  const x0 = (1920 - total) / 2;
  const pop = E.back(seg(t, 0.2, 0.9));
  put($("s10icon"), { x: x0, y: 250, s: lerp(0.5, 1, pop), o: seg(t, 0.2, 0.5) });
  put($("s10word"), { x: x0 + 244, y: 252 });
  L.letters10.forEach((el, i) => {
    const e = E.out4(seg(t, 0.7 + i * 0.05, 0.7 + i * 0.05 + 0.55));
    put(el, { y: (1 - e) * 60, o: e });
  });
  const b = E.out4(seg(t, 1.6, 2.2));
  put($("s10btn"), { y: (1 - b) * 70, o: b });
  put($("s10by"), { o: E.out3(seg(t, 2.5, 3.1)) });
  put($("s10glow"), { s: 1 + Math.sin(t * 1.3) * 0.05, o: 0.45 });
}

const UPDATERS = [s1, s2, s3, s4, s5, s6, s7, s8, s9, s10];

window.seek = (frame) => {
  const t = frame / FPS;
  scenes.forEach((el, i) => {
    const a = STARTS[i];
    const b = (STARTS[i + 1] ?? TOTAL + 1) + WIPE + 0.05;
    const on = t >= a && t < b;
    el.style.display = on ? "block" : "none";
    if (!on) return;
    el.style.zIndex = i;
    if (i > 0) {
      const p = E.io3(seg(t, a, a + WIPE));
      el.style.clipPath = p >= 1 ? "none" : `circle(${p * 1500}px at 50% 58%)`;
    }
    UPDATERS[i](t - a);
  });
  $("fade").style.opacity = clamp((t - (TOTAL - 0.6)) / 0.6) + (1 - clamp(t / 0.4)) * 1;
};

window.__ready = (async () => {
  await document.fonts.ready;
  await Promise.all([...document.images].map((i) => (i.complete ? 0 : new Promise((r) => (i.onload = i.onerror = r)))));
  await Promise.all([400, 500, 600].map((w) => document.fonts.load(`${w} 30px Inter`)).concat(document.fonts.load('700 30px "Inter Display"')));
  init();
  window.seek(0);
  return true;
})();
