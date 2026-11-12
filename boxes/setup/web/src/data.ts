import catalog from "./windows-apps.json";
import { GNOME_LOCALES, LANGS as KNOWN } from "../../../i18n-core/langs";
// Stałe dane Setup. Teksty są w boxes/setup/i18n (klucze); tu zostają identyfikatory, kolejność i dane techniczne.

/** `tag` to język tłumaczeń (i18n), `locale` identyfikator locale dla Agamy. */
export interface Lang { name: string; locale: string; tag: string }
/** Kraj (kod ISO), strefa czasowa i klucz miasta, gdy kraj ma kilka stref. Nazwy liczy Intl, miasta tłumaczy i18n. */
export interface Region { code: string; tz: string; city?: string }
/** Układ xkb; nazwa to język (i kraj) z Intl, `note` to klucz tłumaczenia wariantu. */
export interface Keyboard { keymap: string; lang: string; region?: string; note?: string }

/** Wszystkie języki GNOME. Agama pokaże te, które zna system. */
export const LANGS: Lang[] = GNOME_LOCALES.map(([l, tag, name]) => ({ name: name || KNOWN.find((k) => k.tag === tag)?.name || tag, locale: l + ".UTF-8", tag }));

export const REGIONS: Region[] = ([
  ["PL", "Europe/Warsaw"], ["DE", "Europe/Berlin"], ["AT", "Europe/Vienna"], ["BE", "Europe/Brussels"], ["HR", "Europe/Zagreb"], ["CZ", "Europe/Prague"],
  ["DK", "Europe/Copenhagen"], ["EE", "Europe/Tallinn"], ["FI", "Europe/Helsinki"], ["FR", "Europe/Paris"], ["GR", "Europe/Athens"], ["ES", "Europe/Madrid"],
  ["ES", "Atlantic/Canary", "canary"], ["NL", "Europe/Amsterdam"], ["IE", "Europe/Dublin"], ["IS", "Atlantic/Reykjavik"], ["LT", "Europe/Vilnius"],
  ["LV", "Europe/Riga"], ["NO", "Europe/Oslo"], ["PT", "Europe/Lisbon"], ["RO", "Europe/Bucharest"], ["SK", "Europe/Bratislava"], ["CH", "Europe/Zurich"],
  ["SE", "Europe/Stockholm"], ["TR", "Europe/Istanbul"], ["UA", "Europe/Kyiv"], ["HU", "Europe/Budapest"], ["GB", "Europe/London"], ["IT", "Europe/Rome"],
  ["BG", "Europe/Sofia"], ["SI", "Europe/Ljubljana"], ["RS", "Europe/Belgrade"], ["BA", "Europe/Sarajevo"], ["MK", "Europe/Skopje"], ["AL", "Europe/Tirane"],
  ["ME", "Europe/Podgorica"], ["LU", "Europe/Luxembourg"], ["MT", "Europe/Malta"], ["CY", "Asia/Nicosia"], ["BY", "Europe/Minsk"], ["MD", "Europe/Chisinau"],
  ["FO", "Atlantic/Faroe"], ["RU", "Europe/Moscow"],
  ["US", "America/New_York", "newyork"], ["US", "America/Chicago", "chicago"], ["US", "America/Denver", "denver"], ["US", "America/Los_Angeles", "losangeles"],
  ["CA", "America/Toronto", "toronto"], ["CA", "America/Vancouver", "vancouver"], ["MX", "America/Mexico_City"], ["BR", "America/Sao_Paulo"],
  ["AR", "America/Argentina/Buenos_Aires"], ["CL", "America/Santiago"], ["CO", "America/Bogota"], ["PE", "America/Lima"], ["VE", "America/Caracas"],
  ["UY", "America/Montevideo"],
  ["AE", "Asia/Dubai"], ["SA", "Asia/Riyadh"], ["IQ", "Asia/Baghdad"], ["IR", "Asia/Tehran"], ["IL", "Asia/Jerusalem"], ["JO", "Asia/Amman"], ["LB", "Asia/Beirut"],
  ["GE", "Asia/Tbilisi"], ["AM", "Asia/Yerevan"], ["AZ", "Asia/Baku"], ["KZ", "Asia/Almaty"], ["UZ", "Asia/Tashkent"], ["KG", "Asia/Bishkek"], ["TJ", "Asia/Dushanbe"],
  ["MN", "Asia/Ulaanbaatar"], ["IN", "Asia/Kolkata"], ["PK", "Asia/Karachi"], ["BD", "Asia/Dhaka"], ["LK", "Asia/Colombo"], ["NP", "Asia/Kathmandu"],
  ["BT", "Asia/Thimphu"], ["MM", "Asia/Yangon"], ["TH", "Asia/Bangkok"], ["LA", "Asia/Vientiane"], ["KH", "Asia/Phnom_Penh"], ["VN", "Asia/Ho_Chi_Minh"],
  ["MY", "Asia/Kuala_Lumpur"], ["SG", "Asia/Singapore"], ["ID", "Asia/Jakarta"], ["PH", "Asia/Manila"], ["CN", "Asia/Shanghai"], ["TW", "Asia/Taipei"],
  ["HK", "Asia/Hong_Kong"], ["KR", "Asia/Seoul"], ["JP", "Asia/Tokyo"],
  ["AU", "Australia/Sydney", "sydney"], ["AU", "Australia/Perth", "perth"], ["NZ", "Pacific/Auckland"],
  ["ZA", "Africa/Johannesburg"], ["EG", "Africa/Cairo"], ["ET", "Africa/Addis_Ababa"], ["KE", "Africa/Nairobi"], ["NG", "Africa/Lagos"], ["DZ", "Africa/Algiers"],
  ["MA", "Africa/Casablanca"], ["RW", "Africa/Kigali"], ["MG", "Indian/Antananarivo"],
] as [string, string, string?][]).map(([code, tz, city]) => ({ code, tz, city }));

