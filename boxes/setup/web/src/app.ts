// Setup Shrimp: jedna ciągła prezentacja, instalacja (kroki 1–9) i pierwsza konfiguracja (10–15).
// Układ, teksty i zachowanie przeniesione z shrimp-handoff/mockups/setup.html; dane i akcje idą przez backendy.
import logoSrc from "./assets/logo.svg?raw";
import iconSprite from "./assets/icons.svg?raw";
import type { I18n } from "../../../i18n-core/core";
import {
  HELLO, ENCS, HSEC, IPS, DNSS, PROXY, THEMES, DRV, NV, ACCENTS, PLACES, USES, ERR, SHRIMP_MIN_GB,
  defaultKeymap, defaultTz, slides, type ErrKey, type Lang, type Region, type Keyboard,
} from "./data";
import { firstName, slug, loginFrom, isReservedUser, proposals, windowsList, tnow, esc, prefill, installLang, regionLabel, zoneName, keyboardName, type Prefill } from "./logic";
import { load } from "./i18n";
import { InstallError, OfflineError, type Backends, type Disk, type FirstBootInfo, type InstallPlan, type Network, type Place, type Progress, type Transfer, type UsbResult, type UsbState } from "./backend/types";

const $ = <T extends Element = HTMLElement>(s: string, r: ParentNode = document) => r.querySelector(s) as T | null;
const $$ = <T extends Element = HTMLElement>(s: string, r: ParentNode = document) => Array.from(r.querySelectorAll(s)) as T[];

const STEPS = ["witaj", "jezyk", "region", "klawiatura", "dysk", "konto", "internet", "podsumowanie", "instalacja", "restart",
  "czesc", "siec", "wyglad", "sterowniki", "programy", "pendrive", "gotowe"] as const;
type StepId = (typeof STEPS)[number];
const idx = (id: StepId) => STEPS.indexOf(id);
const THEME_STEP = idx("wyglad");
const FIRST_BOOT = idx("czesc");

/** Logo przycięte do ramki kształtu (DESIGN.md › Logo) i barwione kolorem tekstu. */
const LOGO = logoSrc.replace(/viewBox="[^"]*"/, 'viewBox="-1.1 3.6 184.6 178.5"').replace(/fill="#000"/g, 'fill="currentColor"');
const icon = (name: string, cls = "", label = "") =>
  `<svg class="i ${cls}" ${label ? `role="img" aria-label="${label}"` : 'aria-hidden="true"'}><use href="#${name}"/></svg>`;

