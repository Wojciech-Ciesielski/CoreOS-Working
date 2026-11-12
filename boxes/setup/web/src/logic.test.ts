import { describe, expect, it } from "vitest";
import { slug, firstName, proposals, windowsList, mapWindowsApps, klidToXkb, prefill, loginFrom, isReservedUser, installLang } from "./logic";
import example from "../../../install-schema/shrimp.example.json";
import type { Transfer } from "./backend/types";
import { LANGS, REGIONS, KEYBOARDS, WIN_CATALOG } from "./data";
import { blockingIssues, configFromPlan, disksFromSystem, phaseOf, progressKey, PROBE_SOFTWARE, type AStatus } from "./backend/agama";
import type { InstallPlan } from "./backend/types";
import { createI18n } from "../../../i18n-core/core";
import keys from "../../i18n/_keys.tsv?raw";
import plTxt from "../../i18n/pl.txt?raw";

const pl = createI18n(keys, { "pl.txt": plTxt }, ["pl"]);

describe("konto", () => {
  it("tworzy nazwę użytkownika bez polskich znaków", () => {
    expect(slug("Łukasz")).toBe("lukasz");
    expect(slug("Żaneta-Ćwik")).toBe("zanetacwik");
    expect(firstName("  Anna Kowalska ")).toBe("Anna");
  });
});

describe("programy", () => {
  const win = mapWindowsApps(["Spotify", "Microsoft 365 Apps for business - pl-pl", "Microsoft Office Proofing Tools 2016 - Polski", "Nieznany program"]);
  it("mapuje programy Windows na odpowiedniki i pomija nieznane", () => {
    expect(win.map((w) => w.id)).toEqual(["com.spotify.Client", "org.libreoffice.LibreOffice"]);
    expect(windowsList(win)[1].label).toBe('Microsoft Office<span class="arr">→</span>LibreOffice');
    expect(windowsList(win)[0].label).toBe("Spotify");
  });
  it("rozpoznaje nazwy z wersją, pakiety Store i nie powtarza odpowiedników", () => {
    const w = mapWindowsApps(["Adobe Photoshop 2024", "Adobe Photoshop Lightroom Classic", "Zoom Workplace", "Epic Games Launcher", "GOG GALAXY", "Google Chrome"], ["SpotifyAB.SpotifyMusic", "Microsoft.WindowsCalculator"]);
    expect(w.map((x) => x.windows)).toEqual(["Google Chrome", "Spotify", "Zoom", "Epic Games Launcher", "Adobe Photoshop", "Adobe Lightroom"]);
    expect(mapWindowsApps(["Microsoft Office File Validation Add-In"])).toEqual([]);
  });
  it("proponuje maks. 5 programów na zmianę z kategorii, bez tych z Windows", () => {
    const p = proposals(pl, [true, false, false, false, true], win).map((x) => x.id);
    expect(p).toEqual(["org.videolan.VLC", "org.mozilla.thunderbird"]);
    expect(proposals(pl, [true, true, true, true, true], null)).toHaveLength(5);
  });
});

describe("katalog programów Windows", () => {
  it("ma co najmniej 500 programów, bez powtórzonych nazw", () => {
    const real = WIN_CATALOG.filter((a) => !a.skip);
    expect(real.length).toBeGreaterThanOrEqual(500);
    const names = WIN_CATALOG.map((a) => a.windows.toLowerCase());
    expect(new Set(names).size).toBe(names.length);
  });
  it("identyfikatory Flathub mają poprawną postać, a wpisy są spójne", () => {
    for (const a of WIN_CATALOG) {
      if (a.linux) {
        expect(a.linux.id, a.windows).toMatch(/^[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+){2,}$/);
        expect(a.skip, a.windows).toBeUndefined();
      }
      if (a.base) expect(a.linux, a.windows).not.toBeNull();
    }
  });
  it("wzorce nie łapią przypadkowych nazw, a większość pasuje do własnej nazwy", () => {
    for (const a of WIN_CATALOG) {
      expect(a.re.test(""), a.windows).toBe(false);
      expect(a.re.test("Nieznany program 1.0"), a.windows).toBe(false);
    }
    const own = WIN_CATALOG.filter((a) => a.re.test(a.windows)).length;
    expect(own / WIN_CATALOG.length).toBeGreaterThan(0.9);
  });
  it("ma dokładnie 100 najpopularniejszych programów, z miejscami 1–100 i odpowiednikiem w Linuksie", () => {
    const top = WIN_CATALOG.filter((a) => a.top !== undefined);
    expect(top.map((a) => a.top).sort((a, b) => a! - b!)).toEqual(Array.from({ length: 100 }, (_, i) => i + 1));
    for (const a of top) {
      expect(a.linux, a.windows).not.toBeNull();
      expect(a.base, a.windows).toBeUndefined();
      expect(a.skip, a.windows).toBeUndefined();
    }
  });
  it("podpowiedzi zaczynają od rankingu, niezależnie od kolejności programów w Windows", () => {
    const got = mapWindowsApps(["Zotero", "Discord", "Google Chrome", "Spotify"]).map((a) => a.windows);
    expect(got.slice(0, 3)).toEqual(["Google Chrome", "Spotify", "Discord"]);
  });
  it("pomija sterowniki, składniki systemu i odpowiedniki już zainstalowane", () => {
    expect(mapWindowsApps(["NVIDIA Graphics Driver 560.94", "Microsoft Visual C++ 2015-2022 Redistributable (x64)", "Avast Free Antivirus", "7-Zip 23.01"])).toEqual([]);
  });
});