/** Układy klawiatury; identyfikatory w formacie Agamy (xkb „układ(wariant)”). */
export const KEYBOARDS: Keyboard[] = ([
  ["pl", "pl", "", "kb.programmer"], ["pl(qwertz)", "pl", "", "kb.typist"], ["us", "en", "US"], ["gb", "en", "GB"], ["de", "de"], ["fr", "fr"], ["es", "es"],
  ["cz", "cs"], ["ua", "uk"], ["ru", "ru"], ["it", "it"], ["pt", "pt", "PT"], ["br", "pt", "BR"], ["latam", "es", "419"], ["nl", "nl"], ["be", "nl", "BE"],
  ["ch", "de", "CH"], ["ca", "fr", "CA"], ["sk", "sk"], ["se", "sv"], ["no", "nb"], ["dk", "da"], ["fi", "fi"], ["hu", "hu"], ["lt", "lt"], ["lv", "lv"],
  ["ee", "et"], ["ro", "ro"], ["bg", "bg"], ["hr", "hr"], ["si", "sl"], ["rs", "sr"], ["ba", "bs"], ["mk", "mk"], ["al", "sq"], ["gr", "el"], ["tr", "tr"],
  ["is", "is"], ["ie", "ga"], ["mt", "mt"], ["by", "be"], ["ge", "ka"], ["am", "hy"], ["az", "az"], ["kz", "kk"], ["kg", "ky"], ["uz", "uz"], ["tj", "tg"],
  ["mn", "mn"], ["il", "he"], ["ara", "ar"], ["ir", "fa"], ["in", "hi"], ["lk", "si"], ["np", "ne"], ["mm", "my"], ["th", "th"], ["la", "lo"], ["kh", "km"],
  ["vn", "vi"], ["ph", "fil"], ["jp", "ja"], ["kr", "ko"], ["cn", "zh"], ["et", "am"],
] as [string, string, string?, string?][]).map(([keymap, lang, region, note]) => ({ keymap, lang, region: region || undefined, note }));

/** Domyślny układ dla języka (locale z krajem ma pierwszeństwo przed samym językiem). */
const KB_FOR: Record<string, string> = {
  pl: "pl", en: "us", en_GB: "gb", de: "de", fr: "fr", es: "es", it: "it", pt_PT: "pt", pt_BR: "br", nl: "nl", cs: "cz", sk: "sk", uk: "ua", ru: "ru",
  sv: "se", nb: "no", nn: "no", da: "dk", fi: "fi", hu: "hu", lt: "lt", lv: "lv", et: "ee", ro: "ro", bg: "bg", hr: "hr", sl: "si", sr: "rs", bs: "ba",
  mk: "mk", sq: "al", el: "gr", tr: "tr", is: "is", ga: "ie", gd: "gb", cy: "gb", ja: "jp", ko: "kr", zh: "cn", ar: "ara", he: "il", fa: "ir", hi: "in",
  bn: "in", ta: "in", te: "in", mr: "in", gu: "in", kn: "in", ml: "in", pa: "in", as: "in", or: "in", si: "lk", ne: "np", my: "mm", th: "th", lo: "la",
  km: "kh", vi: "vn", ka: "ge", hy: "am", az: "az", kk: "kz", ky: "kg", uz: "uz", tg: "tj", mn: "mn", be: "by", mt: "mt", am: "et", ca: "es", eu: "es", gl: "es",
};
export function defaultKeymap(locale: string): string {
  const [lang, cc] = locale.split(/[.@]/)[0].split("_");
  return KB_FOR[`${lang}_${cc}`] || KB_FOR[lang] || "us";
}
/** Domyślna strefa dla języka: pierwszy region z kodem kraju z locale (pl_PL → PL). */
export function defaultTz(locale: string): string | undefined {
  const cc = locale.split(/[.@]/)[0].split("_")[1];
  return REGIONS.find((r) => r.code === cc)?.tz;
}

