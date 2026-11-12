// Czyste funkcje Setup (testy: logic.test.ts).
import { resolveTag, type I18n } from "../../../i18n-core/core";
import { APPS, WIN_CATALOG_ORDERED, KLID_XKB, LANGID_XKB, TZ_ALIAS, defaultTz, type WinApp, type Lang, type Region, type Keyboard } from "./data";
import type { Transfer } from "./backend/types";

export const esc = (s: string) =>
  String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/** Pierwsze słowo imienia i nazwiska („Cześć, Anna”). */
export function firstName(full: string, fallback = "") { return full.trim().split(/\s+/)[0] || fallback; }

/** Nazwa użytkownika z imienia: małe litery, bez polskich znaków, nie zaczyna się cyfrą, najwyżej 32 znaki (useradd). */
export function slug(s: string) {
  const map: Record<string, string> = { ł: "l", ø: "o", đ: "d", ð: "d", ß: "ss", æ: "ae", œ: "oe", ı: "i", þ: "th" };
  return s.toLowerCase().replace(/[łøđðßæœıþ]/g, (c) => map[c]).normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/[^a-z0-9]/g, "")
    .replace(/^[0-9]+/, "").slice(0, 32);
}

/** Konta i grupy, które istnieją w systemie (Leap 16 + GNOME + Shrimp); konto użytkownika nie może się tak nazywać. */
const RESERVED_USERS = new Set([
  "root", "bin", "daemon", "adm", "lp", "sync", "shutdown", "halt", "mail", "news", "uucp", "operator", "games", "ftp",
  "nobody", "man", "messagebus", "polkitd", "systemd", "dbus", "gdm", "rtkit", "pipewire", "pulse", "avahi", "colord",
  "geoclue", "flatpak", "nm", "dnsmasq", "chrony", "srvgeoclue", "sshd", "tss", "usbmux", "wheel", "users", "audio",
  "video", "input", "render", "kvm", "shadow", "sudo", "tty", "disk", "utmp", "admin", "shrimp", "agama", "snapper",
]);
export const isReservedUser = (u: string) => RESERVED_USERS.has(u);

/** Proponowana nazwa konta z imienia i nazwiska (albo z nazwy konta Windows). Imię bez liter łacińskich
 *  (np. 李明, Ελένη bez transliteracji) daje „user”; nazwa zajęta przez system dostaje cyfrę („admin1”). */
export function loginFrom(full: string, windowsName = ""): string {
  let u = slug(windowsName) || slug(firstName(full, ""));
  if (!u && full.trim()) u = "user";
  return isReservedUser(u) ? u.slice(0, 31) + "1" : u;
}

/** Godzina w strefie, w formacie języka interfejsu. */
export function tnow(L: I18n, z: string) {
  try { return new Intl.DateTimeFormat(L.locale, { timeZone: z, hour: "2-digit", minute: "2-digit" }).format(new Date()); } catch { return ""; }
}

const names = (L: I18n, type: "region" | "language") => { try { return new Intl.DisplayNames([L.locale], { type, fallback: "code" }); } catch { return null; } };

/** „Polska”, „Stany Zjednoczone · Nowy Jork”: kraj z Intl, miasto z tłumaczeń. */
export function regionLabel(L: I18n, r: Region): string {
  const country = names(L, "region")?.of(r.code) || r.code;
  return r.city ? `${country} · ${L.t("city." + r.city)}` : country;
}

/** Nazwa strefy czasowej w języku interfejsu („czas środkowoeuropejski standardowy”). */
export function zoneName(L: I18n, tz: string): string {
  try {
    const parts = new Intl.DateTimeFormat(L.locale, { timeZone: tz, timeZoneName: "long" }).formatToParts(new Date());
    return parts.find((p) => p.type === "timeZoneName")?.value || tz.replace(/_/g, " ");
  } catch { return tz.replace(/_/g, " "); }
}

/** „Polski (programisty)”, „angielski (Stany Zjednoczone)”: język i kraj z Intl, wariant z tłumaczeń. */
export function keyboardName(L: I18n, k: Keyboard): string {
  const lang = names(L, "language")?.of(k.lang) || k.lang;
  const extra = [k.region ? names(L, "region")?.of(k.region) || k.region : "", k.note ? L.t(k.note) : ""].filter(Boolean).join(", ");
  return (lang.charAt(0).toLocaleUpperCase(L.locale) + lang.slice(1)) + (extra ? ` (${extra})` : "");
}

export interface Pick { key: string; id: string; label: string }

/** „Twoje programy z Windows”: maks. 5 pozycji; inny odpowiednik jako „A → B”. */
export function windowsList(apps: WinApp[] | null): Pick[] {
  return (apps || []).slice(0, 5).map((a) => ({
    key: "w:" + a.windows,
    id: a.id,
    label: a.windows === a.name ? esc(a.name) : esc(a.windows) + '<span class="arr">→</span>' + esc(a.name),
  }));
}

