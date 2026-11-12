// Dane z makiety: podgląd w przeglądarce (npm run dev) i zrzuty ekranów bez Agamy.
// ?fail=install|net|wifi|offline wymusza błąd (offline: nośnik bez internetu, krok „Połącz z internetem”), ?nvidia=0 ukrywa kartę NVIDIA, ?wired=1 pomija Sieć,
// ?bitlocker=1 udaje dysk z BitLockerem (dane z Shrimp Install), ?slowdisks=1 odczyt dysków trwa 2,5 s (Agama sonduje),
// ?repo=1 (z ?fail=offline): po połączeniu z Wi-Fi Agama dalej nie czyta repozytoriów.
import { LANGS, REGIONS, KEYBOARDS } from "../data";
import { mapWindowsApps } from "../logic";
import { InstallError, OfflineError, WifiError, type Disk, type FirstBootBackend, type InstallBackend, type InstallPlan, type Network, type Progress } from "./types";

const q = new URLSearchParams(location.search);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const fast = q.has("fast");

const DISKS: Disk[] = [
  { device: "/dev/nvme0n1", name: "Samsung SSD 980", sizeGB: 476, windows: { partition: "/dev/nvme0n1p3", minGB: 160, poolGB: 476 } },
  { device: "/dev/sda", name: "WD Blue", sizeGB: 931 },
];
const NETS: Network[] = [{ ssid: "Dom 5G", secure: true, strength: 80 }, { ssid: "FRITZ!Box 7590", secure: true, strength: 60 }, { ssid: "Kawiarnia", secure: false, strength: 40 }];

async function simulate(onProgress: (p: Progress) => void, phases: [number, string][], minutes: number, failAt?: number) {
  for (let p = 0; p <= 100; p += 1.5) {
    if (failAt && p >= failAt) throw new Error("demo");
    const phase = phases.find(([lim]) => p < lim)?.[1] || "done";
    onProgress({ percent: p, phase, etaMin: Math.ceil(((100 - p) / 100) * minutes) });
    await sleep(fast ? 5 : 200);
  }
  onProgress({ percent: 100, phase: "done" });
}

/** Nazwy z listy programów Windows, tak jak zapisuje je Shrimp Install. */
const WIN_NAMES = ["Microsoft 365 Apps for business - pl-pl", "Adobe Photoshop 2024", "Google Chrome", "Spotify", "Steam", "Discord", "Zoom Workplace", "7-Zip 24.08 (x64)"];

export const demoInstall: InstallBackend = {
  async transfer() { return q.get("bitlocker") === "1" ? { bitlocker: true } : null; },
  async languages() { return LANGS; },
  async regions() { return REGIONS; },
  async keyboards() { return KEYBOARDS; },
  async disks() { if (q.has("slowdisks")) await sleep(2500); return DISKS; },
  async setLanguage() {},
  async setKeymap() {},
  async reprobe() { return q.get("repo") === "1" ? "Could not read the repositories" : ""; },
  async check(plan: InstallPlan) {
    if (plan.place === "beside" && plan.disk.windows && plan.disk.windows.poolGB - (plan.windowsGB || 0) < 40) throw new InstallError("space");
  },
  async install(_plan, onProgress) {
    try {
      await simulate(onProgress, [[28, "inst.disk"], [80, "inst.system"], [100, "inst.files"]], 11, ["install", "stall", "agama"].includes(q.get("fail") || "") ? 41 : undefined);
    } catch { throw new InstallError(q.get("fail") === "stall" ? "stall" : q.get("fail") === "agama" ? "agama" : "install"); }
  },
  async watch(onProgress) {
    await simulate(onProgress, [[80, "inst.system"], [100, "inst.files"]], 4);
  },
  async reboot() {},
};

let demoOnline = q.get("fail") !== "offline";
export const demoFirstBoot: FirstBootBackend = {
  async online() { return demoOnline; },
  async info() {
    return {
      fullName: "Anna Kowalska",
      beside: true,
      wired: q.get("wired") === "1",
      nvidia: q.get("nvidia") === "0" ? null : "NVIDIA GeForce RTX 3060",
      windowsApps: mapWindowsApps(WIN_NAMES),
    };
  },
  async networks() { if (q.get("nowifi")) return []; return NETS; },
  async wifiDiag() { return q.get("nowifi") === "dev" ? { devices: [], radio: "enabled", blocked: "", hardware: "Intel Wi-Fi 6 AX201 [brak sterownika]" } : q.get("nowifi") === "off" ? { devices: ["wlan0:unavailable"], radio: "disabled", blocked: "soft", hardware: "" } : { devices: ["wlan0:disconnected"], radio: "enabled", blocked: "", hardware: "" }; },
  async connect(_ssid, password) { await sleep(300); if (q.get("fail") === "wifi" || (password !== undefined && password.length < 8)) throw new WifiError(); demoOnline = true; },
  async connectHidden() { await sleep(300); demoOnline = true; },
  async setNetworkAdvanced() {},
  async setAppearance() {},
  async setInputSource() {},
  async setDrivers() {},
  async installApps(_ids, onProgress) {
    try {
      await simulate(onProgress, [[100, "apps.phase"]], 2, q.get("fail") === "net" ? 12 : undefined);
    } catch { throw new OfflineError(); }
  },
  async usb() { return q.get("usb") === "0" ? { pending: false, present: false, bytes: 0, files: 0 } : { pending: true, present: q.get("usb") !== "plug", bytes: 7.4e9, files: 1280 }; },
  async restoreUsb(action) { await sleep(fast ? 50 : 3000); return q.get("fail") === "usb" ? "home" : action === "home" ? "home" : "done"; },
  async finish() {},
};
