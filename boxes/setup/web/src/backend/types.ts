import type { Lang, Region, Keyboard, WinApp } from "../data";

/** Dysk, na którym można zainstalować Shrimp. Rozmiary w GB (10^9, jak na etykietach dysków). */
export interface Disk {
  /** Nazwa urządzenia, np. /dev/nvme0n1. */
  device: string;
  /** Nazwa dla ludzi, np. „Samsung SSD 980”. */
  name: string;
  sizeGB: number;
  /** Windows na tym dysku, jeśli jest. */
  windows?: {
    /** Partycja systemowa Windows (NTFS), którą zmniejszamy przy instalacji „obok”. */
    partition: string;
    /** Najmniejszy rozmiar Windows: zajęte miejsce + zapas. */
    minGB: number;
    /** Miejsce, które dzielimy suwakiem: partycja Windows + wolne miejsce na dysku. */
    poolGB: number;
    /** Partycja zaszyfrowana BitLockerem: nie da się jej zmniejszyć, więc nie ma „Obok Windows”. */
    bitlocker?: boolean;
  };
}

export type Place = "beside" | "replace" | "whole";

/** Podpowiedzi z Shrimp Install (shrimp-transfer/shrimp.json na nośniku, przez shrimp-setup --install). */
export interface Transfer {
  user?: { name?: string; fullName?: string };
  locale?: { language?: string; format?: string; region?: string; timezone?: string | null; keyboards?: string[] };
  /** Dysk z Windows zaszyfrowany BitLockerem. */
  bitlocker?: boolean;
  /** Pliki do przeniesienia (bajty): tyle miejsca więcej potrzebuje Shrimp. */
  bytes?: number;
  /** Język menu GRUB z nośnika (boot/grub2/shrimp-lang.cfg), też z Windows; jest nawet bez shrimp.json. */
  grubLang?: string;
}

/** Wszystko, co zebrały kroki 1–7. */
export interface InstallPlan {
  locale: string;
  timezone: string;
  keymap: string;
  disk: Disk;
  place: Place;
  /** Rozmiar Windows po zmniejszeniu (tylko „beside”). */
  windowsGB?: number;
  /** Hasło szyfrowania dysku (LUKS2) albo brak. */
  diskPassword?: string;
  fullName: string;
  userName: string;
  password: string;
}

export interface Progress {
  /** 0–100. */
  percent: number;
  phase: string;
  /** Pozostały czas w minutach, jeśli da się go oszacować. */
  etaMin?: number;
}

export class InstallError extends Error {
  constructor(public key: "space" | "install" | "stall" | "agama" | "offline", msg?: string) { super(msg || key); }
}

/** Część 1: instalacja z nośnika, nad Agamą. */
export interface InstallBackend {
  /** Dane z Shrimp Install albo null, gdy nośnik przygotowano inaczej. */
  transfer(): Promise<Transfer | null>;
  languages(): Promise<Lang[]>;
  regions(): Promise<Region[]>;
  keyboards(): Promise<Keyboard[]>;
  disks(): Promise<Disk[]>;
  /** Zmienia język działającego instalatora i domyślne wartości kolejnych kroków. */
  setLanguage(locale: string): Promise<void>;
  /** Przełącza układ klawiatury działającego instalatora (pole „Sprawdź układ”). */
  setKeymap(keymap: string): Promise<void>;
  /** Agama czyta repozytoria jeszcze raz (po podłączeniu do internetu). Zwraca "" albo opis problemu, gdy dalej ich nie widzi. */
  reprobe(): Promise<string>;
  /** Sprawdza plan bez instalowania (np. czy starczy miejsca). */
  check(plan: InstallPlan): Promise<void>;
  install(plan: InstallPlan, onProgress: (p: Progress) => void): Promise<void>;
  /** Dalsze śledzenie instalacji, która już trwa (po „Czekaj dalej” albo po utracie kontaktu z instalatorem). */
  watch(onProgress: (p: Progress) => void): Promise<void>;
  reboot(): Promise<void>;
}

export interface Network { ssid: string; secure: boolean; strength: number }
/** Stan karty Wi-Fi (shrimp-setup /network/diag): dlaczego lista sieci jest pusta. */
export interface WifiDiag { devices: string[]; radio: string; blocked: string; hardware: string }
export interface NetworkAdvanced { ip: "auto" | "manual"; dns: "auto" | "cloudflare" | "google"; proxy: "none" | "auto" }

/** Zawartość pendrive'a z Shrimp Install, czekająca na przywrócenie. */
export interface UsbState {
  pending: boolean;
  /** Pendrive z kopii jest teraz podłączony. */
  present: boolean;
  bytes: number;
  files: number;
}
/** "done": pendrive przywrócony; "home": pliki w folderze Pendrive; "failed": kopia została na dysku komputera. */
export type UsbResult = "done" | "home" | "failed";

export interface FirstBootInfo {
  fullName: string;
  /** Instalacja obok Windows (dodatkowe slajdy). */
  beside: boolean;
  /** Komputer ma już internet po kablu: krok Sieć jest pomijany. */
  wired: boolean;
  /** Nazwa wykrytej karty NVIDIA albo null. */
  nvidia: string | null;
  /** Lista z Shrimp Install, jeśli istnieje (maks. 5 pozycji pokazujemy). */
  windowsApps: WinApp[] | null;
}

export interface DriverChoice {
  /** Kodeki i sterowniki („Zainstaluj (Zalecane)”). */
  install: boolean;
  /** Tylko z kartą NVIDIA. */
  nvidia?: "nvidia" | "open";
}

export class WifiError extends Error {}
export class OfflineError extends Error {}

/** Część 2: pierwsza konfiguracja po restarcie, backend Rust (shrimp-setup). */
export interface FirstBootBackend {
  info(): Promise<FirstBootInfo>;
  /** Jest internet (na nośniku: czy Agama może pobrać system). */
  online(): Promise<boolean>;
  networks(): Promise<Network[]>;
  wifiDiag(): Promise<WifiDiag>;
  connect(ssid: string, password?: string): Promise<void>;
  connectHidden(ssid: string, security: "none" | "wpa2" | "wpa3", password?: string): Promise<void>;
  setNetworkAdvanced(a: NetworkAdvanced): Promise<void>;
  setAppearance(scheme: "default" | "prefer-dark", accent: string | null): Promise<void>;
  /** Układ klawiatury w bieżącej sesji GNOME (Agama ustawia go tylko dla instalowanego systemu i konsoli). */
  setInputSource(keymap: string): Promise<void>;
  setDrivers(c: DriverChoice): Promise<void>;
  installApps(ids: string[], onProgress: (p: Progress) => void): Promise<void>;
  usb(): Promise<UsbState>;
  /** "restore": formatuje pendrive i wgrywa pliki; "home": zostawia je w folderze Pendrive. */
  restoreUsb(action: "restore" | "home"): Promise<UsbResult>;
  /** Zapisuje, że konfiguracja się skończyła; wykonuje odłożone zmiany systemu (sterowniki) i restart, jeśli trzeba. */
  finish(): Promise<void>;
}

export interface Backends {
  /** "install": nośnik (kroki 1–9), "firstboot": po restarcie (kroki 10–15), "demo": wszystko na danych z makiety. */
  mode: "install" | "firstboot" | "demo";
  install: InstallBackend;
  firstboot: FirstBootBackend;
}
