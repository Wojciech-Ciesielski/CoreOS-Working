// Część 2 Setup: pierwsza konfiguracja. Mówi z lokalnym serwerem shrimp-setup (Rust)
// pod tym samym originem: /shrimp/api/*. Token sesji przychodzi w adresie strony (?k=…).
import { mapWindowsApps } from "../logic";
import { OfflineError, WifiError, type UsbResult, type UsbState, type DriverChoice, type FirstBootBackend, type FirstBootInfo, type Network, type NetworkAdvanced, type WifiDiag, type Progress } from "./types";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Serwer shrimp-setup: po restarcie ten sam origin (/shrimp/api, klucz w adresie ?k=…),
 *  na nośniku osobny port, który shrimp-setup podaje stronie Agamy w window.SHRIMP_LOCAL (tylko sieć). */
declare global { interface Window { SHRIMP_LOCAL?: { url: string; key: string } } }

function caller(base: string, key: string) {
  return async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
    const r = await fetch(base + path, {
      method,
      headers: { "X-Shrimp-Key": key, ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const t = await r.text();
    const data = t ? JSON.parse(t) : null;
    if (!r.ok) {
      const kind = data?.error;
      if (kind === "wifi") throw new WifiError(data.message);
      if (kind === "offline") throw new OfflineError(data.message);
      throw new Error(data?.message || `${method} ${path}: ${r.status}`);
    }
    return data as T;
  };
}

interface Job { state: "running" | "done" | "failed" | "offline"; percent: number; phase: string; etaMin?: number }

export class ShrimpFirstBoot implements FirstBootBackend {
  private call: <T>(method: string, path: string, body?: unknown) => Promise<T>;
  constructor(base = "/shrimp/api", key = new URLSearchParams(location.search).get("k") || "") { this.call = caller(base, key); }
  async online() { return (await this.call<{ online: boolean }>("GET", "/online")).online; }
  /** Serwer zwraca programy z shrimp.json (nazwy i pakiety Store); odpowiedniki dobieramy tutaj. */
  async info(): Promise<FirstBootInfo> {
    const i = await this.call<Omit<FirstBootInfo, "windowsApps"> & { windowsApps: string[] | null; storeApps?: string[] | null }>("GET", "/info");
    const { storeApps, ...rest } = i;
    const apps = i.windowsApps || storeApps ? mapWindowsApps(i.windowsApps || [], storeApps || []) : null;
    return { ...rest, windowsApps: apps && apps.length ? apps : null };
  }
  networks() { return this.call<Network[]>("GET", "/networks"); }
  wifiDiag() { return this.call<WifiDiag>("GET", "/network/diag"); }
  async connect(ssid: string, password?: string) { await this.call("POST", "/network/connect", { ssid, password }); }
  async connectHidden(ssid: string, security: "none" | "wpa2" | "wpa3", password?: string) {
    await this.call("POST", "/network/connect", { ssid, password, security, hidden: true });
  }
  async setNetworkAdvanced(a: NetworkAdvanced) { await this.call("POST", "/network/advanced", a); }
  async setAppearance(scheme: "default" | "prefer-dark", accent: string | null) { await this.call("POST", "/appearance", { scheme, accent }); }
  async setInputSource(keymap: string) { await this.call("POST", "/keymap", { keymap }); }
  async setDrivers(c: DriverChoice) { await this.call("POST", "/drivers", c); }
  async installApps(ids: string[], onProgress: (p: Progress) => void) {
    await this.call("POST", "/apps", { ids });
    for (;;) {
      await sleep(700);
      const j = await this.call<Job>("GET", "/apps");
      if (j.state === "offline") throw new OfflineError();
      if (j.state === "failed") throw new OfflineError(j.phase);
      onProgress({ percent: j.percent, phase: j.state === "done" ? "done" : j.phase, etaMin: j.etaMin });
      if (j.state === "done") return;
    }
  }
  async usb(): Promise<UsbState> {
    const u = await this.call<UsbState>("GET", "/usb").catch(() => null);
    return u ?? { pending: false, present: false, bytes: 0, files: 0 };
  }
  async restoreUsb(action: "restore" | "home"): Promise<UsbResult> {
    await this.call("POST", "/usb", { action });
    for (;;) {
      await sleep(1000);
      const j = (await this.call<{ job: { state: string } }>("GET", "/usb")).job;
      if (j.state === "done" || j.state === "home" || j.state === "failed") return j.state as UsbResult;
    }
  }
  async finish() { await this.call("POST", "/finish"); }
}