/**
 * Programy z shrimp.json (nazwy z listy programów Windows i pakiety Microsoft Store) → odpowiedniki z Flathub,
 * najpierw 100 najpopularniejszych (pole `top`), potem w kolejności katalogu (windows-apps.json), bez powtórzeń. Pomijamy nieznane, wpisy bez odpowiednika
 * i odpowiedniki, które system ma od razu.
 */
export function mapWindowsApps(names: string[], store: string[] = []): WinApp[] {
  const out: WinApp[] = [];
  for (const m of WIN_CATALOG_ORDERED) {
    if (!m.linux || m.base || out.some((a) => a.id === m.linux!.id)) continue;
    if (names.some((n) => m.re.test(n.trim())) || (m.store && store.some((s) => m.store!.test(s)))) out.push({ windows: m.windows, name: m.linux.name, id: m.linux.id });
  }
  return out;
}

/** Układ Windows (KLID, np. „00000415”) → xkb, jeśli Setup go zna. */
export function klidToXkb(klid: string, known: string[]): string | null {
  const k = klid.toLowerCase();
  const x = KLID_XKB[k] || (/^0000/.test(k) ? LANGID_XKB[k.slice(4)] : undefined);
  return x && known.includes(x) ? x : null;
}

export interface Prefill { lang: number; region: number; kb: number; full: string; user: string }

/** Wartości kroków 1–6 z Shrimp Install. -1: brak podpowiedzi (zostaje domyślna). */
/** Język z Windows (pl-PL, sr-Latn-RS, de…) → indeks na liście języków GNOME albo -1. */
export function langIndex(raw: string | undefined, langs: Lang[]): number {
  if (!raw) return -1;
  const lc = raw.replace("-", "_");
  let lang = langs.findIndex((l) => l.locale.startsWith(lc + "."));
  if (lang < 0) {
    // Inny zapis (sr-Latn-RS, zh-Hant-TW) albo język bez tego kraju: najlepsze dopasowanie z listy języków GNOME.
    const tag = resolveTag(raw), country = /[-_]([A-Za-z]{2})\b/.exec(raw)?.[1]?.toUpperCase();
    if (tag) {
      lang = langs.findIndex((l) => l.tag === tag && country && l.locale.split(/[.@]/)[0].endsWith("_" + country));
      if (lang < 0) lang = langs.findIndex((l) => l.tag === tag);
    }
  }
  return lang;
}

/** Domyślny język instalacji: z Windows (shrimp.json), potem język menu GRUB zapisany przez Shrimp Install
 *  (też z Windows), na końcu angielski. Nigdy język sesji live, bo obraz ISO ma na sztywno pl_PL. */
export function installLang(t: Transfer | null, langs: Lang[]): number {
  for (const raw of [t?.locale?.language, t?.locale?.format, t?.grubLang, "en-US", "en"]) {
    const i = langIndex(raw, langs);
    if (i >= 0) return i;
  }
  return 0;
}

export function prefill(t: Transfer, langs: Lang[], regs: Region[], kbs: Keyboard[]): Prefill {
  const lang = langIndex(t.locale?.language || t.locale?.format || "", langs);
  const tz = t.locale?.timezone ? TZ_ALIAS[t.locale.timezone] || t.locale.timezone : lang >= 0 ? defaultTz(langs[lang].locale) : undefined;
  const region = tz ? regs.findIndex((r) => r.tz === tz) : -1;
  const keys = kbs.map((k) => k.keymap);
  const x = (t.locale?.keyboards || []).map((k) => klidToXkb(k, keys)).find((k) => k);
  const kb = x ? keys.indexOf(x) : -1;
  const full = (t.user?.fullName || "").trim();
  const user = loginFrom(full, t.user?.name || "");
  return { lang, region, kb, full, user };
}

/** „Proponowane programy”: maks. 5, po jednym z każdej wybranej kategorii na zmianę, bez duplikatów i bez tych z Windows. */
export function proposals(L: I18n, use: boolean[], winApps: WinApp[] | null): Pick[] {
  const out: Pick[] = [];
  const seen = new Set((winApps || []).slice(0, 5).map((a) => a.id));
  const sel = use.map((u, i) => (u ? i : -1)).filter((i) => i >= 0);
  for (let r = 0, any = true; any && out.length < 5; r++) {
    any = false;
    for (const i of sel) {
      const a = APPS[i][r];
      if (!a) continue;
      any = true;
      if (!seen.has(a.id) && out.length < 5) { seen.add(a.id); out.push({ key: a.id, id: a.id, label: `${esc(a.name)} <span class="meta">${esc(L.t(a.what))}</span>` }); }
    }
  }
  return out;
}