describe("shrimp install", () => {
  const known = KEYBOARDS.map((k) => k.keymap);
  it("tłumaczy układ klawiatury Windows na xkb", () => {
    expect(klidToXkb("00000415", known)).toBe("pl");
    expect(klidToXkb("00010415", known)).toBe("pl(qwertz)");
    expect(klidToXkb("0000040C", known)).toBe("fr");
    expect(klidToXkb("00000816", known)).toBeNull();
    expect(klidToXkb("E0010411", known)).toBeNull();
  });
  it("podpowiada język, strefę, klawiaturę i konto", () => {
    const p = prefill({ user: { name: "Jan", fullName: "Jan Kowalski" }, locale: { language: "de-DE", timezone: "Europe/Kiev", keyboards: ["E0010411", "00000407"] } }, LANGS, REGIONS, KEYBOARDS);
    expect(LANGS[p.lang].locale).toBe("de_DE.UTF-8");
    expect(REGIONS[p.region].tz).toBe("Europe/Kyiv");
    expect(KEYBOARDS[p.kb].keymap).toBe("de");
    expect([p.full, p.user]).toEqual(["Jan Kowalski", "jan"]);
  });
  it("bez strefy bierze domyślną dla języka; nieznany język zostawia domyślne", () => {
    const p = prefill({ locale: { language: "pl-PL", timezone: null }, user: { fullName: "Łucja Nowak" } }, LANGS, REGIONS, KEYBOARDS);
    expect(REGIONS[p.region].tz).toBe("Europe/Warsaw");
    expect(p.user).toBe("lucja");
    const q = prefill({ locale: { language: "xx-XX" } }, LANGS, REGIONS, KEYBOARDS);
    expect([q.lang, q.region, q.kb]).toEqual([-1, -1, -1]);
  });
});

const base: InstallPlan = {
  locale: "pl_PL.UTF-8", timezone: "Europe/Warsaw", keymap: "pl", place: "beside", windowsGB: 200,
  disk: { device: "/dev/nvme0n1", name: "SSD", sizeGB: 476, windows: { partition: "/dev/nvme0n1p3", minGB: 80, poolGB: 470 } },
  fullName: "Anna Kowalska", userName: "anna", password: "x",
};