export const HELLO = ["Witaj", "Hello", "Hallo", "Bonjour", "Hola", "Ciao"];
/** Klucze tłumaczeń (lub zwykły tekst, jeśli nie ma takiego klucza). Pierwszy element tych list jest zalecany. */
export const ENCS = ["enc.off", "enc.on"];
export const HSEC = ["none", "WPA2", "WPA3"];
export const IPS = ["auto", "net.manual"];
export const DNSS = ["auto", "Cloudflare", "Google"];
export const PROXY = ["none", "auto"];
export const THEMES = ["look.light", "look.dark"];
export const DRV = ["drv.install", "drv.later"];
export const NV = ["nv.driver", "nv.open"];

/** Kolory dodatkowe GNOME: klucz nazwy, wartość `accent-color`, kolor. */
export const ACCENTS: [string, string, string][] = [
  ["acc.blue", "blue", "#3584e4"], ["acc.teal", "teal", "#2190a4"], ["acc.green", "green", "#3a944a"],
  ["acc.yellow", "yellow", "#c88800"], ["acc.orange", "orange", "#ed5b00"], ["acc.red", "red", "#e62d42"],
  ["acc.pink", "pink", "#d56199"], ["acc.purple", "purple", "#9141ac"], ["acc.slate", "slate", "#6f8396"],
];

/** Gdzie zainstalować: klucz nazwy i klucz opisu. */
export const PLACES = [
  { n: "place.beside", d: "" },
  { n: "place.replace", d: "place.replace.d" },
];

/** Minimum dla Shrimp i zapas ponad zajęte miejsce Windows (GB). */
export const SHRIMP_MIN_GB = 40;
export const WINDOWS_SPARE_GB = 20;

/** `what` to klucz tłumaczenia opisu. */
export interface App { id: string; name: string; what: string }
export const USES = ["use.work", "use.games", "use.create", "use.dev", "use.media"];
/** Kategorie → aplikacje z Flathub (specs/setup.md › Programy). */
export const APPS: App[][] = [
  [{ id: "org.libreoffice.LibreOffice", name: "LibreOffice", what: "app.office" }, { id: "org.mozilla.thunderbird", name: "Thunderbird", what: "app.mail" }],
  [{ id: "com.valvesoftware.Steam", name: "Steam", what: "app.games" }, { id: "com.heroicgameslauncher.hgl", name: "Heroic", what: "app.epic" }],
  [{ id: "org.kde.krita", name: "Krita", what: "app.draw" }, { id: "org.kde.kdenlive", name: "Kdenlive", what: "app.video" }, { id: "org.gimp.GIMP", name: "GIMP", what: "app.graphics" }],
  [{ id: "com.visualstudio.code", name: "Visual Studio Code", what: "app.code" }, { id: "org.gnome.Builder", name: "GNOME Builder", what: "app.ide" }],
  [{ id: "org.videolan.VLC", name: "VLC", what: "app.player" }, { id: "com.spotify.Client", name: "Spotify", what: "app.music" }],
];

/** Program z Windows (lista zapisana przez Shrimp Install) i jego odpowiednik. */
export interface WinApp { windows: string; name: string; id: string }

/**
 * Katalog programów Windows (windows-apps.json): cała logika dopasowania jest w danych.
 * `re` sprawdza nazwę z listy programów Windows (DisplayName, np. „Microsoft 365 Apps for business - pl-pl”),
 * `store` nazwę pakietu Microsoft Store. Kolejność wpisów = kolejność podpowiedzi. Wpis bez `linux` czeka na odpowiednik.
 */
export interface CatalogApp {
  windows: string; re: RegExp; store?: RegExp;
  linux: { name: string; id: string } | null;
  /** Miejsce (1–100) na liście najpopularniejszych programów Windows: takie wpisy idą w podpowiedziach pierwsze. */
  top?: number;
  /** Odpowiednik jest już w systemie, nie instalujemy go. */
  base?: boolean;
  /** driver, builtin, unneeded: nic do zaproponowania, bo system robi to sam. */
  skip?: "driver" | "builtin" | "unneeded";
}
interface CatalogJson { apps: { windows: string; top?: number; re: string; store?: string; linux: { name: string; id: string } | null; base?: boolean; skip?: CatalogApp["skip"] }[] }

