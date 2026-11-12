// Część 1 Setup nad HTTP API Agamy (API v2, Leap 16.1 / Agama 24).
// Interfejs jest serwowany przez agama-web-server (--web-ui-dir), więc /api/* to ten sam origin,
// a token z nośnika (/run/agama/token) trafia do ciasteczka przez /login?token=… (robi to shrimp-setup).
import { LANGS, REGIONS, KEYBOARDS, WINDOWS_SPARE_GB, type Lang, type Region, type Keyboard } from "../data";
import { InstallError, type Disk, type InstallBackend, type InstallPlan, type Progress, type Transfer } from "./types";

declare global { interface Window { SHRIMP_TRANSFER?: Transfer } }

const GB = 1e9;
const PRODUCT = { id: "Shrimp", mode: "immutable" };
/** Akcja Agamy: ponowne czytanie repozytoriów (bez sondowania dysków). */
export const PROBE_SOFTWARE = { probe: { only: ["software"] } };

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  const r = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: body === undefined ? {} : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!r.ok) throw new Error(`${method} ${path}: ${r.status}`);
  const t = await r.text();
  return (t ? JSON.parse(t) : null) as T;
}
const get = <T>(p: string) => call<T>("GET", p);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

// --- typy odpowiedzi Agamy (tylko pola, których używamy) ---
interface ADevice {
  sid: number; name: string; description?: string; class?: string;
  block?: { size: number; encrypted?: boolean; udevPaths?: string[]; systems?: string[]; shrinking?: { supported: boolean; minSize?: number } };
  drive?: { vendor?: string; model?: string; transport?: string; bus?: string };
  filesystem?: { type: string; label?: string };
  encryption?: { type?: string; method?: string };
  partition?: { efi: boolean; id?: string };
  partitionTable?: { unusedSlots: { start: number; size: number }[] };
  partitions?: ADevice[];
}
interface ASystem {
  l10n?: { locales?: { id: string }[]; keymaps?: { id: string }[]; timezones?: { id: string }[] };
  storage?: { devices?: ADevice[]; candidateDrives?: number[] };
}
export interface AStatus {
  stage: "configuring" | "installing" | "finished" | "failed";
  /** Zadania w toku (np. propozycja dysku po PUT /api/config). */
  tasks?: { id: number; name: string; scope: string }[];
  progresses: { scope: string; index: number; size: number; step: string }[];
}
interface AIssue { scope: string; class: string; description: string; details?: string }

/** Problemy Agamy, które blokują instalację. Repozytorium aktualizacji (…/update/…) bez metadanych
 *  (np. Leap tuż po wydaniu albo lustro w trakcie synchronizacji) nie blokuje: system instaluje się
 *  z repozytorium dystrybucji, a aktualizacje przyjdą później przez Cocktail. Test w QEMU: Agama
 *  kończy instalację mimo tego problemu, a Setup wcześniej się na nim zatrzymywał. */
