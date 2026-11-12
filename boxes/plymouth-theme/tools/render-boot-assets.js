// Renders Plymouth/GRUB bitmaps straight from the design mockup (mockups/boot.html)
// at the 1920x1080 reference size, plus @2x, and writes measured geometry to JSON.
// Usage: node boxes/plymouth-theme/tools/render-boot-assets.js <handoff-dir> <out-dir>
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const [handoff, out] = process.argv.slice(2);
const W = 1920, H = 1080;

const css = `
  html, body { background: transparent !important; }
  body { padding: 0 !important; margin: 0 !important; }
  .pane { background: transparent !important; }
  .preview, .sample { display: none !important; }
  .screen { width: ${W}px !important; height: ${H}px !important; max-width: none !important;
            aspect-ratio: auto !important; border-radius: 0 !important; box-shadow: none !important;
            margin: 0 !important; --s-bg: transparent !important; background: transparent !important; }
  *, *::before, *::after { animation: none !important; transition: none !important; }
  .pw .go { opacity: 1 !important; }
  /* Shrimp: the logo keeps one size in every Plymouth state (14% of width) */
  .ply.pw .logo { width: 14cqw !important; }
`;

async function run(scale) {
  const b = await chromium.launch();
  let p = await b.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: scale });
  await p.goto('file://' + path.resolve(handoff, 'mockups/boot.html'));
  await p.addStyleTag({ content: css });
  await p.evaluate(() => document.fonts.ready);
  await p.waitForTimeout(800);
  const geo = {};
  const suffix = scale === 1 ? '' : '@2x';
  const dir = path.join(out, 'plymouth');
  fs.mkdirSync(dir, { recursive: true });

  async function show(state) {
    await p.evaluate((s) => {
      document.querySelectorAll('.pane').forEach(el => el.hidden = el.dataset.pane !== s);
      document.querySelectorAll('.pane').forEach(el => el.classList.remove('show'));
    }, state);
    await p.waitForTimeout(200);
  }
  async function box(sel) {
    const el = await p.$(sel);
    const r = await el.boundingBox();
    const o = await (await p.$('.screen')).boundingBox();
    return { x: +(r.x - o.x).toFixed(2), y: +(r.y - o.y).toFixed(2), w: +(r.width).toFixed(2), h: +(r.height).toFixed(2) };
  }
  async function shot(sel, name, prep) {
    if (prep) await p.evaluate(prep);
    const el = await p.$(sel);
    await el.screenshot({ path: path.join(dir, name + suffix + '.png'), omitBackground: true });
  }

  // boot
  await show('ply-start');
  geo.boot = { logo: await box('[data-pane="ply-start"] .logo svg'), bar: await box('[data-pane="ply-start"] .pbar') };
  await shot('[data-pane="ply-start"] .logo svg', 'logo');

  // password
  await show('ply-haslo');
  await p.evaluate(() => {
    const d = document.querySelector('#dots');
    d.querySelectorAll('i').forEach(i => i.remove());
    const i = document.createElement('i'); d.insertBefore(i, d.firstChild);
    const j = document.createElement('i'); d.insertBefore(j, d.firstChild);
  });
  const P = '[data-pane="ply-haslo"]';
  geo.password = {
    logo: await box(P + ' .logo svg'),
    field: await box(P + ' .field'),
    lock: await box(P + ' .field > .lk'),
    dots: await box(P + ' .dots'),
    dot0: await box(P + ' .dots i:nth-of-type(1)'),
    dot1: await box(P + ' .dots i:nth-of-type(2)'),
    caret: await box(P + ' .caret'),
    go: await box(P + ' .go'),
    goicon: await box(P + ' .go svg'),
    cap: await box(P + ' .cap'),
  };
  await shot(P + ' .logo svg', 'logo-small');
  const noField = () => { document.querySelector('[data-pane="ply-haslo"] .field').style.background = 'transparent'; };
  const field = () => { document.querySelector('[data-pane="ply-haslo"] .field').style.background = ''; };
  await shot(P + ' .dots i', 'bullet', noField);
  await shot(P + ' .caret', 'caret');
  await shot(P + ' .cap', 'hint-password');
  // empty field and go circle without their glyphs (Adwaita icons are added at build time)
  await p.addStyleTag({ content: `${P} .field > * { visibility: hidden; } ${P} .field .go { visibility: visible; } ${P} .go svg { visibility: hidden; }` });
  await shot(P + ' .go', 'go');
  await p.addStyleTag({ content: `${P} .field .go { visibility: hidden; }` });
  await shot(P + ' .field', 'entry', field);

  // updates
  await show('ply-aktualizacja');
  const U = '[data-pane="ply-aktualizacja"]';
  geo.updates = { logo: await box(U + ' .logo svg'), bar: await box(U + ' .pbar'), title: await box(U + ' .cap b'), cap: await box(U + ' .cap') };
  await shot(U + ' .cap b', 'title-updates');
  const capStyle = await p.evaluate((s) => { const c = getComputedStyle(document.querySelector(s)); return { fontSize: c.fontSize, lineHeight: c.lineHeight, color: c.color }; }, U + ' .cap');
  geo.updates.capStyle = capStyle;

  // bar parts: round caps and a 1px body, track and fill colours from the mockup
  const barH = Math.round(geo.boot.bar.h);
  for (const [name, color] of [['track', '#2c2c2e'], ['fill', '#ffffff']]) {
    await p.setContent(`<html><body style="margin:0;background:transparent">
      <div id="cap" style="width:${barH}px;height:${barH}px;border-radius:50%;background:${color}"></div>
      <div id="body" style="width:${barH}px;height:${barH}px;background:${color}"></div></body></html>`);
    await (await p.$('#cap')).screenshot({ path: path.join(dir, `${name}-cap${suffix}.png`), omitBackground: true });
    await (await p.$('#body')).screenshot({ path: path.join(dir, `${name}-body${suffix}.png`), omitBackground: true });
  }

  // GRUB
  const gdir = path.join(out, 'grub');
  fs.mkdirSync(gdir, { recursive: true });
  await p.close();
  p = await b.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: scale });
  await p.goto('file://' + path.resolve(handoff, 'mockups/boot.html') + '#grub');
  await p.addStyleTag({ content: css });
  await p.evaluate(() => document.fonts.ready);
  await p.waitForTimeout(700);
  const g = 'grub';
  const G = `[data-pane="${g}"]`;
  const O = await (await p.$('.screen')).boundingBox();
  const items = await p.$$eval(G + ' .menu .item', (els, O) => els.map(e => { const r = e.getBoundingClientRect(); return { x: r.x - O.x, y: r.y - O.y, w: r.width, h: r.height, text: e.textContent.trim(), sel: e.classList.contains('sel') }; }), O);
  const mk = await p.$eval(G + ' .item.sel .mk', (e, O) => { const r = e.getBoundingClientRect(); const c = getComputedStyle(e); return { x: r.x - O.x, y: r.y - O.y, w: r.width, h: r.height, fontSize: c.fontSize }; }, O);
  const istyle = await p.$eval(G + ' .item', e => { const c = getComputedStyle(e); return { fontSize: c.fontSize, lineHeight: c.lineHeight }; });
  const hint = await p.$eval(G + ' .hintbar', (e, O) => { const r = e.getBoundingClientRect(); const c = getComputedStyle(e); return { x: r.x - O.x, y: r.y - O.y, w: r.width, h: r.height, fontSize: c.fontSize, text: e.textContent.trim() }; }, O);
  geo.grub = { pane: g, logo: await box(G + ' .logo svg'), menu: await box(G + ' .menu'), items, marker: mk, itemStyle: istyle, hint };
  await (await p.$(G + ' .logo svg')).screenshot({ path: path.join(gdir, `logo${suffix}.png`), omitBackground: true });

  await b.close();
  return geo;
}

(async () => {
  const geo = await run(1);
  await run(2);
  fs.writeFileSync(path.join(out, 'geometry.json'), JSON.stringify(geo, null, 2));
  console.log(JSON.stringify(geo, null, 2));
})();