describe("agama", () => {
  it("obok Windows: zmniejsza partycję Windows i generuje partycje Shrimp", () => {
    const c = configFromPlan(base) as any;
    expect(c.product).toEqual({ id: "Shrimp", mode: "immutable" });
    expect(c.storage.drives[0].partitions).toEqual([{ search: "/dev/nvme0n1p3", size: "200 GB" }, { generate: { partitions: "default" } }]);
  });
  it("zamiast Windows z szyfrowaniem: usuwa wszystko i szyfruje LUKS2", () => {
    const c = configFromPlan({ ...base, place: "replace", diskPassword: "tajne" }) as any;
    expect(c.storage.drives[0].partitions[0]).toEqual({ search: "*", delete: true });
    expect(c.storage.drives[0].partitions[1].generate.encryption).toEqual({ luks2: { password: "tajne" } });
  });
  it("odczytuje dysk z Windows i minimum z informacji o zmniejszaniu", () => {
    const d = disksFromSystem({
      storage: {
        candidateDrives: [1],
        devices: [{
          sid: 1, name: "/dev/sda", block: { size: 500e9 }, drive: { vendor: "WD", model: "Blue" },
          partitionTable: { unusedSlots: [{ start: 0, size: 10e9 }] },
          partitions: [
            { sid: 2, name: "/dev/sda1", block: { size: 1e8 }, filesystem: { type: "vfat" } },
            { sid: 3, name: "/dev/sda3", block: { size: 480e9, systems: ["Windows 11"], shrinking: { supported: true, minSize: 120.4e9 } }, filesystem: { type: "ntfs" } },
          ],
        }],
      },
    });
    expect(d).toEqual([{ device: "/dev/sda", name: "WD Blue", sizeGB: 500, windows: { partition: "/dev/sda3", minGB: 141, poolGB: 490 } }]);
  });
  it("rozpoznaje Windows zaszyfrowany BitLockerem", () => {
    const d = disksFromSystem({
      storage: {
        devices: [{
          sid: 1, name: "/dev/nvme0n1", block: { size: 256e9 },
          partitions: [{ sid: 3, name: "/dev/nvme0n1p3", block: { size: 250e9 }, filesystem: { type: "bitlocker" } }],
        }],
      },
    });
    expect(d[0].windows).toEqual({ partition: "/dev/nvme0n1p3", minGB: 270, poolGB: 250, bitlocker: true });
  });
  it("Vivobook: BitLocker jako szyfrowanie, pendrive z Shrimp pierwszy na liście Agamy", () => {
    // Układ z testu na ASUS Vivobook X513EA (2026-11-09): Agama wymienia najpierw pendrive 32 GB z Shrimp Install,
    // a partycja Windows z BitLockerem nie ma systemu plików, tylko block.encrypted i typ danych Windows.
    const d = disksFromSystem({
      storage: {
        candidateDrives: [10, 1],
        devices: [
          { sid: 10, name: "/dev/sda", block: { size: 32e9 }, drive: { vendor: "SanDisk", model: "Ultra" },
            partitions: [{ sid: 11, name: "/dev/sda1", block: { size: 32e9 }, filesystem: { type: "vfat", label: "SHRIMP_LIVE" } }] },
          { sid: 1, name: "/dev/nvme0n1", block: { size: 512e9 }, drive: { model: "INTEL SSDPEKNW512G8" },
            partitionTable: { unusedSlots: [] },
            partitions: [
              { sid: 2, name: "/dev/nvme0n1p1", block: { size: 1e8 }, filesystem: { type: "vfat" }, partition: { efi: true, id: "esp" } },
              { sid: 3, name: "/dev/nvme0n1p3", description: "Encrypted Partition", block: { size: 510e9, encrypted: true, shrinking: { supported: false } }, partition: { efi: false, id: "windows_basic_data" } },
            ] },
        ],
      },
    });
    expect(d).toEqual([{ device: "/dev/nvme0n1", name: "INTEL SSDPEKNW512G8", sizeGB: 512, windows: { partition: "/dev/nvme0n1p3", minGB: 530, poolGB: 510, bitlocker: true } }]);
  });
  it("bez Windows: największy dysk pierwszy, zaszyfrowany Linux to nie BitLocker", () => {
    const d = disksFromSystem({
      storage: {
        devices: [
          { sid: 1, name: "/dev/sda", block: { size: 128e9 }, partitions: [{ sid: 2, name: "/dev/sda2", block: { size: 120e9, encrypted: true }, partition: { efi: false, id: "linux" } }] },
          { sid: 5, name: "/dev/nvme0n1", block: { size: 1000e9 } },
        ],
      },
    });
    expect(d.map((x) => [x.device, !!x.windows])).toEqual([["/dev/nvme0n1", false], ["/dev/sda", false]]);
  });
  it("tłumaczy postęp Agamy na fazy", () => {
    expect(phaseOf({ stage: "installing", progresses: [{ scope: "manager", index: 2, size: 4, step: "" }, { scope: "software", index: 5, size: 10, step: "" }] }))
      .toEqual({ percent: 37.5, phase: "inst.system" });
  });
});