export function start(root: HTMLElement, be: Backends, L0: I18n) {
  /** Język interfejsu; zmienia się od razu po wyborze języka w kroku „Wybierz język”. */
  let L = L0;
  const tr = (k: string, a?: Record<string, string | number>) => L.t(k, a);
  /** Klucz tłumaczenia albo zwykły tekst (WPA2, Cloudflare…). */
  const tx = (s: string) => (L.has(s) ? L.t(s) : s);
  const logo = (w: string) => `<div class="logo" style="width:${w}">${LOGO.replace("<svg", `<svg class="sh" role="img" aria-label="${esc(tr("logo.aria"))}"`)}</div>`;
  document.body.insertAdjacentHTML("afterbegin", `<div hidden>${iconSprite}</div>`);
  root.innerHTML = `
    <div class="screen" id="screen" data-look="boot">
      <div class="wall" aria-hidden="true"></div><div class="veil" aria-hidden="true"></div>
      <button class="back" id="back" type="button" hidden>${icon("go-previous-symbolic")}</button>
      <button class="infobtn" id="infoBtn" type="button" hidden>${icon("help-about-symbolic")}</button>
      <div class="pop" id="infoPop" role="tooltip"></div>
      <section class="pane" id="pane" aria-live="polite"></section>
      <div class="scrim" id="modal" hidden>
        <div class="dlg" role="alertdialog" aria-modal="true" aria-labelledby="dT" aria-describedby="dD">
          ${icon("dialog-warning-symbolic", "ic")}
          <h2 id="dT"></h2><p id="dD"></p>
          <div class="acts"><button class="btn" id="dP" type="button"></button><button class="skip" id="dS" type="button"></button></div>
        </div>
      </div>
    </div>`;
  const screen = $("#screen")!, pane = $("#pane")!, modal = $("#modal")!;
  // Zanim przyjdą dane z instalatora: czarny ekran z logo, tak jak ekran startu, tło sesji live
  // i osłona strażnika Setup. Potem spokojne przejście (0,7 s) do pierwszego ekranu.
  pane.className = "pane splash";
  pane.innerHTML = `<div class="logo splash-logo">${LOGO.replace("<svg", '<svg class="sh" aria-hidden="true"')}</div>`;
  /** Teksty poza widokami kroków (zależą od języka). */
  function chromeText() {
    document.documentElement.lang = L.lang;
    // Układ zostaje ten sam, a kierunek tekstu wynika z jego treści (style.css, data-rtl); lustrzane są tylko strzałki i narożniki.
    document.documentElement.toggleAttribute("data-rtl", L.rtl);
    $("#back")!.setAttribute("aria-label", tr("back.aria"));
    $("#infoBtn")!.setAttribute("aria-label", tr("info.aria"));
    $("#infoPop")!.textContent = tr("nvidia.pop");
  }
  chromeText();

  // --- dane z backendów ---
  let LANGS: Lang[] = [], REGS: Region[] = [], KBS: Keyboard[] = [], DISKS: Disk[] = [], NETS: Network[] = [];
  let INFO: FirstBootInfo = { fullName: "", beside: false, wired: false, nvidia: null, windowsApps: null };
  /** Dane z Shrimp Install i wartości, które z nich wynikają (język, strefa, klawiatura, konto). */
  let TR: Transfer | null = null, PRE: Prefill | null = null;
  /** Pendrive z Shrimp Install: jego zawartość czeka na przywrócenie (krok „pendrive”, tylko gdy jest co przywracać). */
  let USB: UsbState = { pending: false, present: false, bytes: 0, files: 0 }, USB_RES: UsbResult = "done";

  const S = {
    lang: 0, region: 0, kb: 0, place: 0, win: 0, disk: 0, enc: 0, dp: "", dp2: "", online: true,
    full: "", user: "", userEdited: false, pass: "", pass2: "", confirm: false,
    net: 0, netPass: "", hsec: 1, ssid: "", hpass: "", ip: 0, dns: 0, proxy: 0,
    theme: "light" as "light" | "dark", accent: null as number | null, drv: 0, nv: 0,
    use: [true, false, false, false, true], sel: {} as Record<string, boolean>, err: null as ErrKey | null,
  };
  let step = 0, sub = 0, boot = true, busy = false;
  /** Krok „dysk”: trwa świeży odczyt dysków z Agamy (diskSeq odrzuca odpowiedzi z wcześniejszych wejść). */
  let diskLoading = false, diskSeq = 0;
  /** Nośnik: jest internet (Agama pobiera system z repozytoriów openSUSE). Bez niego krok „Połącz z internetem”. */
  let ONLINE = true;
  const T: Record<string, number> = {};
  const cur = () => STEPS[step];
  const disk = () => DISKS[S.disk];
  const hasWin = () => !!disk()?.windows;
  /** Windows zaszyfrowany BitLockerem: tylko „Zamiast Windows”. */
  const locked = () => hasWin() && (!!disk()!.windows!.bitlocker || !!TR?.bitlocker);
  /** Minimum dla Shrimp: system + pliki przenoszone z Windows. */
  const shrimpMin = () => SHRIMP_MIN_GB + Math.ceil((TR?.bytes || 0) / 1e9);
  const beside = () => (be.mode === "install" ? hasWin() && S.place === 0 : INFO.beside);
  const pool = () => disk()?.windows?.poolGB ?? disk()?.sizeGB ?? 0;
  const winMin = () => disk()?.windows?.minGB ?? 0;
  const place = (): Place => (!hasWin() ? "whole" : S.place === 0 ? "beside" : "replace");
  const winApps = () => windowsList(INFO.windowsApps);
  const props = () => proposals(L, S.use, INFO.windowsApps);
  const hasNvidia = () => !!INFO.nvidia;

  // --- wygląd: jasny do kroku „Wygląd”, od niego wybrany ---
  function applyLook() { screen.dataset.look = boot ? "boot" : step >= THEME_STEP ? S.theme : "light"; }
  function applyAccent() {
    if (S.accent === null) { screen.style.removeProperty("--s-acc"); screen.style.removeProperty("--s-acc-on"); }
    else { screen.style.setProperty("--s-acc", ACCENTS[S.accent][2]); screen.style.setProperty("--s-acc-on", "#fff"); }
  }

  // --- klocki widoku ---
  const head = (h: string, s?: string) => `<h1 class="h1">${h}</h1>` + (s ? `<p class="sub">${s}</p>` : "");
  const body = (h: string) => `<div class="body">${h}</div>`;
  function acts(p: string, skip?: string, o: { hid?: boolean; act?: string; skipAct?: string } = {}) {
    return `<div class="acts"${o.hid ? " hidden" : ""}>` + (p ? `<button class="btn" data-act="${o.act || "next"}" type="button">${p}</button>` : "") +
      (skip ? `<button class="skip" data-act="${o.skipAct || "skip"}" type="button">${skip}</button>` : "") + "</div>";
  }
  type Item = string | { t: string; m?: string; off?: boolean };
  function lst(l: string, items: Item[], curI: number, o: { row?: boolean; scroll?: boolean } = {}) {
    return `<div class="list${o.row ? " row" : ""}${o.scroll ? " scroll" : ""}" data-l="${l}" role="radiogroup">` + items.map((t, j) => {
      const it = typeof t === "string" ? { t } : t;
      return `<button class="opt${j === curI ? " sel" : ""}" role="radio" aria-checked="${j === curI}" data-i="${j}" type="button"${it.off ? " disabled" : ""}><span class="t">${icon("go-next-symbolic", "mk")}${it.t}</span>` +
        (it.m ? ` <span class="meta">${it.m}</span>` : "") + "</button>";
    }).join("") + "</div>";
  }
  function tgl(l: string, items: string[], onFn: (j: number) => boolean) {
    return `<div class="list" data-l="${l}">` + items.map((t, j) => {
      const on = onFn(j);
      return `<button class="opt${on ? " on" : ""}" role="checkbox" aria-checked="${on}" data-t="${j}" type="button"><span class="t">${icon("object-select-symbolic", "mk")}${t}</span></button>`;
    }).join("") + "</div>";
  }
  const col = (label: string, l: string, items: Item[], c: number) => `<div class="col"><div class="cl">${label}</div>${lst(l, items, c)}</div>`;
  /** Pole tekstowe. Hasło ma przycisk z okiem: pokazuje wpisane znaki, żeby było widać, czy układ klawiatury
   *  i hasło się zgadzają (hasło Wi-Fi wpisuje się, zanim jest jakikolwiek inny sposób sprawdzenia). */
  const field = (id: string, label: string, val: string, type = "text", extra = "") =>
    `<div class="f" ${extra}><label for="${id}">${label}</label>` + (type === "password"
      ? `<div class="pw"><input id="${id}" type="password" value="${esc(val)}" autocomplete="off" spellcheck="false" autocapitalize="off">` +
        `<button class="eye" type="button" data-eye="${id}" aria-pressed="false" aria-label="${esc(tr("pass.show"))}" title="${esc(tr("pass.show"))}">${icon("view-reveal-symbolic")}</button></div>`
      : `<input id="${id}" type="${type}" value="${esc(val)}" autocomplete="off" spellcheck="false">`) + "</div>";

  /** Pozycja listy ze znacznikiem „(Zalecane)”. */
  const rec = (s: string) => `${esc(tx(s))} <em>${esc(tr("recommended"))}</em>`;
  const plain = (l: string[]) => l.map((s) => esc(tx(s)));
  const regLabel = (r: Region) => regionLabel(L, r);
  const kbLabel = (k: Keyboard) => keyboardName(L, k);

  function descFor() {
    const id = cur();
    if (id === "region") { const r = REGS[S.region]; return r ? zoneName(L, r.tz) + " · " + tnow(L, r.tz) : ""; }
    if (id === "dysk") {
      if (sub === 1) return S.enc === 1 ? tr("disk.enc.warn") : "";
      if (locked()) return tr(PLACES[1].d) + " " + tr("place.locked");
      return hasWin() ? (PLACES[S.place].d ? tr(PLACES[S.place].d) : "") : tr("disk.whole", { name: disk()?.name || "" }) + (DISKS.some((d) => d.windows) ? " " + tr("disk.whole.keep") : "");
    }
    if (id === "wyglad") return tr("look.note");
    if (id === "sterowniki") return S.nv === 0 ? tr("nv.best") : tr("nv.simple");
    return "";
  }

  // --- sieć (krok „Połącz z internetem” na nośniku i krok „Sieć” po restarcie) ---
  function netList() {
    const n = NETS[S.net];
    return lst("net", NETS.map((x) => ({ t: esc(x.ssid) + (x.secure ? " " + icon("network-wireless-encrypted-symbolic", "lk", esc(tr("net.secured"))) : ""), m: x.secure ? "" : esc(tr("net.open")) })), S.net, { scroll: true }) +
      (!n ? "" : n.secure ? `<div class="fields" style="margin-top:calc(1.8 * var(--u))">${field("inNet", esc(tr("net.pass", { ssid: n.ssid })), S.netPass, "password")}</div>`
        : `<p class="desc" id="desc" style="margin-top:calc(1.8 * var(--u))">${esc(tr("net.open.note"))}</p>`) +
      (cur() === "internet" ? `<p class="nstat" id="nstat" role="status" hidden></p>` : "");
  }
  function netHidden() {
    return logo("calc(6 * var(--u))") + head(esc(tr("net.hiddenlink")), esc(tr("net.hidden.sub"))) + body(`<div class="fields">` + field("inSsid", esc(tr("net.ssid")), S.ssid) +
      `<div class="f"><label>${esc(tr("net.security"))}</label>${lst("hsec", plain(HSEC), S.hsec, { row: true })}</div>` + (S.hsec ? field("inHPass", esc(tr("account.pass")), S.hpass, "password") : "") + "</div>") +
      acts(esc(tr("net.connect")), esc(tr("cancel")), { act: "hconnect", skipAct: "cancel" });
  }
  const isNet = () => cur() === "siec" || cur() === "internet";

  // --- widoki kroków ---
  function view(): string {
    const NEXT = tr("next"), SKIP = tr("skip");
    switch (cur()) {
      case "witaj": return logo("calc(13 * var(--u))") + `<h1 class="h1" id="hello">${esc(HELLO[0])}</h1><p class="sub">${esc(tr("welcome.sub"))}</p>` + acts(esc(tr("start")));
      case "internet": {
        if (sub === 1) return netHidden();
        return logo("calc(6 * var(--u))") + head(esc(tr("net.title")), esc(tr("inet.sub"))) + body(NETS.length ? netList() : `<p class="desc" id="desc" style="margin-top:0">${esc(tr("inet.none"))}</p><p class="nstat" id="ndiag" role="status">${esc(tr("inet.scanning"))}</p>`) +
          acts(NETS.length ? esc(NEXT) : "", esc(tr("inet.rescan")), { skipAct: "rescan" }) + `<div class="mini"><button class="mlink" data-act="hid" type="button">${esc(tr("net.hiddenlink"))}</button></div>`;
      }
      case "jezyk": return logo("calc(6 * var(--u))") + head(esc(tr("lang.title"))) + body(lst("lang", LANGS.map((l) => esc(l.name)), S.lang, { scroll: true })) + acts(esc(NEXT));
      case "region": return logo("calc(6 * var(--u))") + head(esc(tr("region.title")), esc(tr("region.sub"))) +
        body(lst("region", REGS.map((r) => esc(regLabel(r))), S.region, { scroll: true }) + `<p class="desc" id="desc">${esc(descFor())}</p>`) + acts(esc(NEXT));
      case "klawiatura": return logo("calc(6 * var(--u))") + head(esc(tr("kb.title"))) +
        body(lst("kb", KBS.map((k) => esc(kbLabel(k))), S.kb, { scroll: true }) + `<div class="fields" style="margin-top:calc(2.2 * var(--u))">${field("kbT", esc(tr("kb.test")), "")}</div>`) + acts(esc(NEXT));
      case "dysk": {
        if (diskLoading && sub === 0) return logo("calc(6 * var(--u))") + head(esc(tr("disk.where"))) + `<div class="pbar run slow" role="progressbar" aria-label="${esc(tr("disk.checking"))}"><i></i></div><div class="cap">${esc(tr("disk.checking"))}</div>`;
        if (sub === 1) return logo("calc(6 * var(--u))") + head(esc(tr("advanced"))) + body(`<div class="cols">` +
          col(esc(tr("disk.drive")), "disk", DISKS.map((d) => ({ t: esc(d.name), m: `${esc(L.unit(d.sizeGB, "gigabyte"))} · ${esc(d.windows ? "Windows" : tr("disk.empty"))}` })), S.disk) + col(esc(tr("disk.encrypt")), "enc", plain(ENCS), S.enc) + "</div>" +
          (S.enc === 1 ? `<div class="fields wide" style="margin-top:calc(2.4 * var(--u))"><div class="frow">${field("inDp", esc(tr("disk.pass")), S.dp, "password")}${field("inDp2", esc(tr("disk.pass2")), S.dp2, "password")}</div></div>` : "") +
          `<p class="desc" id="desc">${esc(descFor())}</p>`) + acts(esc(tr("done")), "", { act: "done" });
        if (!hasWin()) return logo("calc(6 * var(--u))") + head(esc(tr("disk.where"))) + body(`<p class="desc" id="desc" style="margin-top:0">${esc(descFor())}</p>`) + acts(esc(NEXT), esc(tr("advanced")), { skipAct: "adv" });
        return logo("calc(6 * var(--u))") + head(esc(tr("disk.where"))) + body(lst("place", PLACES.map((p, j) => ({ t: j === 0 ? rec(p.n) : esc(tr(p.n)), off: j === 0 && locked() })), S.place) +
          (S.place === 0
            ? `<div class="split"><div class="sp">Windows<b id="wGB"></b></div><div class="sp r">Shrimp<b id="sGB"></b></div><input class="rng" id="rng" type="range" min="0" max="${pool()}" step="1" value="${S.win}" aria-label="${esc(tr("disk.split.aria"))}"></div>`
            : `<p class="desc" id="desc">${esc(descFor())}</p>`)) + acts(esc(NEXT), esc(tr("advanced")), { skipAct: "adv" });
      }
      case "konto": return logo("calc(6 * var(--u))") + head(esc(tr("account.title")), esc(tr("account.sub"))) + body(`<div class="fields wide"><div class="frow">` +
        field("inFull", esc(tr("account.full")), S.full) + field("inUser", esc(tr("account.user")), S.user) + "</div>" + field("inPass", esc(tr("account.pass")), S.pass, "password") +
        field("inPass2", esc(tr("account.pass2")), S.pass2, "password", 'id="f2"' + (S.confirm ? "" : " hidden")) + "</div>") + acts(esc(NEXT));
      case "podsumowanie": {
        const rows: [string, string][] = [[tr("sum.lang"), esc(LANGS[S.lang]?.name)], [tr("sum.region"), esc(REGS[S.region] ? regLabel(REGS[S.region]) : "")], [tr("sum.kb"), esc(KBS[S.kb] ? kbLabel(KBS[S.kb]) : "")], [tr("sum.disk"), esc(disk()?.name || "")],
          [tr("sum.how"), esc(place() === "whole" ? tr("sum.whole") : tr(place() === "beside" ? "place.beside" : "place.replace"))]];
        if (beside()) rows.push([tr("sum.split"), esc(tr("sum.split.v", { win: L.unit(S.win, "gigabyte"), shrimp: L.unit(pool() - S.win, "gigabyte") }))]);
        rows.push([tr("sum.account"), `${esc(S.full)} (${esc(S.user)})`]);
        if (S.enc === 1) rows.push([tr("sum.enc"), esc(tr("enc.on"))]);
        return logo("calc(6 * var(--u))") + head(esc(tr("sum.title")), esc(tr("sum.sub"))) +
          body(`<div class="sum">${rows.map((r) => `<span>${esc(r[0])}</span><b>${r[1]}</b>`).join("")}</div>`) + acts(esc(tr("sum.go")));
      }
      case "instalacja": return logo("calc(9 * var(--u))") + head(esc(tr("inst.title"))) + `<div class="pbar" role="progressbar" aria-label="${esc(tr("inst.aria"))}"><i id="ib"></i></div><div class="cap"><b id="ph">${esc(tr("inst.disk"))}</b><span id="eta">${esc(tr("eta.left", { min: L.num(11, 0) }))}</span></div>` + acts(esc(NEXT), "", { hid: true });
      case "restart": return logo("calc(13 * var(--u))") + head(esc(tr("restart.title")), esc(tr("restart.sub"))) + acts(esc(tr("restart.go")));
      case "czesc": return logo("calc(13 * var(--u))") + head(esc(tr("hi.title", { name: firstName(be.mode === "install" ? S.full : INFO.fullName || S.full, tr("hi.fallback")) })), esc(tr("hi.sub"))) + acts(esc(NEXT));
      case "siec": {
        if (sub === 1) return netHidden();
        if (sub === 2) return logo("calc(6 * var(--u))") + head(esc(tr("advanced"))) + body(`<div class="cols">${col(esc(tr("net.ip")), "ip", IPS.map((s, j) => (j === 0 ? rec(s) : esc(tx(s)))), S.ip)}${col(esc(tr("net.dns")), "dns", DNSS.map((s, j) => (j === 0 ? rec(s) : esc(tx(s)))), S.dns)}${col(esc(tr("net.proxy")), "proxy", plain(PROXY), S.proxy)}</div>`) + acts(esc(tr("done")), "", { act: "done" });
        return logo("calc(6 * var(--u))") + head(esc(tr("net.title"))) + body(netList()) +
          acts(esc(NEXT), esc(SKIP)) + `<div class="mini"><button class="mlink" data-act="hid" type="button">${esc(tr("net.hiddenlink"))}</button><button class="mlink" data-act="nadv" type="button">${esc(tr("advanced"))}</button></div>`;
      }
      case "wyglad":
        if (sub === 1) return logo("calc(6 * var(--u))") + head(esc(tr("look.color"))) + body(`<div class="sws" role="radiogroup">` + ACCENTS.map((a, j) =>
          `<button class="sw" role="radio" aria-checked="${S.accent === j}" aria-label="${esc(tr(a[0]))}" data-c="${j}" style="--c:${a[2]}" type="button"></button>`).join("") +
          `</div><p class="accname" id="accn">${S.accent === null ? "" : esc(tr(ACCENTS[S.accent][0]))}</p>`) + acts(esc(NEXT));
        return logo("calc(6 * var(--u))") + head(esc(tr("look.title"))) + body(lst("theme", plain(THEMES), S.theme === "dark" ? 1 : 0) + `<p class="desc" id="desc">${esc(descFor())}</p>`) + acts(esc(NEXT));
      case "sterowniki":
        if (sub === 1) return logo("calc(6 * var(--u))") + head(esc(tr("nv.title")), esc(tr("nv.sub", { name: INFO.nvidia || "NVIDIA" }))) + body(lst("nv", NV.map((s, j) => (j === 0 ? rec(s) : esc(tx(s)))), S.nv) + `<p class="desc" id="desc">${esc(descFor())}</p>`) + acts(esc(NEXT));
        return logo("calc(6 * var(--u))") + head(esc(tr("drv.title")), esc(tr("drv.sub"))) + body(lst("drv", DRV.map((s, j) => (j === 0 ? rec(s) : esc(tx(s)))), S.drv)) + acts(esc(NEXT));
      case "programy": {
        if (sub === 1) {
          const W = winApps();
          return logo("calc(5 * var(--u))") + head(esc(tr("win.title")), esc(tr("win.sub"))) + body(tgl("win", W.map((p) => p.label), (j) => S.sel[W[j].key] !== false)) + acts(esc(NEXT), esc(SKIP));
        }
        if (sub === 2) {
          const P = props();
          return logo("calc(5 * var(--u))") + head(esc(tr("prop.title")), esc(tr("prop.sub"))) +
            body(P.length ? tgl("sel", P.map((p) => p.label), (j) => S.sel[P[j].key] !== false) : `<p class="desc">${esc(tr("prop.none"))}</p>`) + acts(P.length ? esc(tr("drv.install")) : "", esc(SKIP));
        }
        if (sub === 3) return logo("calc(9 * var(--u))") + head(esc(tr("apps.run"))) + `<div class="pbar" role="progressbar"><i id="ab"></i></div><div class="cap"><b id="aph">${esc(tr("apps.phase"))}</b><span id="aeta">${esc(tr("eta.left", { min: L.num(2, 0) }))}</span></div>` + acts(esc(NEXT), "", { hid: true });
        return logo("calc(5 * var(--u))") + head(esc(tr("apps.title")), esc(tr("apps.sub"))) + body(tgl("use", USES.map((s) => esc(tr(s))), (j) => S.use[j])) + acts(esc(NEXT), esc(SKIP));
      }
      case "pendrive": {
        if (sub === 1) return logo("calc(9 * var(--u))") + head(esc(tr("usb.run"))) + `<div class="pbar run" role="progressbar"><i></i></div>`;
        if (sub === 2) {
          const k = USB_RES === "done" ? "usb.done" : USB_RES === "home" ? "usb.home" : "usb.fail";
          return logo("calc(6 * var(--u))") + head(esc(tr(k + ".t")), esc(tr(k + ".d"))) + acts(esc(USB_RES === "failed" ? tr("usb.retry") : NEXT), "", { act: USB_RES === "failed" ? "usbcheck" : "next" });
        }
        const what = tr("usb.sub", { files: L.num(USB.files, 0), size: L.unit(Math.round(USB.bytes / 1e8) / 10, "gigabyte") });
        return USB.present
          ? logo("calc(6 * var(--u))") + head(esc(tr("usb.title")), esc(what)) + acts(esc(tr("usb.go")), esc(tr("usb.keep")), { act: "usbgo", skipAct: "usbhome" })
          : logo("calc(6 * var(--u))") + head(esc(tr("usb.plug.title")), esc(tr("usb.plug.sub"))) + acts(esc(tr("usb.check")), esc(tr("usb.keep")), { act: "usbcheck", skipAct: "usbhome" });
      }
      case "gotowe": {
        const SL = slides(beside());
        if (sub >= 1) {
          const s = SL[sub - 1];
          return logo("calc(6 * var(--u))") + head(esc(tr(s.t)), esc(tr(s.d) + (beside() && s.extra ? " " + tr(s.extra) : ""))) + `<p class="count">${esc(tr("slide.count", { n: L.num(sub, 0), total: L.num(SL.length, 0) }))}</p>` + acts(esc(sub === SL.length ? tr("start") : NEXT));
        }
        return logo("calc(13 * var(--u))") + head(esc(tr("ready.title")), esc(tr("ready.sub"))) + acts(esc(tr("start")), esc(tr("ready.more")), { skipAct: "more" });
      }
    }
  }

  const bigStep = () => { const id = cur(); return id === "witaj" || id === "restart" || id === "czesc" || (id === "gotowe" && sub === 0); };
  function clearTimers() { clearInterval(T.hello); clearTimeout(T.boot); scanSeq++; }
  /** Krok „Połącz z internetem” bez żadnej sieci na liście: Setup sam szuka dalej (serwer czeka na skan do 12 s)
   *  i mówi, czego brakuje: karty Wi-Fi (sterownik, firmware), włączonego radia albo po prostu zasięgu. */
  let scanSeq = 0;
  async function scanLoop() {
    const my = ++scanSeq, alive = () => my === scanSeq && cur() === "internet" && sub === 0;
    void be.firstboot.wifiDiag().then((d) => {
      console.log("shrimp-setup: Wi-Fi, brak sieci:", JSON.stringify(d));
      const e = $("#ndiag", pane); if (!e || !alive() || NETS.length) return;
      e.textContent = !d.devices.length ? tr("inet.nodev") + (d.hardware ? " (" + d.hardware.slice(0, 120) + ")" : "")
        : d.blocked || d.radio === "disabled" ? tr("inet.off") : tr("inet.scanning");
    }).catch(() => {});
    while (alive() && !NETS.length) {
      const n = await be.firstboot.networks().catch(() => [] as Network[]);
      if (!alive()) return;
      if (n.length) { NETS = n; S.net = 0; render(false); return; }
      await new Promise((r) => setTimeout(r, 1500));
    }
  }
  function rise() { $$("#pane > *").forEach((el, i) => { el.style.setProperty("--i", String(i)); el.classList.add("rise"); }); }
  function render(anim: boolean) {
    pane.className = "pane" + (bigStep() ? " big" : "");
    pane.innerHTML = view();
    if (anim) rise();
    post();
  }
  function post() {
    const id = cur();
    if (id === "witaj") {
      let k = 0; const h = $("#hello")!;
      T.hello = window.setInterval(() => { h.classList.add("fade"); setTimeout(() => { k = (k + 1) % HELLO.length; h.textContent = HELLO[k]; h.classList.remove("fade"); }, 380); }, 2300);
    }
    if (id === "region") T.hello = window.setInterval(() => { const d = $("#desc"); if (d) d.textContent = descFor(); }, 15000);
    if (id === "instalacja") runInstall();
    if (id === "konto") syncAccount();
    if (id === "internet" && sub === 0 && !NETS.length) void scanLoop();
    if (id === "dysk" && sub === 0) updSplit();
    if (id === "dysk" && sub === 1) syncDisk();
    if (id === "programy" && sub === 3) runApps();
    if (id === "siec" || id === "internet") syncNet();
    $$<HTMLElement>(".list.scroll", pane).forEach(centerSel);
  }
  function centerSel(box: HTMLElement) { const o = $<HTMLElement>(".opt.sel", box); if (o) box.scrollTop = o.offsetTop - box.clientHeight / 2 + o.offsetHeight / 2; }
  function syncDisk() { const b = $<HTMLButtonElement>(".btn", pane); if (b) b.disabled = S.enc === 1 && (!S.dp || !S.dp2); }
  function chrome() {
    const id = cur();
    const first = be.mode === "firstboot" ? FIRST_BOOT : 0;
    $("#back")!.hidden = step <= first || ["witaj", "instalacja", "restart", "czesc"].includes(id) || (id === "programy" && sub === 3) || (id === "pendrive" && sub > 0);
    const nv = id === "sterowniki" && sub === 1;
    $("#infoBtn")!.hidden = !nv; $("#infoPop")!.hidden = !nv; $("#infoPop")!.classList.remove("show");
  }
  function go(n: number, s = 0, keep = false) {
    clearTimers(); closeErr(); boot = false;
    const lo = be.mode === "firstboot" ? FIRST_BOOT : 0, hi = be.mode === "install" ? FIRST_BOOT - 1 : STEPS.length - 1;
    const prev = step;
    step = Math.max(lo, Math.min(hi, n)); sub = s;
    // Internet po kablu: krok Sieć jest pomijany w obie strony.
    if (cur() === "siec" && INFO.wired && be.mode !== "install") step += n >= prev ? 1 : -1;
    // Nośnik z internetem (kabel albo Wi-Fi już połączone): krok „Połącz z internetem” jest pomijany.
    if (cur() === "internet" && (ONLINE || be.mode === "firstboot") && s === 0) step += n >= prev ? 1 : -1;
    // Lista sieci z chwili startu bywa nieaktualna, gdy użytkownik dochodzi do tego kroku po kilku minutach.
    if (cur() === "internet" && s === 0 && be.mode === "install") void be.firstboot.networks().then((n) => {
      const changed = n.map((x) => x.ssid).join("\n") !== NETS.map((x) => x.ssid).join("\n");
      if (!n.length || !changed) return;
      const was = NETS[S.net]?.ssid; NETS = n; S.net = Math.max(0, n.findIndex((x) => x.ssid === was));
      if (cur() === "internet" && sub === 0) render(false);
    }).catch(() => {});
    // Bez pendrive'a do przywrócenia krok „pendrive” jest pomijany w obie strony.
    if (cur() === "pendrive" && !USB.pending && s === 0) step += n >= prev ? 1 : -1;
    if (cur() === "konto" && !keep) S.confirm = false;
    // Dyski czytamy od nowa przy wejściu w krok: pierwszy odczyt (przy starcie) bywa niepełny, bo Agama
    // jeszcze sonduje dyski (dysk bez partycji = bez Windows = bez wyboru „Obok Windows”). Do końca odczytu
    // krok pokazuje „Sprawdzam dyski…”, a nie stary widok, który potem nagle się podmieniał (zgłoszenie z Vivobooka).
    if (cur() === "dysk" && be.mode !== "firstboot" && prev !== step) {
      const seq = ++diskSeq; diskLoading = true;
      void be.install.disks().catch(() => [] as Disk[]).then((d) => {
        if (seq !== diskSeq) return;
        diskLoading = false;
        if (d.length && JSON.stringify(d) !== JSON.stringify(DISKS)) {
          const had = DISKS[S.disk]?.windows;
          DISKS = d; S.disk = Math.min(S.disk, d.length - 1);
          if (!had && disk()?.windows) { S.place = 0; defaultSplit(); }
        }
        if (cur() === "dysk") render(true);
      });
    }
    chrome(); applyLook(); render(true);
  }

  // --- instalowanie ---
  function plan(): InstallPlan {
    return {
      locale: LANGS[S.lang].locale, timezone: REGS[S.region].tz, keymap: KBS[S.kb].keymap,
      disk: disk(), place: place(), windowsGB: beside() ? S.win : undefined,
      diskPassword: S.enc === 1 ? S.dp : undefined,
      fullName: S.full.trim(), userName: S.user, password: S.pass,
    };
  }
  function setBar(bar: string, ph: string, eta: string, p: { percent: number; phase: string; etaMin?: number }) {
    const b = $(bar); if (b) b.style.width = p.percent + "%";
    const f = $(ph); if (f) f.textContent = tx(p.phase);
    const e = $(eta); if (e) e.textContent = p.percent >= 100 ? "" : !p.etaMin ? "" : p.etaMin <= 1 ? tr("eta.soon") : tr("eta.left", { min: L.num(p.etaMin, 0) });
  }
  async function runInstall(watchOnly = false) {
    const my = step;
    const show = (p: Progress) => { if (step === my) setBar("#ib", "#ph", "#eta", p); };
    try {
      await (watchOnly ? be.install.watch(show) : be.install.install(plan(), show));
      if (step === my) showActs();
    } catch (e) {
      if (step === my) openErr(e instanceof InstallError ? e.key : "install", String((e as Error)?.message || e));
    }
  }
  async function runApps() {
    const my = step, mySub = sub;
    const ids = [...winApps(), ...props()].filter((p) => S.sel[p.key] !== false).map((p) => p.id);
    const uniq = [...new Set(ids)];
    try {
      if (!S.online) throw new OfflineError();
      await be.firstboot.installApps(uniq, (p) => { if (step === my && sub === mySub) setBar("#ab", "#aph", "#aeta", p); });
      if (step === my && sub === mySub) showActs();
    } catch {
      if (step === my && sub === mySub) openErr("net");
    }
  }
  async function runUsb(action: "restore" | "home") {
    const my = step;
    go(step, 1);
    try { USB_RES = await be.firstboot.restoreUsb(action); } catch { USB_RES = "failed"; }
    USB = { ...USB, pending: USB_RES === "failed" };
    if (step === my) go(step, 2);
  }
  function showActs() { const a = $(".acts", pane)!; a.hidden = false; a.classList.remove("rise"); void a.offsetWidth; a.style.setProperty("--i", "0"); a.classList.add("rise"); }
  function restart() {
    clearTimers(); boot = true; applyLook(); $("#back")!.hidden = true; $("#infoBtn")!.hidden = true; $("#infoPop")!.hidden = true;
    pane.className = "pane";
    pane.innerHTML = logo("calc(14 * var(--u))") + '<div class="pbar run"><i></i></div>'; rise();
    if (be.mode === "install") { be.install.reboot().catch(() => openErr("install")); return; }
    // Podgląd: czarny ekran startu (Plymouth), potem część 2.
    T.boot = window.setTimeout(() => go(FIRST_BOOT), 3600);
  }

  // --- formularze ---
  function syncAccount() { const b = $<HTMLButtonElement>(".btn", pane); if (b) b.disabled = !S.full.trim() || !S.user || !S.pass || (S.confirm && !S.pass2); }
  function syncNet() {
    const b = $<HTMLButtonElement>(".btn", pane); if (!b) return;
    if (sub === 0) b.disabled = !NETS[S.net] || (NETS[S.net].secure && !S.netPass);
    if (sub === 1) b.disabled = !S.ssid.trim() || (S.hsec > 0 && !S.hpass);
  }
  function updSplit() {
    const r = $<HTMLInputElement>("#rng"); if (!r) return;
    const t = pool(), w = Math.max(winMin(), Math.min(t - shrimpMin(), +r.value)); r.value = String(w); S.win = w;
    r.style.setProperty("--v", String(w / t));
    $("#wGB")!.textContent = L.unit(w, "gigabyte"); $("#sGB")!.textContent = L.unit(t - w, "gigabyte");
  }
  pane.addEventListener("input", (e) => {
    const t = e.target as HTMLInputElement, id = t.id;
    if (id === "rng") { updSplit(); return; }
    if (id === "inFull") { S.full = t.value; if (!S.userEdited) { S.user = loginFrom(S.full); $<HTMLInputElement>("#inUser")!.value = S.user; } syncAccount(); }
    else if (id === "inUser") { S.user = t.value = slug(t.value); S.userEdited = true; syncAccount(); }
    else if (id === "inPass") { S.pass = t.value; syncAccount(); }
    else if (id === "inPass2") { S.pass2 = t.value; syncAccount(); }
    else if (id === "inDp") { S.dp = t.value; syncDisk(); }
    else if (id === "inDp2") { S.dp2 = t.value; syncDisk(); }
    else if (id === "inNet") { S.netPass = t.value; syncNet(); }
    else if (id === "inSsid") { S.ssid = t.value; syncNet(); }
    else if (id === "inHPass") { S.hpass = t.value; syncNet(); }
  });

  // --- wybór ---
  const SINGLE: Record<string, 1> = { lang: 1, region: 1, kb: 1, place: 1, disk: 1, enc: 1, net: 1, hsec: 1, ip: 1, dns: 1, proxy: 1, drv: 1, nv: 1, theme: 1 };
  const RELAYOUT: Record<string, 1> = { place: 1, net: 1, hsec: 1, disk: 1, enc: 1 };
  /** Interfejs przechodzi na wybrany język, gdy wybór się zatrzyma (przewijanie listy klawiszem albo
   *  touchpadem nie ładuje po drodze każdego języka i nie przebudowuje listy w trakcie ruchu). */
  let langTimer = 0, kbTimer = 0;
  function langSwitch(i: number) {
    clearTimeout(langTimer);
    langTimer = window.setTimeout(async () => {
      const next = await load([LANGS[i].locale]);
      if (S.lang !== i) return;
      L = next; chromeText();
      if (cur() !== "jezyk") return;
      // Na liście są nazwy języków w ich własnym języku: zmieniają się tylko tytuł i przycisk.
      const h = $(".h1", pane); if (h) h.textContent = tr("lang.title");
      const b = $(".acts .btn", pane); if (b) b.textContent = tr("next");
    }, 250);
  }
  /** Zaznaczona pozycja listy przewijanej na środku. Klawisze i kółko/touchpad: od razu, bez animacji,
   *  żeby strzałka zaznaczenia nie zostawała w tyle; kliknięcie: płynnie, tylko gdy pozycja jest przy krawędzi. */
  function keepSelVisible(box: HTMLElement, how: "click" | "key") {
    const so = $<HTMLElement>(".opt.sel", box); if (!so) return;
    const top = so.offsetTop, bot = top + so.offsetHeight, mid = top - box.clientHeight / 2 + so.offsetHeight / 2;
    if (how === "key") box.scrollTop = mid;
    else if (top < box.scrollTop + box.clientHeight * 0.25 || bot > box.scrollTop + box.clientHeight * 0.75) box.scrollTo({ top: mid, behavior: "smooth" });
  }
  function pick(l: string, i: number, how: "click" | "key" = "click") {
    if (l === "theme") { S.theme = i === 1 ? "dark" : "light"; applyLook(); } else (S as unknown as Record<string, number>)[l] = i;
    if (l === "disk") defaultSplit();
    if (l === "lang") langSwitch(i);
    if (l === "kb") { clearTimeout(kbTimer); kbTimer = window.setTimeout(() => { be.install.setKeymap(KBS[i].keymap).catch(() => {}); be.firstboot.setInputSource(KBS[i].keymap).catch(() => {}); }, 250); }
    if (RELAYOUT[l]) { render(false); return; }
    const box = $<HTMLElement>(`.list[data-l="${l}"]`, pane)!;
    $$(".opt", box).forEach((o, k) => { o.classList.toggle("sel", k === i); o.setAttribute("aria-checked", String(k === i)); });
    if (box.classList.contains("scroll")) keepSelVisible(box, how);
    const d = $("#desc"); if (d) d.textContent = descFor();
  }
  /** Kółko myszy i touchpad na liście przewijanej (język, kraj, klawiatura) przesuwają zaznaczenie
   *  o jedną pozycję na „ząbek” albo na wysokość pozycji przy touchpadzie, zamiast przewijać listę
   *  bez zaznaczenia. Wcześniej WebKitGTK przewijał kilka pozycji naraz, a strzałka zostawała na miejscu. */
  let wheelAcc = 0, wheelAt = 0;
  pane.addEventListener("wheel", (e) => {
    const box = (e.target as HTMLElement).closest<HTMLElement>(".list.scroll[data-l]");
    if (!box || boot || !modal.hidden) return;
    e.preventDefault();
    const opts = $$<HTMLButtonElement>(".opt", box), c = opts.findIndex((o) => o.classList.contains("sel"));
    const h = opts[0]?.offsetHeight || 30;
    if (e.timeStamp - wheelAt > 400) wheelAcc = 0;
    wheelAt = e.timeStamp;
    // Mysz: duży skok na „ząbek” (linie albo ≥ 50 px) = jedna pozycja. Touchpad: małe kroki sumowane,
    // jedna pozycja na 1,5 wysokości pozycji (wolniej niż zwykłe przewijanie, które było za szybkie).
    const step = h * 1.5;
    if (e.deltaMode !== 0 || Math.abs(e.deltaY) >= 50) wheelAcc = Math.sign(e.deltaY) * step;
    else wheelAcc += e.deltaY;
    const n = Math.trunc(wheelAcc / step);
    if (!n) return;
    wheelAcc -= n * step;
    let k = Math.max(0, Math.min(opts.length - 1, (c < 0 ? 0 : c) + n));
    while (opts[k]?.disabled && k > 0 && k < opts.length - 1) k += Math.sign(n);
    if (k !== c && opts[k] && !opts[k].disabled) pick(box.getAttribute("data-l")!, k, "key");
  }, { passive: false });
  function toggle(l: string, i: number) {
    const o = $(`.list[data-l="${l}"] .opt[data-t="${i}"]`, pane)!;
    let on: boolean;
    if (l === "sel" || l === "win") { const P = l === "win" ? winApps() : props(); S.sel[P[i].key] = S.sel[P[i].key] === false; on = S.sel[P[i].key] !== false; }
    else { S.use[i] = !S.use[i]; on = S.use[i]; }
    o.classList.toggle("on", on); o.setAttribute("aria-checked", String(on));
  }
  pane.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    const eye = t.closest<HTMLButtonElement>("[data-eye]");
    if (eye) {
      const inp = $<HTMLInputElement>("#" + eye.dataset.eye, pane); if (!inp) return;
      const show = inp.type === "password"; inp.type = show ? "text" : "password";
      const lbl = tr(show ? "pass.hide" : "pass.show");
      eye.setAttribute("aria-pressed", String(show)); eye.setAttribute("aria-label", lbl); eye.title = lbl;
      eye.innerHTML = icon(show ? "view-conceal-symbolic" : "view-reveal-symbolic");
      inp.focus(); inp.setSelectionRange(inp.value.length, inp.value.length);
      return;
    }
    const o = t.closest(".opt");
    if (o && (o as HTMLButtonElement).disabled) return;
    if (o) { const l = o.closest(".list")!.getAttribute("data-l")!; if (o.hasAttribute("data-t")) toggle(l, +o.getAttribute("data-t")!); else pick(l, +o.getAttribute("data-i")!); return; }
    const sw = t.closest(".sw");
    if (sw) {
      S.accent = +sw.getAttribute("data-c")!; applyAccent();
      $$(".sw", pane).forEach((b) => b.setAttribute("aria-checked", String(b === sw)));
      $("#accn")!.textContent = ACCENTS[S.accent][0]; return;
    }
    const a = t.closest("[data-act]"); if (a) act(a.getAttribute("data-act")!);
  });

  function defaultSplit() {
    const d = disk(); if (!d?.windows) return;
    // Domyślnie połowa wolnego ponad minimum Windows, ale Shrimp co najmniej shrimpMin().
    const w = Math.round(Math.max(d.windows.minGB, Math.min(d.windows.poolGB - shrimpMin(), d.windows.minGB + (d.windows.poolGB - d.windows.minGB) / 2)));
    S.win = w;
    if (locked()) S.place = 1;
  }

  // --- przejścia ---
  async function guarded(fn: () => Promise<void>) {
    if (busy) return; busy = true;
    const b = $<HTMLButtonElement>(".btn", pane); if (b) b.disabled = true;
    try { await fn(); } finally { busy = false; const b2 = $<HTMLButtonElement>(".btn", pane); if (b2 && b2 === b) b2.disabled = false; }
  }
  function act(a: string) {
    const id = cur();
    if (a === "adv" || a === "hid") return go(step, 1);
    if (a === "nadv") return go(step, 2);
    if (a === "done" && id === "dysk" && S.enc === 1 && S.dp !== S.dp2) return openErr("dpass");
    if (a === "done" && id === "siec") {
      return guarded(async () => {
        await be.firstboot.setNetworkAdvanced({ ip: S.ip ? "manual" : "auto", dns: (["auto", "cloudflare", "google"] as const)[S.dns], proxy: S.proxy ? "auto" : "none" }).catch(() => {});
        go(step, 0);
      });
    }
    if (a === "done" || a === "cancel") return go(step, 0);
    if (a === "more") return go(step, 1);
    if (a === "usbgo") return void runUsb("restore");
    if (a === "usbhome") return void runUsb("home");
    if (a === "usbcheck") return void be.firstboot.usb().then((u) => { USB = u; if (step === idx("pendrive")) go(step, 0); });
    if (a === "skip") {
      if (id === "siec") S.online = false;
      if (id === "programy" && sub === 1) return go(step, 2);
      if (id === "programy" && sub === 0) return go(step + 1);
      return go(step + 1);
    }
    if (a === "rescan") return guarded(async () => {
      if (await afterConnect(1)) { const was = NETS[S.net]?.ssid; NETS = await be.firstboot.networks().catch(() => []); S.net = Math.max(0, NETS.findIndex((x) => x.ssid === was)); render(false); }
    });
    if (a === "hconnect") {
      return guarded(async () => {
        try { await be.firstboot.connectHidden(S.ssid.trim(), (["none", "wpa2", "wpa3"] as const)[S.hsec], S.hsec ? S.hpass : undefined); S.online = true; }
        catch (e) { console.log("shrimp-setup: Wi-Fi (ukryta sieć) nie połączone:", String((e as Error)?.message || e)); return openErr("wifi", String((e as Error)?.message || "")); }
        if (id === "internet") { const r = await afterConnect(30, S.ssid.trim()); if (r) openErr(r.k, r.why); return; }
        go(step + 1);
      });
    }
    // „next”
    if (id === "jezyk") {
      const l = LANGS[S.lang];
      be.install.setLanguage(l.locale).catch(() => {});
      const lc = l.locale.split(".")[0];
      // Język z Windows: strefa i klawiatura też z Windows. Inny język: domyślne dla niego.
      const same = PRE && PRE.lang === S.lang;
      const tz = same && PRE!.region >= 0 ? PRE!.region : REGS.findIndex((r) => r.tz === defaultTz(lc)); if (tz >= 0) S.region = tz;
      const kb = same && PRE!.kb >= 0 ? PRE!.kb : KBS.findIndex((k) => k.keymap === defaultKeymap(lc)); if (kb >= 0) S.kb = kb;
    }
    if (id === "klawiatura" && KBS[S.kb]) be.firstboot.setInputSource(KBS[S.kb].keymap).catch(() => {});
    if (id === "konto") {
      if (!S.confirm) { S.confirm = true; const f2 = $("#f2")!; f2.hidden = false; f2.classList.add("rise"); $<HTMLInputElement>("#inPass2")!.focus(); syncAccount(); return; }
      if (S.pass !== S.pass2) return openErr("pass");
      if (isReservedUser(S.user)) return openErr("user");
      return go(step + 1);
    }
    if (id === "dysk" && sub === 0 && !disk()) return openErr("nodisk");
    if (id === "dysk" && sub === 0 && (beside() ? pool() - S.win : disk()?.sizeGB ?? 0) < shrimpMin()) return openErr("space");
    if (id === "podsumowanie") {
      return guarded(async () => {
        try { await be.install.check(plan()); go(step + 1); }
        catch (e) { openErr(e instanceof InstallError ? e.key : "install", String((e as Error)?.message || e)); }
      });
    }
    if (isNet() && sub === 0) {
      const n = NETS[S.net]; if (!n) return id === "internet" ? undefined : go(step + 1);
      return guarded(async () => {
        if (id === "internet") netStatus(tr("inet.connecting", { ssid: n.ssid }));
        try { await be.firstboot.connect(n.ssid, n.secure ? S.netPass : undefined); S.online = true; }
        catch (e) { netStatus(""); console.log("shrimp-setup: Wi-Fi nie połączone:", String((e as Error)?.message || e)); return openErr("wifi", String((e as Error)?.message || "")); }
        if (id === "internet") { const r = await afterConnect(30, n.ssid); if (r) openErr(r.k, r.why); return; }
        go(step + 1);
      });
    }
    if (id === "wyglad") {
      be.firstboot.setAppearance(S.theme === "dark" ? "prefer-dark" : "default", S.accent === null ? null : ACCENTS[S.accent][1]).catch(() => {});
      if (sub === 0) return go(step, 1);
    }
    if (id === "sterowniki") {
      if (sub === 0 && S.drv === 0 && hasNvidia()) return go(step, 1);
      be.firstboot.setDrivers({ install: S.drv === 0, nvidia: S.drv === 0 && hasNvidia() ? (S.nv === 0 ? "nvidia" : "open") : undefined }).catch(() => {});
    }
    if (id === "programy") {
      if (sub === 0) return go(step, winApps().length ? 1 : 2);
      if (sub === 1) return go(step, 2);
      if (sub === 2) {
        const any = [...winApps(), ...props()].some((p) => S.sel[p.key] !== false);
        return any ? go(step, 3) : go(step + 1);
      }
    }
    if (id === "restart") return restart();
    if (id === "gotowe") {
      const n = slides(beside()).length;
      if (sub === 0 || sub === n) return finish();
      return go(step, sub + 1);
    }
    go(step + 1);
  }
  /** Nośnik po połączeniu z siecią: czeka na internet (do 60 s), Agama czyta repozytoria od nowa, dalej.
   *  Zwraca "" (dalej) albo przyczynę do okna „Brak połączenia z internetem”: osobno brak internetu
   *  po połączeniu z Wi-Fi i osobno repozytoria, których Agama nie czyta mimo internetu. */
  async function afterConnect(tries = 30, ssid = ""): Promise<"" | { k: ErrKey; why: string }> {
    netStatus(ssid ? tr("inet.check", { ssid }) : "");
    for (let i = 0; i < tries && !(ONLINE = await be.firstboot.online().catch(() => false)); i++) await new Promise((r) => setTimeout(r, 2000));
    console.log(`shrimp-setup: internet po połączeniu z Wi-Fi: ${ONLINE ? "jest" : "brak"}`);
    if (!ONLINE) { netStatus(""); return { k: "offline", why: tr("err.offline.nonet") }; }
    netStatus(tr("inet.repo"));
    let issue: string;
    try { issue = await be.install.reprobe(); } catch (e) { issue = String((e as Error)?.message || e); }
    netStatus("");
    if (issue) { console.log("shrimp-setup: repozytoria po połączeniu:", issue); return { k: "repo", why: issue.slice(0, 160) }; }
    go(idx("internet") + 1);
    return "";
  }
  /** Napis pod polem hasła w kroku „Połącz z internetem”: co Setup teraz robi (łączenie bywa długie). */
  function netStatus(t: string) { const e = $("#nstat", pane); if (e) { e.textContent = t; e.hidden = !t; } }
  function finish() {
    if (be.mode === "demo") { S.theme = "light"; S.accent = null; S.confirm = false; applyAccent(); go(0); return; }
    guarded(() => be.firstboot.finish());
  }
  $("#back")!.addEventListener("click", () => {
    const id = cur();
    if (sub > 0) {
      if (id === "programy" && sub === 2 && !winApps().length) return go(step, 0, true);
      return go(step, sub - 1, true);
    }
    // Wstecz na krok z podkrokami wraca do jego ostatniego podkroku, tak jak w makiecie wraca do początku kroku.
    go(step - 1);
  });
  $("#infoBtn")!.addEventListener("click", () => $("#infoPop")!.classList.toggle("show"));

  // --- okna błędów ---
  function openErr(k: ErrKey, detail = "") {
    const e = ERR[k]; S.err = k;
    $("#dT")!.textContent = tr(e.t);
    $("#dD")!.textContent = k === "wifi" && sub === 0 && NETS[S.net] ? tr("err.wifi.ssid", { ssid: NETS[S.net].ssid }) : tr(e.d);
    // Przyczyna zgłoszona przez Agamę (krótko): bez niej nie wiadomo, co poszło nie tak.
    // „Brak internetu” z kroku Wi-Fi: zamiast ogólnego zdania konkretna przyczyna (afterConnect).
    if (detail && k === "offline") $("#dD")!.textContent = detail;
    else if (detail && detail !== k) $("#dD")!.textContent += ` (${detail.slice(0, 220)})`;
    $("#dP")!.textContent = tr(e.p); const skip = e.s && !(k === "wifi" && cur() === "internet") ? e.s : "";
    $("#dS")!.textContent = skip ? tr(skip) : ""; $("#dS")!.hidden = !skip;
    modal.hidden = false; $("#dP")!.focus();
  }
  function closeErr() { modal.hidden = true; S.err = null; }
  $("#dP")!.addEventListener("click", () => {
    const k = S.err; closeErr();
    if (k === "pass") { S.pass2 = ""; render(false); $<HTMLInputElement>("#inPass2")!.focus(); }
    else if (k === "dpass") { S.dp2 = ""; render(false); $<HTMLInputElement>("#inDp2")!.focus(); syncDisk(); }
    else if (k === "space") {
      if (cur() !== "dysk") return go(idx("dysk"));
      const r = $<HTMLInputElement>("#rng"); if (r) { defaultSplit(); r.value = String(S.win); updSplit(); r.focus(); }
    }
    else if (k === "nodisk") {
      void be.install.disks().then((d) => { DISKS = d; S.disk = 0; defaultSplit(); go(idx("dysk")); }).catch(() => openErr("nodisk"));
    }
    else if (k === "install") go(step);
    else if (k === "offline" || k === "repo") { ONLINE = false; if (cur() === "internet") render(false); else go(idx("internet")); }
    else if (k === "stall" || k === "agama") runInstall(true);
    else if (k === "user") { S.userEdited = true; render(false); const u = $<HTMLInputElement>("#inUser"); if (u) { u.focus(); u.select(); } }
    else if (k === "wifi") { if (sub === 0) S.netPass = ""; else S.hpass = ""; render(false); $<HTMLInputElement>(sub === 0 ? "#inNet" : "#inHPass")?.focus(); }
    else if (k === "net") { S.online = true; go(step, 3); }
  });
  $("#dS")!.addEventListener("click", () => {
    const k = S.err; closeErr();
    if (k === "install" || k === "stall" || k === "agama") go(0);
    else if (k === "space") go(idx("dysk"));
    else if (k === "wifi") { if (cur() === "internet") return; S.online = false; go(step + 1); }
    else if (k === "net") go(step + 1);
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { if (!modal.hidden) { closeErr(); return; } $("#infoPop")!.classList.remove("show"); }
    if (boot || !modal.hidden) return;
    const box = $(".list[data-l]", pane), l = box && box.getAttribute("data-l");
    const target = e.target as HTMLElement;
    if (l && SINGLE[l] && (e.key === "ArrowDown" || e.key === "ArrowUp") && !target.closest("input")) {
      e.preventDefault();
      const opts = $$<HTMLButtonElement>(".opt", box!), c = opts.findIndex((o) => o.classList.contains("sel"));
      const step1 = e.key === "ArrowDown" ? 1 : opts.length - 1;
      let n = (c + step1) % opts.length;
      while (opts[n].disabled && n !== c) n = (n + step1) % opts.length;
      pick(l, n, "key");
    } else if (e.key === "Enter" && !target.closest("button")) {
      const p = $<HTMLButtonElement>(".btn:not([disabled])", pane); if (p && !p.closest("[hidden]")) { e.preventDefault(); p.click(); }
    }
  });

  // --- start ---
  (async () => {
    const q = new URLSearchParams(location.search);
    if (be.mode !== "firstboot") {
      // Dyski czytamy w tle: Agama sonduje je nawet kilkadziesiąt sekund, a pierwszy ekran nie musi na to czekać
      // (wcześniej przez ten czas był jasny pusty ekran). Krok „dysk” i tak czyta je od nowa przy wejściu.
      const disksLoad = be.install.disks().catch(() => [] as Disk[]);
      [LANGS, REGS, KBS, TR] = await Promise.all([be.install.languages(), be.install.regions(), be.install.keyboards(), be.install.transfer().catch(() => null)]);
      void disksLoad.then((d) => {
        if (!d.length || DISKS.length) return;
        // disksFromSystem() stawia na początku dysk z Windows, potem największy (bez pendrive'a z Shrimp).
        DISKS = d; S.disk = 0; defaultSplit();
        if (cur() === "dysk" && !diskLoading) render(false);
      });
      {
        ONLINE = await be.firstboot.online().catch(() => true);
        if (!ONLINE) NETS = await be.firstboot.networks().catch(() => []);
        console.log(`shrimp-setup: internet na nośniku: ${ONLINE ? "jest" : "brak"}, sieci Wi-Fi: ${NETS.length}`);
      }
      // Instalacja: język z Windows (Shrimp Install), a bez niego angielski, nie polski z obrazu sesji live.
      // Podgląd (demo): język przeglądarki. Dla wybranego języka domyślna strefa i klawiatura.
      const own = be.mode === "install" ? installLang(TR, LANGS) : Math.max(0, LANGS.findIndex((l) => l.tag === L.tag)); S.lang = own;
      if (LANGS[own] && LANGS[own].tag !== L.tag) { L = await load([LANGS[own].locale]); chromeText(); }
      const ktz = REGS.findIndex((r) => r.tz === defaultTz(LANGS[own].locale)); if (ktz >= 0) S.region = ktz;
      const plk = KBS.findIndex((k) => k.keymap === defaultKeymap(LANGS[own].locale)); if (plk >= 0) S.kb = plk;
      if (TR) {
        PRE = prefill(TR, LANGS, REGS, KBS);
        if (PRE.lang >= 0 && PRE.lang !== S.lang) { S.lang = PRE.lang; L = await load([LANGS[S.lang].locale]); chromeText(); }
        if (PRE.region >= 0) S.region = PRE.region;
        if (PRE.kb >= 0) { S.kb = PRE.kb; be.install.setKeymap(KBS[S.kb].keymap).catch(() => {}); be.firstboot.setInputSource(KBS[S.kb].keymap).catch(() => {}); }
        if (PRE.full) S.full = PRE.full;
        if (PRE.user) { S.user = PRE.user; S.userEdited = true; }
      }
    }
    if (be.mode !== "install") {
      [INFO, NETS] = await Promise.all([be.firstboot.info(), be.firstboot.networks().catch(() => [])]);
      S.online = INFO.wired || S.online;
      USB = await be.firstboot.usb();
      if (be.mode === "demo") { S.full = "Anna Kowalska"; S.user = "anna"; S.pass = S.pass2 = "shrimp2024"; S.netPass = "dom12345"; }
    }
    const h = parseInt(q.get("step") || location.hash.slice(1), 10);
    const s = parseInt(q.get("sub") || "0", 10);
    go(isNaN(h) ? (be.mode === "firstboot" ? FIRST_BOOT : 0) : h, s);
    if (q.get("err")) openErr(q.get("err") as ErrKey);
  })();
}