export const WIN_CATALOG: CatalogApp[] = (catalog as CatalogJson).apps.map((a) => ({
  ...a, re: new RegExp(a.re, "i"), store: a.store ? new RegExp(a.store, "i") : undefined,
}));

/** Katalog w kolejności podpowiedzi: najpierw 100 najpopularniejszych (wg `top`), potem reszta w kolejności wpisów. */
export const WIN_CATALOG_ORDERED: CatalogApp[] = [...WIN_CATALOG.filter((a) => a.top).sort((a, b) => a.top! - b.top!), ...WIN_CATALOG.filter((a) => !a.top)];

/** Układ klawiatury Windows (KLID) → układ xkb. Poza tą listą liczy się język układu (4 ostatnie cyfry). */
export const KLID_XKB: Record<string, string> = {
  "00000415": "pl", "00010415": "pl(qwertz)", "00000409": "us", "00020409": "us", "00000809": "gb",
  "00000407": "de", "0000040c": "fr", "0000040a": "es", "00000c0a": "es", "00000405": "cz",
  "00000422": "ua", "00020422": "ua", "00000419": "ru",
};
export const LANGID_XKB: Record<string, string> = {
  "0415": "pl", "0409": "us", "0809": "gb", "0407": "de", "040c": "fr", "040a": "es", "0c0a": "es", "0405": "cz", "0422": "ua", "0419": "ru",
};
/** Stare nazwy stref, które Windows (ICU) może jeszcze zwracać. */
export const TZ_ALIAS: Record<string, string> = { "Europe/Kiev": "Europe/Kyiv", "Asia/Calcutta": "Asia/Kolkata", "America/Buenos_Aires": "America/Argentina/Buenos_Aires" };

export type ErrKey = "pass" | "dpass" | "space" | "nodisk" | "install" | "stall" | "agama" | "user" | "wifi" | "net" | "offline" | "repo";
/** Klucze tłumaczeń okna błędu: tytuł, opis, przycisk główny, przycisk dodatkowy (opcjonalny). */
export const ERR: Record<ErrKey, { t: string; d: string; p: string; s?: string }> = {
  dpass: { t: "err.dpass.t", d: "err.dpass.d", p: "err.retry" },
  pass: { t: "err.pass.t", d: "err.pass.d", p: "err.retry" },
  space: { t: "err.space.t", d: "err.space.d", p: "err.space.p", s: "err.space.s" },
  nodisk: { t: "err.nodisk.t", d: "err.nodisk.d", p: "err.retry" },
  install: { t: "err.install.t", d: "err.install.d", p: "err.retry", s: "err.install.s" },
  stall: { t: "err.stall.t", d: "err.stall.d", p: "err.stall.p", s: "err.install.s" },
  agama: { t: "err.agama.t", d: "err.agama.d", p: "err.agama.p", s: "err.install.s" },
  user: { t: "err.user.t", d: "err.user.d", p: "err.retry" },
  wifi: { t: "err.wifi.t", d: "err.wifi.d", p: "err.retry", s: "skip" },
  net: { t: "err.net.t", d: "err.net.d", p: "err.retry", s: "skip" },
  offline: { t: "err.net.t", d: "err.offline.d", p: "err.retry" },
  repo: { t: "err.repo.t", d: "err.offline.repo", p: "err.retry" },
};

/** Slajdy końcowe: klucze tytułu i opisu; `win` tylko gdy Shrimp stoi obok Windows, `extra` dopisek, gdy obok Windows. */
export const SLIDES: { t: string; d: string; win?: boolean; extra?: string }[] = [
  { t: "slide.linux.t", d: "slide.linux.d" },
  { t: "slide.win.t", d: "slide.win.d", win: true },
  { t: "slide.apps.t", d: "slide.apps.d" },
  { t: "slide.pass.t", d: "slide.pass.d" },
  { t: "slide.files.t", d: "slide.files.d", extra: "slide.files.win" },
  { t: "slide.panko.t", d: "slide.panko.d" },
  { t: "slide.cocktail.t", d: "slide.cocktail.d" },
  { t: "slide.search.t", d: "slide.search.d" },
  { t: "slide.help.t", d: "slide.help.d" },
];
export const slides = (beside: boolean) => SLIDES.filter((s) => !s.win || beside);
