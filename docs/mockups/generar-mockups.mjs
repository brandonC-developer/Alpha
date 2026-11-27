// Genera los mockups HTML estáticos del Gestor de Tokens en docs/mockups/.
import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const OUT = process.argv[2];
mkdirSync(OUT, { recursive: true });

// ---------- datos de ejemplo deterministas ----------
let seed = 7;
const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const days = [];
for (let i = 0; i < 30; i++) {
  const d = new Date(Date.UTC(2026, 9, 28 + i));
  const wk = d.getUTCDay();
  const base = wk === 0 ? 0.15 : wk === 6 ? 0.9 : 0.55;
  const v = i === 9 || i === 10 ? 0.05 : base * (0.4 + rnd() * 1.1);
  days.push({ label: `${d.getUTCDate()}/${d.getUTCMonth() + 1}`, input: v * 70e6, output: v * 0.38e6 * (0.7 + rnd() * 0.6) });
}
const heat = Array.from({ length: 7 }, (_, r) => Array.from({ length: 24 }, (_, h) => {
  const work = h >= 9 && h <= 23 ? 1 : h <= 1 ? 0.5 : 0.05;
  const weekend = r >= 5 ? 1.25 : 1;
  return Math.min(4, Math.round(work * weekend * rnd() * 4.4));
}));

// ---------- utilidades ----------
const fmt = (n) => {
  const u = [[1e9, 'B'], [1e6, 'M'], [1e3, 'K']];
  for (const [d, s] of u) if (n >= d) return (n / d).toFixed(1).replace('.', ',') + ' ' + s;
  return String(Math.round(n));
};
const spark = (vals, w = 300, h = 34) => {
  const max = Math.max(...vals);
  const bw = w / vals.length;
  return `<svg class="spark" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true">${vals.map((v, i) => {
    const bh = Math.max(2, (v / max) * h);
    return `<rect x="${(i * bw + 1).toFixed(1)}" y="${(h - bh).toFixed(1)}" width="${(bw - 2).toFixed(1)}" height="${bh.toFixed(1)}" rx="1"/>`;
  }).join('')}</svg>`;
};

