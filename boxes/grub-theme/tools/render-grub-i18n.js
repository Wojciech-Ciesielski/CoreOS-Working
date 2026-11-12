// GRUB w językach GNOME: z i18n/*.txt (box grub-theme) tworzy pliki, które GRUB czyta z nośnika / dysku.
//   lang/<tag>.cfg      set shrimp_fw="…" (nazwa wpisu „Ustawienia komputera”), set shrimp_img=0|1
//   lang/timeouts.tsv   tag<TAB>tekst odliczania (do theme-<tag>.txt, składa je edit_boot_config.sh)
//   icons/efi-<tag>.png nazwa wpisu jako obraz (Secure Boot nie wczytuje czcionek, a pismo spoza
//                       Inter, np. CJK czy arabskie, i tak nie ma w nich glifów)
// shrimp_img=1: język, którego nazwy wpisu Inter nie obejmuje, więc menu zawsze używa obrazów.
// Użycie: node boxes/grub-theme/tools/render-grub-i18n.js [katalog-motywu]   (potrzebne: playwright, fonty Noto, Inter z repo)
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const box = path.resolve(__dirname, '..');
const theme = process.argv[2] || path.join(box, 'root/usr/share/grub2/themes/shrimp');
const fonts = path.join(box, '../fonts/root/usr/share/fonts/truetype/inter');
const W = 424, H = 77, SIZE = 38;

const read = (f) => fs.readFileSync(f, 'utf8');
const english = {};
for (const l of read(path.join(box, 'i18n/_keys.tsv')).split('\n')) {
  if (!l || l.startsWith('#')) continue;
  const [id, , en] = l.split('\t');
  english[id] = en;
}
const tr = { en: { 1: english[1], 2: english[2] } };
for (const f of fs.readdirSync(path.join(box, 'i18n'))) {
  if (!f.endsWith('.txt')) continue;
  const m = {};
  for (const l of read(path.join(box, 'i18n', f)).split('\n')) {
    if (!l) continue;
    const [id, ...rest] = l.split('\t');
    m[id] = rest.join('\t');
  }
  tr[f.slice(0, -4)] = { ...tr.en, ...m };
}

const bcp = (t) => ({ 'zh-hans': 'zh-Hans', 'zh-hant': 'zh-Hant', 'sr-cyrl': 'sr-Cyrl', 'sr-latn': 'sr-Latn',
  'uz-cyrl': 'uz-Cyrl', 'be-latn': 'be-Latn', 'ca-valencia': 'ca' }[t] || t);
// Inter obejmuje łacinę, cyrylicę i grekę
const rtl = new Set(['ar', 'he', 'fa', 'ckb', 'ug', 'sd', 'ur', 'ps']);
const inter = /^[\p{Script=Latin}\p{Script=Cyrillic}\p{Script=Greek}\p{N}\p{P}\p{M}\p{Z}%]+$/u;

(async () => {
  fs.mkdirSync(path.join(theme, 'icons'), { recursive: true });
  fs.mkdirSync(path.join(theme, 'lang'), { recursive: true });
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
  const inter64 = fs.readFileSync(path.join(fonts, 'Inter-Regular.ttf')).toString('base64');
  const tsv = [];
  for (const [tag, t] of Object.entries(tr).sort()) {
    const fw = t[1], timeout = t[2];
    if (/["$\\`]/.test(fw) || /["$\\`]/.test(timeout)) throw new Error(tag + ': niedozwolony znak w tekście GRUB');
    const img = inter.test(fw) ? 0 : 1;
    fs.writeFileSync(path.join(theme, 'lang', tag + '.cfg'),
      `# generowane przez boxes/grub-theme/tools/render-grub-i18n.js z boxes/grub-theme/i18n/${tag}.txt\nset shrimp_fw="${fw}"\nset shrimp_img=${img}\n`);
    tsv.push(`${tag}\t${timeout}`);
    const esc = fw.replace(/&/g, '&amp;').replace(/</g, '&lt;');
    await p.setContent(`<!doctype html><html lang="${bcp(tag)}"><meta charset="utf-8"><style>
      @font-face { font-family: I; font-weight: 400; src: url(data:font/ttf;base64,${inter64}); }
      html, body { margin: 0; background: transparent; }
      div { width: ${W}px; height: ${H}px; display: flex; align-items: center;
            font: 400 ${SIZE}px/1 I, 'Noto Sans', 'Noto Sans Devanagari', 'Noto Sans Bengali', sans-serif; color: #8c8c8c; white-space: nowrap; }
      span { unicode-bidi: plaintext; padding: 0 ${rtl.has(tag) ? 8 : 0}px; }
    </style><div><span id="t">${esc}</span></div>`);
    await p.evaluate(() => document.fonts.load('38px I'));
    await p.evaluate(() => document.fonts.ready);
    const w = await p.evaluate(() => document.getElementById('t').getBoundingClientRect().width);
    if (w > W - 8) {
      const size = Math.max(22, Math.floor(SIZE * (W - 8) / w));
      await p.evaluate((s) => { document.querySelector('div').style.fontSize = s + 'px'; }, size);
      console.log(tag, 'zmniejszone do', size, 'px (szerokość', w.toFixed(0), ')');
    }
    await p.screenshot({ path: path.join(theme, 'icons', 'efi-' + tag + '.png'), omitBackground: true, clip: { x: 0, y: 0, width: W, height: H } });
  }
  fs.writeFileSync(path.join(theme, 'lang', 'timeouts.tsv'), tsv.join('\n') + '\n');
  await b.close();
  console.log('języków:', tsv.length);
})();