describe("domyślny język instalacji", () => {
  const idx = (t: any) => LANGS[installLang(t, LANGS)].locale;
  it("z Windows (shrimp.json), potem z menu GRUB, na końcu angielski, nie polski z sesji live", () => {
    expect(idx({ locale: { language: "de-DE" }, grubLang: "fr" })).toBe("de_DE.UTF-8");
    expect(idx({ grubLang: "fr" })).toBe("fr_FR.UTF-8");
    expect(idx(null)).toBe("en_US.UTF-8");
    expect(idx({ locale: { language: "xx-YY" } })).toBe("en_US.UTF-8");
    expect(idx({ locale: { language: "pl-PL" } })).toBe("pl_PL.UTF-8");
  });
});

describe("problemy Agamy", () => {
  it("brak metadanych repozytorium aktualizacji nie blokuje instalacji", () => {
    const upd = { scope: "software", class: "loadSource", description: "Could not read the repositories",
      details: "[agama-1|https://download.opensuse.org/update/leap/16.1/oss/x86_64] Failed to retrieve new repository metadata." };
    const dist = { ...upd, details: "[agama-0|https://download.opensuse.org/distribution/leap/16.1/repo/oss/x86_64] Failed to retrieve new repository metadata." };
    const disk = { scope: "storage", class: "proposal", description: "Not enough space" };
    expect(blockingIssues([upd])).toEqual([]);
    expect(blockingIssues([upd, dist, disk])).toEqual([dist, disk]);
  });
});

describe("nazwa konta: przypadki brzegowe", () => {
  it("nie zaczyna się cyfrą i ma najwyżej 32 znaki", () => {
    expect(slug("2pac")).toBe("pac");
    expect(slug("x".repeat(40)).length).toBe(32);
  });
  it("imię bez liter łacińskich daje „user”, puste daje pusto", () => {
    expect(loginFrom("李明")).toBe("user");
    expect(loginFrom("  ")).toBe("");
  });
  it("nazwy zajęte przez system dostają cyfrę", () => {
    expect(loginFrom("Admin Kowalski")).toBe("admin1");
    expect(loginFrom("", "root")).toBe("root1");
    expect(isReservedUser("gdm")).toBe(true);
    expect(isReservedUser("anna")).toBe(false);
  });
});

describe("kontrakt Shrimp Install ↔ Setup (boxes/install-schema)", () => {
  // Strona dostaje to, co shrimp-setup --install wycina z shrimp.json (transfer.rs › hint).
  const hint: Transfer = { user: example.user, locale: example.locale, bitlocker: example.windows.bitlocker };
  it("podpowiedzi z przykładu trafiają w język, strefę, klawiaturę i konto", () => {
    const p = prefill(hint, LANGS, REGIONS, KEYBOARDS);
    expect(LANGS[p.lang].locale.startsWith("pl_PL")).toBe(true);
    expect(REGIONS[p.region].tz).toBe("Europe/Warsaw");
    expect(KEYBOARDS[p.kb].keymap).toBe("pl");
    expect(p.full).toBe("Jan Kowalski");
    expect(p.user).toBe("jankowalski");
  });
  it("programy z przykładu mają odpowiedniki", () => {
    const apps = mapWindowsApps(example.apps.map((a) => a.name!).filter(Boolean), example.storeApps);
    expect(apps.length).toBeGreaterThan(0);
  });
});

describe("śledzenie instalacji", () => {
  const st = (index: number, step = "x"): AStatus => ({ stage: "installing", progresses: [{ scope: "software", index, size: 10, step }] });
  it("odcisk postępu zmienia się z każdym krokiem i tylko wtedy", () => {
    expect(progressKey(st(1))).toBe(progressKey(st(1)));
    expect(progressKey(st(1))).not.toBe(progressKey(st(2)));
    expect(progressKey(st(1, "a"))).not.toBe(progressKey(st(1, "b")));
  });
});

describe("ponowne czytanie repozytoriów (Agama 24)", () => {
  // Agama: enum Action { #[serde(rename = "probe")] Probe { only: Option<Vec<Scope>> }, … }.
  // {"probe":null} dawało HTTP 422 („expected struct variant Action::Probe”) i Setup pokazywał
  // „Brak połączenia z internetem” po udanym połączeniu z Wi-Fi.
  it("akcja probe to wariant ze strukturą, tylko repozytoria", () => {
    expect(PROBE_SOFTWARE).toEqual({ probe: { only: ["software"] } });
    expect(JSON.stringify(PROBE_SOFTWARE)).not.toContain("null");
  });
});