const NAV = [
  ['Panel', 'Panel.html', 'M3 13h8V3H3v10zm0 8h8v-6H3v6zm10 0h8V11h-8v10zm0-18v6h8V3h-8z'],
  ['Costes', 'Costes.html', 'M12 1v22M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6'],
  ['Planes', 'Planes.html', 'M4 19h16M4 15h10M4 11h14M4 7h6'],
  ['Sesiones', 'Sesiones.html', 'M4 6h16M4 12h16M4 18h10'],
  ['Fuentes', 'Fuentes.html', 'M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 17h6'],
  ['Ajustes', 'Ajustes.html', 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z'],
];
const nav = (active) => `
<nav class="nav" aria-label="Principal">
  <div class="brand"><span class="logo" aria-hidden="true"></span>Gestor de Tokens</div>
  <ul>${NAV.map(([t, f, d], i) => `
    <li><a href="${f}"${t === active ? ' aria-current="page"' : ''}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="${d}"/></svg>${t}<kbd>Ctrl ${i + 1}</kbd></a></li>`).join('')}
  </ul>
  <!-- datos: get_today_summary → windows -->
  <div class="nav-foot">
    <div class="mini"><span>Claude Pro · 5 h</span><b>62 %</b></div>
    <div class="minibar"><i style="width:62%"></i></div>
    <div class="mini"><span>Google AI Pro · 5 h</span><b class="t-warn">81 %</b></div>
    <div class="minibar warn"><i style="width:81%"></i></div>
    <p class="faint">Actualizado hace 4 s</p>
  </div>
</nav>`;

const page = (title, active, body) => `<!doctype html>
<html lang="es" data-theme="dark">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${title} · Gestor de Tokens (mockup)</title>
<link rel="stylesheet" href="mockups.css">
</head>
<body>
<div class="shell">
${nav(active)}
<main class="main">
${body}
</main>
</div>
<button class="theme" onclick="document.documentElement.dataset.theme = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark'">Cambiar tema</button>
</body>
</html>
`;

const ranges = (sel = '30 días') => `<div class="seg" role="group" aria-label="Rango">${['7 días', '30 días', '90 días', 'Todo'].map((r) => `<button${r === sel ? ' aria-pressed="true"' : ''}>${r}</button>`).join('')}</div>`;

const kpi = (label, value, sub, delta, vals, cls = '') => `
  <article class="card kpi">
    <header><h2 class="label">${label}</h2><span class="delta ${cls}">${delta}</span></header>
    <p class="big">${value}</p>
    <p class="sub">${sub}</p>
    ${spark(vals)}
  </article>`;

// ---------- Panel ----------
const W = 760, H = 250, mid = 150;
const maxIn = Math.max(...days.map((d) => d.input));
const maxOut = Math.max(...days.map((d) => d.output));
const bw = (W - 60) / days.length;
const byDay = `<svg class="chart" viewBox="0 0 ${W} ${H}" role="img" aria-label="Tokens por día: máximo de entrada 79,3 M el 15/11">
  <text x="0" y="16" class="axis">${fmt(maxIn)}</text><text x="0" y="30" class="axis">entrada</text>
  <text x="0" y="${H - 34}" class="axis">${fmt(maxOut)}</text><text x="0" y="${H - 20}" class="axis accent">salida</text>
  <line x1="60" x2="${W}" y1="${mid}" y2="${mid}" class="baseline"/>
  ${days.map((d, i) => {
    const x = 60 + i * bw + 2;
    const hi = (d.input / maxIn) * (mid - 10);
    const ho = (d.output / maxOut) * (H - mid - 30);
    return `<rect class="b-in" x="${x.toFixed(1)}" y="${(mid - hi).toFixed(1)}" width="${(bw - 4).toFixed(1)}" height="${hi.toFixed(1)}"/><rect class="b-out" x="${x.toFixed(1)}" y="${mid + 1}" width="${(bw - 4).toFixed(1)}" height="${ho.toFixed(1)}"/>`;
  }).join('')}
  <text x="60" y="${H - 2}" class="axis">28/10</text><text x="${60 + 15 * bw}" y="${H - 2}" class="axis">12/11</text><text x="${W - 30}" y="${H - 2}" class="axis">hoy</text>
</svg>`;
const DAYS = ['L', 'M', 'X', 'J', 'V', 'S', 'D'];
const heatSvg = `<svg class="chart" viewBox="0 0 560 250" role="img" aria-label="Cuándo trabajan: pico el sábado a las 18 h">
  ${heat.map((row, r) => `<text x="0" y="${r * 30 + 20}" class="axis">${DAYS[r]}</text>` + row.map((v, h) => `<rect class="h${v}" x="${20 + h * 22.5}" y="${r * 30 + 4}" width="20" height="26" rx="3"/>`).join('')).join('')}
  ${[0, 6, 12, 18].map((h) => `<text x="${24 + h * 22.5}" y="230" class="axis">${h}</text>`).join('')}
  <text x="410" y="248" class="axis">menos</text>${[0, 1, 2, 3, 4].map((v) => `<rect class="h${v}" x="${452 + v * 16}" y="238" width="12" height="12" rx="2"/>`).join('')}<text x="536" y="248" class="axis">más</text>
</svg>`;

const models = [
  ['anthropic', 'claude-opus-5-5', '812 M', '3,9 M', '201', '298,40 $'],
  ['google', 'gemini-4-neon-xhigh', '214 M', '1,1 M', '64', '—'],
  ['google', 'gemini-4-hydrogen-high', '168 M', '0,9 M', '58', '—'],
  ['google', 'gemini-4-hydrogen-xhigh', '41 M', '0,3 M', '14', '—'],
  ['deepseek', 'deepseek-v4-flash', '12,4 M', '0,2 M', '11', '3,84 $'],
  ['local', 'qwen3:4b', '0,9 M', '0,1 M', '4', '0,00 $'],
];
const panel = page('Panel', 'Panel', `
<header class="head">
  <div><h1>Uso de agentes</h1><p class="muted">28 oct – hoy</p></div>
  ${ranges()}
</header>
<!-- estado: filtro activo → chip «Antigravity ×» aquí; vacío → «Aún no hay uso registrado en este rango» + «Ver todo» -->
<section class="kpis" aria-label="Resumen">
  <!-- datos: get_dashboard → kpis -->
  ${kpi('Tokens de entrada', '1,25 B', '97 % desde caché', '+212 %', days.map((d) => d.input), 'up')}
  ${kpi('Tokens de salida', '6,5 M', '18,4 K por turno', '+148 %', days.map((d) => d.output), 'up')}
  ${kpi('Turnos', '352', '61 sesiones', '+87 %', days.map((d) => d.input * 0.8 + d.output * 40), 'up')}
  ${kpi('Tiempo de agente', '58 h', 'hasta 4 a la vez', '−12 %', days.map((d) => d.output), 'down')}
</section>
<section class="grid2">
  <article class="card">
    <header class="sec"><h2 class="label">Tokens por día</h2><span class="muted small">29 de 30 días con uso · 41,7 M/día</span></header>
    <!-- datos: get_dashboard → byDay -->
    ${byDay}
  </article>
  <article class="card">
    <header class="sec"><h2 class="label">Cuándo trabajan</h2><span class="muted small">pico: sábado 18 h</span></header>
    <!-- datos: get_dashboard → heatmap, peak -->
    ${heatSvg}
  </article>
</section>
<section class="grid2">
  <article class="card">
    <header class="sec"><h2 class="label">Modelos</h2><span class="muted small">6 modelos</span></header>
    <!-- datos: get_dashboard → byProvider, byModel -->
    <div class="split" aria-label="Anthropic 65 %, Google 34 %, DeepSeek 1 %, local 0 %"><i class="p-anthropic" style="flex:65"></i><i class="p-google" style="flex:34"></i><i class="p-deepseek" style="flex:1"></i></div>
    <p class="legend"><span><i class="dot p-anthropic"></i>Anthropic 65 %</span><span><i class="dot p-google"></i>Google 34 %</span><span><i class="dot p-deepseek"></i>DeepSeek 1 %</span><span><i class="dot p-local"></i>Local 0 %</span></p>
    <table>
      <thead><tr><th>Modelo</th><th class="n">Entrada</th><th class="n">Salida</th><th class="n">Turnos</th><th class="n">Coste</th></tr></thead>
      <tbody>${models.map(([p, m, i, o, t, c]) => `<tr><td><i class="dot p-${p}"></i><a href="#">${m}</a></td><td class="n">${i}</td><td class="n">${o}</td><td class="n">${t}</td><td class="n">${c === '—' ? '<span class="tag">sin precio</span>' : c}</td></tr>`).join('')}</tbody>
    </table>
  </article>
  <article class="card">
    <header class="sec"><h2 class="label">Agentes</h2><span class="muted small">clic en uno para ver solo su uso</span></header>
    <!-- datos: get_dashboard → byTool -->
    <table class="agents">
      <thead><tr><th>Agente</th><th></th><th class="n">Entrada</th><th class="n">Salida</th><th class="n">Tiempo</th></tr></thead>
      <tbody>
        <tr><td><a href="#"><i class="dot p-anthropic"></i>Claude Code</a></td><td class="barcell"><span class="hbar" style="width:65%"></span></td><td class="n">812 M</td><td class="n">3,9 M</td><td class="n">31 h</td></tr>
        <tr><td><a href="#"><i class="dot p-google"></i>Antigravity</a></td><td class="barcell"><span class="hbar" style="width:34%"></span></td><td class="n">423 M</td><td class="n">2,3 M</td><td class="n">25 h</td></tr>
        <tr><td><a href="#"><i class="dot p-deepseek"></i>opencode</a></td><td class="barcell"><span class="hbar" style="width:1.5%"></span></td><td class="n">13,3 M</td><td class="n">0,3 M</td><td class="n">2 h</td></tr>
      </tbody>
    </table>
  </article>
</section>`);

// ---------- Costes ----------
const cw = 760, ch = 220;
const costDays = days.map((d, i) => ({ eq: (d.input / 1e6) * 0.33 + (d.output / 1e6) * 8, real: i > 20 ? rnd() * 0.6 : 0 }));
const maxC = Math.max(...costDays.map((c) => c.eq + c.real));
const cbw = (cw - 50) / costDays.length;
const costSvg = `<svg class="chart" viewBox="0 0 ${cw} ${ch}" role="img" aria-label="Coste por día: real en barras sólidas, equivalente API rayado">
  <defs><pattern id="rayas" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="6" height="6" class="hatch-bg"/><line x1="0" y1="0" x2="0" y2="6" class="hatch"/></pattern></defs>
  <text x="0" y="14" class="axis">${maxC.toFixed(0)} $</text><text x="0" y="${ch - 24}" class="axis">0 $</text>
  <line x1="50" x2="${cw}" y1="${ch - 28}" y2="${ch - 28}" class="baseline"/>
  ${costDays.map((c, i) => {
    const x = 50 + i * cbw + 2;
    const he = (c.eq / maxC) * (ch - 40);
    const hr = (c.real / maxC) * (ch - 40);
    return `<rect x="${x.toFixed(1)}" y="${(ch - 28 - he - hr).toFixed(1)}" width="${(cbw - 4).toFixed(1)}" height="${he.toFixed(1)}" fill="url(#rayas)"/><rect class="b-real" x="${x.toFixed(1)}" y="${(ch - 28 - hr).toFixed(1)}" width="${(cbw - 4).toFixed(1)}" height="${hr.toFixed(1)}"/>`;
  }).join('')}
  <text x="50" y="${ch - 6}" class="axis">28/10</text><text x="${cw - 30}" y="${ch - 6}" class="axis">hoy</text>
</svg>`;
const costes = page('Costes', 'Costes', `
<header class="head">
  <div><h1>Costes</h1><p class="muted">28 oct – hoy · precios vigentes en la fecha de cada llamada</p></div>
  <div class="row">${ranges()}<div class="seg" role="group" aria-label="Tipo de coste"><button aria-pressed="true">Ambos</button><button>Real</button><button>Equivalente API</button></div></div>
</header>
<section class="kpis" aria-label="Resumen de costes">
  <!-- datos: get_costs → realUsd, apiEquivalentUsd, feesUsd, yield -->
  ${kpi('Gasto real', '3,84 $', 'DeepSeek · local 0 $', '+3,84 $', costDays.map((c) => c.real + 0.001))}
  ${kpi('Equivalente API', '298,40 $', 'Claude Pro · Antigravity sin precio', '+164 %', costDays.map((c) => c.eq), 'up')}
  ${kpi('Cuota pagada', '20,00 $', 'Claude Pro · Google AI Pro sin confirmar', '=', costDays.map(() => 1))}
  ${kpi('Rendimiento', '14,9×', 'equivalente / cuota', '+5,2×', costDays.map((c) => c.eq), 'up')}
</section>
<p class="notice">El equivalente API es una referencia: con tu suscripción no pagas por token</p>
<p class="notice warn"><b>412 llamadas sin precio</b> (gemini-4-neon-xhigh, gemini-4-hydrogen-high…). No se cuentan como 0. <a href="Ajustes.html#precios">Añadir precio</a></p>
<!-- estado: unpriced vacío → sin esta banda -->
<section class="grid2 wide-left">
  <article class="card">
    <header class="sec"><h2 class="label">Coste por día</h2><p class="legend"><span><i class="sw solid"></i>Real</span><span><i class="sw hatched"></i>Equivalente API</span></p></header>
    <!-- datos: get_costs → byDay -->
    ${costSvg}
  </article>
  <article class="card">
    <header class="sec"><h2 class="label">Rendimiento por suscripción</h2></header>
    <!-- datos: get_costs → yield -->
    <div class="yield"><div><b>Claude Pro</b><span class="muted small">20,00 $/mes · equivalente 298,40 $</span></div><p class="big2">14,9×</p></div>
    <div class="yield"><div><b>Google AI Pro</b><span class="muted small">cuota sin confirmar · 412 llamadas sin precio</span></div><p class="big2 muted">—</p></div>
    <h3 class="label mt">DeepSeek este mes</h3>
    <!-- datos: get_costs → deepseekBudget, monthProjection -->
    <div class="bar" role="progressbar" aria-valuenow="38" aria-valuemin="0" aria-valuemax="100" aria-label="Presupuesto de DeepSeek"><i style="width:38%"></i></div>
    <p class="small"><b>3,84 $</b> de 10,00 $ · saldo 8,42 $ · proyección del mes 5,10 $</p>
  </article>
</section>
<section class="grid2">
  <article class="card">
    <header class="sec"><h2 class="label">Por modelo</h2></header>
    <table><thead><tr><th>Modelo</th><th>Tipo</th><th class="n">Turnos</th><th class="n">Coste</th></tr></thead><tbody>
      <tr><td><i class="dot p-anthropic"></i>claude-opus-5-5</td><td><span class="tag hatched-tag">equivalente</span></td><td class="n">201</td><td class="n">298,40 $</td></tr>
      <tr><td><i class="dot p-deepseek"></i>deepseek-v4-flash</td><td><span class="tag real">real</span></td><td class="n">11</td><td class="n">3,84 $</td></tr>
      <tr><td><i class="dot p-local"></i>qwen3:4b</td><td><span class="tag real">real</span></td><td class="n">4</td><td class="n">0,00 $</td></tr>
      <tr><td><i class="dot p-google"></i>gemini-4-neon-xhigh</td><td><span class="tag">sin precio</span></td><td class="n">64</td><td class="n">—</td></tr>
    </tbody></table>
  </article>
  <article class="card">
    <header class="sec"><h2 class="label">Por proyecto</h2></header>
    <!-- datos: get_costs → byProject -->
    <table><thead><tr><th>Proyecto</th><th class="n">Sesiones</th><th class="n">Media</th><th class="n">Coste</th></tr></thead><tbody>
      <tr><td>AZ-900</td><td class="n">24</td><td class="n">7,10 $</td><td class="n">170,40 $</td></tr>
      <tr><td>Alpha</td><td class="n">9</td><td class="n">9,92 $</td><td class="n">89,30 $</td></tr>
      <tr><td>Portafolio</td><td class="n">14</td><td class="n">2,90 $</td><td class="n">40,54 $</td></tr>
      <tr><td class="muted">(sin proyecto)</td><td class="n">14</td><td class="n">—</td><td class="n">—</td></tr>
    </tbody></table>
  </article>
</section>`);

// ---------- Planes ----------
const wbar = (pct, cls = '') => `<div class="bar ${cls}" role="progressbar" aria-valuenow="${pct}" aria-valuemin="0" aria-valuemax="100"><i style="width:${pct}%"></i><span class="mark" style="left:80%"></span></div>`;
const planes = page('Planes', 'Planes', `
<header class="head"><div><h1>Planes y límites</h1><p class="muted">Ventana de 5 h desde tu primer mensaje · semana = últimas 168 h</p></div></header>
<section class="plans">
  <!-- datos: get_plans -->
  <article class="card plan">
    <header class="sec"><h2><i class="dot p-anthropic"></i>Claude Pro</h2><span class="muted small">20,00 $/mes · Claude Code</span></header>
    <div class="win"><div class="winhead"><b>Ventana de 5 h</b><span><b>62 %</b> · reinicia 15:10</span></div>${wbar(62)}<p class="small muted">7,8 $ equivalentes de 12,6 $ estimados · a este ritmo no se agota antes del reinicio</p></div>
    <div class="win"><div class="winhead"><b>Semanal</b><span><b>41 %</b></span></div>${wbar(41)}<p class="small muted">calibrado 2 veces · última hoy 11:02</p></div>
    <p class="notice small">No incluye lo que uses en la web o la app móvil (claude.ai, Gemini), que comparten este límite</p>
    <footer class="actions"><button class="btn">Calibrar</button><button class="btn ghost">Editar cuota</button></footer>
  </article>
  <article class="card plan alert">
    <header class="sec"><h2><i class="dot p-google"></i>Google AI Pro</h2><span class="muted small">cuota sin confirmar · Antigravity</span></header>
    <div class="win"><div class="winhead"><b>Ventana de 5 h</b><span class="t-warn"><b>81 %</b> · se agota ~14:20</span></div>${wbar(81, 'warn')}<p class="small muted">Reinicia 16:40 · 412 llamadas sin precio no cuentan</p></div>
    <div class="win"><div class="winhead"><b>Semanal</b><span class="muted">Sin calibrar</span></div><div class="bar off"><i style="width:0"></i></div><p class="small muted">Sin calibrar: se muestra el consumo, pero no el porcentaje · 423 M tokens esta semana</p></div>
    <p class="notice small">No incluye lo que uses en la web o la app móvil (claude.ai, Gemini), que comparten este límite</p>
    <footer class="actions"><button class="btn">Calibrar</button><button class="btn ghost">Editar cuota</button></footer>
  </article>
  <article class="card plan">
    <header class="sec"><h2><i class="dot p-deepseek"></i>DeepSeek</h2><span class="muted small">pago por uso · opencode</span></header>
    <div class="row3"><div><span class="label">Saldo</span><p class="big2">8,42 $</p></div><div><span class="label">Este mes</span><p class="big2">3,84 $</p></div><div><span class="label">Presupuesto</span><p class="big2">10 $</p></div></div>
    ${wbar(38)}
    <p class="small muted">Saldo consultado hace 6 min en api.deepseek.com</p>
    <footer class="actions"><button class="btn ghost">Editar presupuesto</button></footer>
  </article>
  <article class="card plan">
    <header class="sec"><h2><i class="dot p-local"></i>Local (Ollama)</h2><span class="muted small">sin límite · opencode</span></header>
    <div class="row3"><div><span class="label">Tokens este mes</span><p class="big2">1,0 M</p></div><div><span class="label">Coste</span><p class="big2">0 $</p></div><div><span class="label">Modelo</span><p class="big2 small2">qwen3:4b</p></div></div>
  </article>
</section>
<!-- estado: diálogo de calibración -->
<dialog open class="card dialog" aria-labelledby="dlg-t">
  <h2 id="dlg-t">Calibrar Claude Pro · ventana de 5 h</h2>
  <p class="muted">En Claude Code escribe <code>/usage</code> y copia el porcentaje de la sesión actual</p>
  <label>Porcentaje que muestra<input type="number" min="1" max="100" value="62"> %</label>
  <p class="small muted">Ahora mismo la app mide 7,8 $ equivalentes en esta ventana → límite estimado 12,6 $</p>
  <footer class="actions"><button class="btn">Guardar</button><button class="btn ghost">Cancelar</button></footer>
</dialog>`);

// ---------- Sesiones ----------
const sess = [
  ['26/11 13:02', 'Antigravity', '—', 'gemini-4-neon-xhigh', '2 h 10 min', '58', '41,2 M', 'sin precio', true],
  ['26/11 10:15', 'Claude Code', 'Alpha', 'claude-opus-5-5', '1 h 42 min', '37', '28,9 M', '14,20 $ eq.', false],
  ['25/11 19:40', 'Claude Code', 'AZ-900', 'claude-opus-5-5', '58 min', '22', '17,4 M', '8,65 $ eq.', false],
  ['25/11 18:05', 'opencode', 'Alpha', 'deepseek-v4-flash', '24 min', '9', '6,1 M', '0,94 $', false],
  ['25/11 11:30', 'Antigravity', '—', 'gemini-4-hydrogen-high', '1 h 05 min', '31', '12,8 M', 'sin precio', false],
  ['24/11 21:12', 'opencode', 'Alpha', 'qwen3:4b', '6 min', '4', '0,3 M', '0,00 $', false],
];
const sesiones = page('Sesiones', 'Sesiones', `
<header class="head"><div><h1>Sesiones</h1><p class="muted">61 sesiones · 1 fuera de lo normal</p></div><div class="row">${ranges('7 días')}<label class="check"><input type="checkbox"> Solo anómalas</label></div></header>
<div class="grid-side">
<article class="card">
  <!-- datos: get_sessions -->
  <table class="sortable">
    <thead><tr><th aria-sort="descending">Inicio ▾</th><th>Agente</th><th>Proyecto</th><th>Modelo principal</th><th class="n">Duración</th><th class="n">Turnos</th><th class="n">Tokens</th><th class="n">Coste</th></tr></thead>
    <tbody>${sess.map(([s, a, p, m, d, t, tk, c, an]) => `<tr${an ? ' class="anom" aria-label="Sesión anómala: 12× tu media"' : ''}${s === '26/11 13:02' ? ' aria-selected="true"' : ''}><td>${an ? '<span class="tag bad">12×</span> ' : ''}${s}</td><td>${a}</td><td>${p}</td><td>${m}</td><td class="n">${d}</td><td class="n">${t}</td><td class="n">${tk}</td><td class="n">${c === 'sin precio' ? '<span class="tag">sin precio</span>' : c}</td></tr>`).join('')}</tbody>
  </table>
</article>
<aside class="card side" aria-label="Desglose de la sesión">
  <!-- datos: get_session_detail · Esc cierra -->
  <header class="sec"><h2 class="label">Antigravity · 26/11 13:02</h2><button class="x" aria-label="Cerrar">×</button></header>
  <p class="small muted">58 llamadas · 41,2 M tokens · 12× tu media · sin contenido: solo cifras</p>
  <table class="compact"><thead><tr><th>Hora</th><th>Modelo</th><th class="n">Entrada</th><th class="n">Caché</th><th class="n">Salida</th></tr></thead><tbody>
    <tr><td>13:02:11</td><td>neon-xhigh</td><td class="n">14,2 K</td><td class="n">0</td><td class="n">799</td></tr>
    <tr><td>13:04:05</td><td>neon-xhigh</td><td class="n">11,9 K</td><td class="n">14,2 K</td><td class="n">509</td></tr>
    <tr class="hot"><td>13:41:37</td><td>hydrogen-high</td><td class="n">1,9 M</td><td class="n">212 K</td><td class="n">3,1 K</td></tr>
    <tr><td>13:42:20</td><td>hydrogen-high</td><td class="n">48,0 K</td><td class="n">2,1 M</td><td class="n">1,2 K</td></tr>
    <tr><td colspan="5" class="muted small">… 54 llamadas más</td></tr>
  </tbody></table>
</aside>
</div>`);

// ---------- Fuentes ----------
const fuentes = page('Fuentes', 'Fuentes', `
<header class="head"><div><h1>Fuentes</h1><p class="muted">La app solo lee estas carpetas; nunca escribe en ellas ni guarda el contenido</p></div><button class="btn">Reescanear <kbd>R</kbd></button></header>
<section class="sources">
  <!-- datos: get_sources -->
  <article class="card src"><header class="sec"><h2><i class="dot p-anthropic"></i>Claude Code</h2><span class="tag ok">Leyendo</span></header>
    <code>%USERPROFILE%\\.claude\\projects</code>
    <dl><div><dt>Archivos</dt><dd>214</dd></div><div><dt>Llamadas</dt><dd>9.812</dd></div><div><dt>Último evento</dt><dd>hace 4 s</dd></div><div><dt>Errores</dt><dd>0</dd></div></dl></article>
  <article class="card src"><header class="sec"><h2><i class="dot p-google"></i>Antigravity</h2><span class="tag ok">Leyendo</span></header>
    <code>%USERPROFILE%\\.gemini\\antigravity\\brain</code>
    <dl><div><dt>Conversaciones</dt><dd>21</dd></div><div><dt>Llamadas</dt><dd>3.140</dd></div><div><dt>Último evento</dt><dd>hace 2 min</dd></div><div><dt>Errores</dt><dd>0</dd></div></dl></article>
  <article class="card src alert"><header class="sec"><h2><i class="dot p-deepseek"></i>opencode</h2><span class="tag warn">Con errores</span></header>
    <code>%USERPROFILE%\\.local\\share\\opencode</code>
    <dl><div><dt>Sesiones</dt><dd>12</dd></div><div><dt>Llamadas</dt><dd>96</dd></div><div><dt>Último evento</dt><dd>ayer</dd></div><div><dt>Errores</dt><dd class="t-warn">2</dd></div></dl>
    <p class="small">2 líneas no se pudieron leer</p>
    <ul class="errs small"><li><code>storage\\message\\ses_4f…\\msg_9a.json</code> línea 1: JSON incompleto</li><li><code>storage\\message\\ses_4f…\\msg_9b.json</code> línea 1: JSON incompleto</li></ul></article>
</section>
<!-- estado: fuente no encontrada → «No se encontró .gemini\\antigravity\\brain. ¿Está instalado Antigravity?» -->`);

// ---------- Ajustes ----------
const ajustes = page('Ajustes', 'Ajustes', `
<header class="head"><div><h1>Ajustes</h1></div></header>
<nav class="tabs" aria-label="Secciones"><a href="#general" aria-current="true">General</a><a href="#precios">Precios</a><a href="#alertas">Alertas</a><a href="#deepseek">DeepSeek</a><a href="#datos">Datos</a></nav>
<section class="card" id="general"><h2 class="label">General</h2>
  <div class="form"><label>Tema<select><option>Oscuro</option><option>Claro</option><option>Sistema</option></select></label><label class="check"><input type="checkbox"> Abrir con Windows (en la bandeja)</label></div></section>
<section class="card" id="precios"><header class="sec"><h2 class="label">Precios · $ por millón de tokens</h2><button class="btn ghost">Añadir precio</button></header>
  <!-- datos: get_prices · upsert_price → RecalcResult -->
  <table><thead><tr><th>Modelo</th><th>Desde</th><th class="n">Entrada</th><th class="n">Salida</th><th class="n">Escr. caché</th><th class="n">Lect. caché</th><th>Fuente</th></tr></thead><tbody>
    <tr><td>claude-opus-5-5*</td><td>01/01/2026</td><td class="n">4,00</td><td class="n">20,00</td><td class="n">5,00</td><td class="n">0,20</td><td><a href="#">Anthropic</a></td></tr>
    <tr><td>claude-sonnet-5-5* <span class="tag warn">aprox.</span></td><td>01/01/2026</td><td class="n">2,00</td><td class="n">10,00</td><td class="n">2,50</td><td class="n">0,10</td><td><a href="#">DevelopersDigest</a></td></tr>
    <tr><td>deepseek-v4-flash* <span class="tag warn">aprox.</span></td><td>01/01/2026</td><td class="n">0,14</td><td class="n">0,28</td><td class="n">0,00</td><td class="n">0,0028</td><td><a href="#">DeepSeek</a></td></tr>
    <tr class="missing"><td>gemini-4-* <span class="tag">sin precio</span></td><td colspan="5" class="muted">Google no publica precio de API para Neon, Hydrogen ni Carbon</td><td><button class="btn ghost small">Añadir</button></td></tr>
  </tbody></table>
  <p class="notice ok small">Recalculadas 120 llamadas: 4,10 $ → 5,30 $</p>
  <h3 class="label mt">Historial de recálculos</h3>
  <ul class="small muted"><li>26/11 12:04 · claude-opus-5-5* desde 20/11 · 120 llamadas · 4,10 $ → 5,30 $</li></ul></section>
<section class="card" id="alertas"><h2 class="label">Alertas</h2>
  <div class="form"><label>Ventana de 5 h<input value="80, 95"> %</label><label>Semanal<input value="80"> %</label><label>Presupuesto de DeepSeek<input value="80"> %</label><label class="check"><input type="checkbox" checked> Avisar de sesiones fuera de lo normal</label></div></section>
<section class="card" id="deepseek"><h2 class="label">DeepSeek</h2>
  <div class="form"><label>Clave de la API<input type="password" value="" placeholder="Guardada en el Administrador de credenciales de Windows"></label><p class="small"><span class="tag ok">Guardada</span> La clave nunca se muestra ni sale de este PC salvo hacia api.deepseek.com.</p><div class="actions"><button class="btn">Cambiar</button><button class="btn ghost">Borrar</button></div></div></section>
<section class="card" id="datos"><h2 class="label">Datos</h2>
  <div class="actions"><button class="btn">Exportar CSV (FOCUS)</button><button class="btn ghost">Exportar JSON</button><button class="btn ghost">Importar JSON</button></div>
  <p class="small muted">Base de datos: %APPDATA%\\GestorTokens\\gestor.db · 41 MB · 13.048 llamadas desde el 02/09/2026</p></section>`);

// ---------- Bandeja ----------
const bandeja = `<!doctype html>
<html lang="es" data-theme="dark"><head><meta charset="utf-8"><title>Bandeja · Gestor de Tokens (mockup)</title><link rel="stylesheet" href="mockups.css"></head>
<body class="traybody">
<div class="taskbar"><span class="trayicon" title="Gestor de Tokens · hoy 4,2 M tokens · 1,84 $ equiv."><svg viewBox="0 0 36 36"><circle cx="18" cy="18" r="14" class="ring-bg"/><circle cx="18" cy="18" r="14" class="ring warn" stroke-dasharray="71 88"/></svg></span></div>
<div class="traymenu" role="menu">
  <div class="ti strong">Hoy: 4,2 M tokens · 1,84 $ equiv.</div><hr>
  <div class="ti"><span>Claude Pro (5 h)</span><span>62 % · reinicia 15:10</span></div>
  <div class="ti t-warn"><span>Google AI Pro (5 h)</span><span>81 % · se agota ~14:20</span></div>
  <div class="ti"><span>DeepSeek</span><span>8,42 $ de saldo</span></div><hr>
  <div class="ti act">Abrir panel</div><div class="ti act">Reescanear</div><div class="ti act">Pausar avisos 1 h</div><hr>
  <div class="ti act">Salir</div>
</div>
<div class="toast"><b>Google AI Pro: 80 % de la ventana de 5 h</b><span>Se reinicia a las 16:40. A este ritmo se agota a las 14:20.</span></div>
</body></html>`;

// ---------- índice ----------
const SCREENS = [['Panel', 'Panel.html'], ['Costes', 'Costes.html'], ['Planes', 'Planes.html'], ['Sesiones', 'Sesiones.html'], ['Fuentes', 'Fuentes.html'], ['Ajustes', 'Ajustes.html'], ['Bandeja y notificación', 'Bandeja.html']];
const index = `<!doctype html>
<html lang="es" data-theme="dark"><head><meta charset="utf-8"><title>Mockups · Gestor de Tokens</title><link rel="stylesheet" href="mockups.css"></head>
<body class="indexbody"><h1>Mockups · Gestor de Tokens</h1><p class="muted">Datos de ejemplo. Tema oscuro por defecto; botón «Cambiar tema» abajo a la derecha en cada pantalla. Contratos en <code>docs/05-ui.md</code>.</p>
<ol class="index">${SCREENS.map(([t, f]) => `<li><a href="${f}">${t}</a><iframe src="${f}" title="${t}" loading="lazy" tabindex="-1"></iframe></li>`).join('')}</ol></body></html>`;

const files = { 'Panel.html': panel, 'Costes.html': costes, 'Planes.html': planes, 'Sesiones.html': sesiones, 'Fuentes.html': fuentes, 'Ajustes.html': ajustes, 'Bandeja.html': bandeja, 'index.html': index };
for (const [f, c] of Object.entries(files)) writeFileSync(join(OUT, f), c);
console.log('ok', Object.keys(files).length);