export function blockingIssues(issues: AIssue[]): AIssue[] {
  return issues.filter((i) => !(i.scope === "software" && i.class === "loadSource" && /\/update\//.test(i.details || i.description)));
}

/** Etykieta nośnika z Shrimp Install (FAT32) i obrazu ISO (volid w image-base/config.kiwi). */
export const LIVE_LABEL = "SHRIMP_LIVE";

/** Partycja zaszyfrowana BitLockerem. libstorage-ng pokazuje ją na dwa sposoby, zależnie od wersji:
 *  jako system plików „bitlocker” albo jako szyfrowanie (block.encrypted, bez systemu plików,
 *  typ partycji danych Windows). Wcześniej Setup znał tylko pierwszy sposób, więc na Vivobooku
 *  z BitLockerem (Leap 16.1) dysk wyglądał na pusty. */
export function isBitlocker(p: ADevice): boolean {
  if (/bitlocker/i.test(JSON.stringify([p.filesystem?.type, p.encryption, p.description, p.block?.systems]))) return true;
  return !!p.block?.encrypted && !p.filesystem && /windows|microsoft|basic_data|ntfs/i.test(p.partition?.id || "");
}

/** Nośnik, z którego działa Setup (pendrive z Shrimp Install): nie wolno na nim instalować. */
export function isLiveMedium(d: ADevice): boolean {
  return [d, ...(d.partitions || [])].some((p) => p.filesystem?.label === LIVE_LABEL);
}

/** Dyski z systemu Agamy → dyski Setup. Czysta funkcja (testy w logic.test.ts).
 *  Kolejność: najpierw dyski z Windows, potem od największego. Pendrive z Shrimp, z którego
 *  działa Setup, nie jest dyskiem docelowym (wcześniej bywał pierwszy na liście i Setup
 *  wybierał go domyślnie, a 32 GB to za mało, stąd „za mało miejsca” przy dysku 512 GB). */
export function disksFromSystem(sys: ASystem): Disk[] {
  const devs = sys.storage?.devices || [];
  const cand = new Set(sys.storage?.candidateDrives || devs.map((d) => d.sid));
  const disks = devs.filter((d) => cand.has(d.sid) && d.block && !isLiveMedium(d)).map((d) => {
    const sizeGB = Math.floor(d.block!.size / GB);
    // „Multipath” (opis urządzenia bez producenta i modelu) nie mówi nic: wtedy nazwa urządzenia.
    const named = [d.drive?.vendor, d.drive?.model].filter(Boolean).join(" ").trim();
    const name = named || (d.description && !/multipath/i.test(d.description) ? d.description : d.name);
    const parts = d.partitions || [];
    const bySize = (a: ADevice, b: ADevice) => (b.block?.size || 0) - (a.block?.size || 0);
    // BitLocker: Agama nie zajrzy do środka, więc rozpoznajemy go po typie (isBitlocker).
    const locked = parts.filter(isBitlocker).sort(bySize)[0];
    const win = parts
      .filter((p) => p.filesystem?.type === "ntfs" && (p.block?.systems || []).some((s) => /windows/i.test(s)))
      .sort(bySize)[0] || locked;
    const disk: Disk = { device: d.name, name, sizeGB };
    if (win && win.block) {
      const free = (d.partitionTable?.unusedSlots || []).reduce((s, u) => s + u.size, 0);
      const shrink = win.block.shrinking;
      const used = shrink?.supported && shrink.minSize ? shrink.minSize : win.block.size;
      disk.windows = {
        partition: win.name,
        minGB: Math.ceil(used / GB) + WINDOWS_SPARE_GB,
        poolGB: Math.floor((win.block.size + free) / GB),
        ...(win === locked ? { bitlocker: true } : {}),
      };
    }
    return disk;
  });
  return disks.sort((a, b) => Number(!!b.windows) - Number(!!a.windows) || b.sizeGB - a.sizeGB);
}

/** Plan Setup → konfiguracja Agamy (PUT /api/config). Czysta funkcja. */
export function configFromPlan(plan: InstallPlan, extra: { files?: unknown[]; scripts?: unknown } = {}) {
  const encryption = plan.diskPassword ? { luks2: { password: plan.diskPassword } } : undefined;
  const generate = { generate: { partitions: "default", ...(encryption ? { encryption } : {}) } };
  const partitions: unknown[] =
    plan.place === "beside" && plan.disk.windows
      ? [{ search: plan.disk.windows.partition, size: `${plan.windowsGB} GB` }, generate]
      : [{ search: "*", delete: true }, generate];
  return {
    product: PRODUCT,
    l10n: { locale: plan.locale, keymap: plan.keymap, timezone: plan.timezone },
    user: { fullName: plan.fullName, userName: plan.userName, password: plan.password, hashedPassword: false },
    storage: { drives: [{ search: plan.disk.device, partitions }] },
    ...extra,
  };
}

/** Ile nieudanych odpytań /api/status z rzędu (co sekundę) oznacza, że instalator nie odpowiada. */
export const POLL_MISSES = 30;
/** Po ilu minutach bez żadnej zmiany postępu Setup pyta, czy czekać dalej. */
export const STALL_MIN = 20;

/** Odcisk postępu: zmienia się przy każdym kroku, pakiecie albo nowym zadaniu Agamy. */
export function progressKey(st: AStatus): string {
  return JSON.stringify([st.stage, st.progresses.map((p) => [p.scope, p.index, p.size, p.step]), (st.tasks || []).map((t) => t.id)]);
}

/** Faza instalacji (klucz tłumaczenia) na podstawie zakresu (scope) postępu Agamy. */
export function phaseOf(st: AStatus): { percent: number; phase: string } {
  const p = st.progresses.find((x) => x.scope === "manager") || st.progresses[0];
  const sub = st.progresses.find((x) => x.scope !== "manager");
  const scope = sub?.scope || p?.scope || "storage";
  const phase = scope === "storage" ? "inst.disk" : scope === "software" ? "inst.system" : "inst.files";
  let percent = 0;
  if (p && p.size > 0) {
    const inner = sub && sub !== p && sub.size > 0 ? sub.index / sub.size : 0;
    percent = Math.min(99, ((p.index - 1 + inner) / p.size) * 100);
  }
  return { percent: Math.max(0, percent), phase };
}

/** Plik przekazywany do pierwszego uruchomienia (czyta go shrimp-setup --first-boot). */
function handoff(plan: InstallPlan) {
  return {
    destination: "/var/lib/shrimp-setup/install.json",
    permissions: "0644",
    content: JSON.stringify({ fullName: plan.fullName, userName: plan.userName, beside: plan.place === "beside" }),
  };
}

/** Pełna konfiguracja instalacji: plan + plik dla pierwszego uruchomienia + skrypt po instalacji.
 *  Tę samą funkcję wywołuje test instalacji w QEMU (scripts/agama-test-config.ts). */
export function installConfig(plan: InstallPlan) {
  return configFromPlan(plan, {
    files: [handoff(plan)],
    scripts: { post: [{ name: "shrimp-post-install", chroot: false, url: "file:///usr/share/shrimp-setup/post-install.sh" }] },
  });
}

export class AgamaBackend implements InstallBackend {
  private sys: Promise<ASystem> | null = null;
  private system() { return (this.sys ||= get<ASystem>("/api/system")); }

  async transfer() { return window.SHRIMP_TRANSFER || null; }
  async languages(): Promise<Lang[]> {
    const ids = new Set(((await this.system()).l10n?.locales || []).map((l) => l.id));
    return ids.size ? LANGS.filter((l) => ids.has(l.locale)) : LANGS;
  }
  async regions(): Promise<Region[]> {
    const ids = new Set(((await this.system()).l10n?.timezones || []).map((t) => t.id));
    return ids.size ? REGIONS.filter((r) => ids.has(r.tz)) : REGIONS;
  }
  async keyboards(): Promise<Keyboard[]> {
    const ids = new Set(((await this.system()).l10n?.keymaps || []).map((k) => k.id));
    return ids.size ? KEYBOARDS.filter((k) => ids.has(k.keymap)) : KEYBOARDS;
  }
  async disks(): Promise<Disk[]> {
    // Agama sprawdza dyski dopiero po starcie (libstorage-ng, może to trwać dłużej na laptopach z NVMe/RST).
    // Pierwszy odczyt bywa więc pusty albo niepełny (jeden dysk, bez układu partycji): czekamy, aż Agama
    // skończy zadania, czytamy od nowa i ponawiamy, dopóki lista się nie ustabilizuje (najwyżej ok. 40 s).
    let sys = await get<ASystem>("/api/system");
    let disks = disksFromSystem(sys);
    for (let i = 0; i < 20; i++) {
      await this.settle(20);
      const next = await get<ASystem>("/api/system");
      const nd = disksFromSystem(next);
      const same = JSON.stringify(nd) === JSON.stringify(disks);
      sys = next; disks = nd;
      if (disks.length && same) break;
      await sleep(2000);
    }
    // Do dziennika (journalctl --user): co widzi Agama, gdy na sprzęcie coś nie gra.
    console.log("shrimp-setup: dyski Agamy", JSON.stringify((sys.storage?.devices || []).map((d) => ({
      name: d.name, size: d.block?.size, live: isLiveMedium(d),
      parts: (d.partitions || []).map((p) => [p.name, p.block?.size, p.filesystem?.type, p.filesystem?.label, p.block?.encrypted, p.partition?.id, p.description]),
    }))), "→ Setup", JSON.stringify(disks));
    return disks;
  }

  async setLanguage(locale: string) {
    await call("POST", "/api/action", { configureL10n: { locale } });
  }
  async setKeymap(keymap: string) {
    await call("POST", "/api/action", { configureL10n: { keymap } });
  }

  /** Czeka, aż Agama przeliczy propozycję po zmianie konfiguracji. Agama 24 zaraz po PUT /api/config
   *  ma puste „progresses”, ale zadania (storage_config, software_config…) dalej są w „tasks”. */
  private async settle(tries = 240) {
    for (let i = 0; i < tries; i++) {
      const st = await get<AStatus>("/api/status");
      if (!st.progresses.length && !(st.tasks || []).length) return;
      await sleep(500);
    }
  }

  /** Po podłączeniu do internetu: Agama czyta repozytoria od nowa, potem wybiera produkt jeszcze raz.
   *  Bez internetu przy wyborze produktu zgłasza „Could not read the repositories; Failed to find the product”.
   *  Akcja to `{"probe":{"only":["software"]}}`: Agama 24 ma `Probe { only: Option<Vec<Scope>> }`, więc
   *  `{"probe":null}` (do Bety 4) kończyło się HTTP 422, wyjątek przerywał reprobe() i Setup pokazywał
   *  „Brak połączenia z internetem” mimo działającego Wi-Fi. Bez „only” Agama sondowałaby też dyski.
   *  Zwraca opis problemu (do okna błędu i dziennika) albo "" gdy repozytoria są czytelne. */
  async reprobe(): Promise<string> {
    // Nieudane sondowanie nie przerywa: ponowny wybór produktu (PUT) też czyta repozytoria od nowa
    // (test QEMU: po 422 na probe sam PUT usunął problem z repozytoriami).
    await call("POST", "/api/action", PROBE_SOFTWARE).catch((e) => console.log("shrimp-setup: probe:", String(e)));
    await this.settle();
    await call("PUT", "/api/config", { product: PRODUCT });
    await this.settle();
    const issues = await get<AIssue[]>("/api/issues");
    const soft = blockingIssues(issues.filter((i) => i.scope === "software"));
    console.log("shrimp-setup: po ponownym czytaniu repozytoriów problemy:", JSON.stringify(soft));
    return soft.map((i) => i.description).join("; ");
  }

  async check(plan: InstallPlan) {
    await call("PUT", "/api/config", configFromPlan(plan));
    await this.settle();
    const issues = blockingIssues(await get<AIssue[]>("/api/issues"));
    if (issues.length) console.log("shrimp-setup: problemy Agamy", JSON.stringify(issues));
    const storage = issues.filter((i) => i.scope === "storage");
    if (storage.length) throw new InstallError("space", storage.map((i) => i.description).join("; "));
    if (issues.length) throw new InstallError("install", issues.map((i) => i.description).join("; "));
  }

  async install(plan: InstallPlan, onProgress: (p: Progress) => void) {
    await call("PUT", "/api/config", installConfig(plan));
    await this.settle();
    await call("POST", "/api/action", { install: null });
    await this.watch(onProgress);
  }

  /** Śledzi /api/status. Pojedyncze nieudane odpytanie (restart usługi, chwilowy brak odpowiedzi) nie przerywa
   *  instalacji; dopiero POLL_MISSES z rzędu to „instalator nie odpowiada”. Postęp, który stoi STALL_MIN minut,
   *  to „trwa dłużej niż zwykle” (wtedy użytkownik może czekać dalej: watch() jeszcze raz). */
  async watch(onProgress: (p: Progress) => void) {
    const t0 = Date.now();
    let misses = 0, last = "", changed = Date.now();
    for (;;) {
      await sleep(1000);
      let st: AStatus;
      try { st = await get<AStatus>("/api/status"); misses = 0; }
      catch (e) { if (++misses >= POLL_MISSES) throw new InstallError("agama", String(e)); continue; }
      if (st.stage === "failed") throw new InstallError("install");
      if (st.stage === "finished") { onProgress({ percent: 100, phase: "done" }); return; }
      const key = progressKey(st);
      if (key !== last) { last = key; changed = Date.now(); }
      else if (Date.now() - changed > STALL_MIN * 60000) throw new InstallError("stall");
      const { percent, phase } = phaseOf(st);
      const el = (Date.now() - t0) / 60000;
      const etaMin = percent > 3 ? Math.max(1, Math.ceil((el / percent) * (100 - percent))) : 11;
      onProgress({ percent, phase, etaMin });
    }
  }

  async reboot() { await call("POST", "/api/action", { finish: "reboot" }); }
}
