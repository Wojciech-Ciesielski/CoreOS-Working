// Renders the GRUB menu entries as images for Secure Boot (lockdown).
// The signed GRUB cannot load our PF2 fonts there, so the entry text becomes
// icons/os.png (class os = "Shrimp"). Entry "Computer settings" in every language: boxes/grub-theme/tools/render-grub-i18n.js.
// Usage: node boxes/grub-theme/tools/render-grub-sb-icons.js <out-dir>   (needs playwright + the Inter TTFs in the repo)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const out = process.argv[2];
const fonts = path.resolve(__dirname, '../../fonts/root/usr/share/fonts/truetype/inter');
const W = 424, H = 77, SIZE = 38;
const items = [
  { name: 'os', text: 'Shrimp', weight: 600, color: '#ffffff', file: 'Inter-SemiBold.ttf' },
];

(async () => {
  fs.mkdirSync(path.join(out, 'icons'), { recursive: true });
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
  for (const it of items) {
    const html = `<style>
      @font-face { font-family: I; font-weight: ${it.weight}; src: url(data:font/ttf;base64,${fs.readFileSync(path.join(fonts, it.file)).toString('base64')}); }
      html, body { margin: 0; background: transparent; }
      div { width: ${W}px; height: ${H}px; display: flex; align-items: center;
            font: ${it.weight} ${SIZE}px/1 I; color: ${it.color}; white-space: nowrap; }
    </style><div id="t">${it.text}</div>`;
    await p.setContent(html);
    await p.evaluate(() => document.fonts.load('38px I'));
    await p.evaluate(() => document.fonts.ready);
    const w = await p.evaluate(() => { const r = document.createRange(); r.selectNodeContents(document.getElementById('t')); return r.getBoundingClientRect().width; });
    console.log(it.name, 'text width', w.toFixed(1), 'of', W);
    await p.screenshot({ path: path.join(out, 'icons', it.name + '.png'), omitBackground: true, clip: { x: 0, y: 0, width: W, height: H } });
  }
  await b.close();
})();
