import { describe, expect, it } from "vitest";
import { createI18n, parseKeys, parseValues, placeholders } from "../../../i18n-core/core";
import { GNOME_LOCALES, LANGS as KNOWN } from "../../../i18n-core/langs";
import keys from "../../i18n/_keys.tsv?raw";
import appSrc from "./app.ts?raw";
import {
  LANGS, REGIONS, KEYBOARDS, ENCS, THEMES, DRV, NV, IPS, ACCENTS, PLACES, USES, APPS, ERR, SLIDES, defaultKeymap, defaultTz,
} from "./data";
import { keyboardName, regionLabel, slug, zoneName } from "./logic";

const files = import.meta.glob("../../i18n/*.txt", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const en = createI18n(keys, files, ["en"]);
const pl = createI18n(keys, files, ["pl-PL"]);
const entries = parseKeys(keys);
const ids = new Set(entries.map((e) => e.key));

describe("tłumaczenia Setup", () => {
  it("każdy używany klucz istnieje", () => {
    const used = new Set<string>();
    for (const m of appSrc.matchAll(/\btr\(\s*"([\w.]+)"/g)) used.add(m[1]);
    const data = [...ENCS, ...THEMES, ...DRV, ...NV, ...IPS, ...ACCENTS.map((a) => a[0]), ...PLACES.flatMap((p) => [p.n, p.d]), ...USES,
      ...APPS.flat().map((a) => a.what), ...Object.values(ERR).flatMap((e) => [e.t, e.d, e.p, e.s || ""]),
      ...SLIDES.flatMap((s) => [s.t, s.d, s.extra || ""]), ...REGIONS.map((r) => (r.city ? "city." + r.city : "")), ...KEYBOARDS.map((k) => k.note || "")];
    for (const k of data) if (k && /^[a-z]+(\.[a-z0-9]+)+$|^[a-z]+$/.test(k) && !["auto", "none", "skip"].includes(k) || ["auto", "none", "skip"].includes(k)) used.add(k);
    for (const k of used) expect(ids.has(k), k).toBe(true);
    for (const k of ["WPA2", "Cloudflare"]) expect(ids.has(k)).toBe(false);
  });
  it("polski ma wszystkie klucze, a parametry się zgadzają", () => {
    const pv = parseValues(files["../../i18n/pl.txt"]);
    for (const e of entries) {
      expect(pv.has(e.id), e.key).toBe(true);
      expect(placeholders(pv.get(e.id)!), e.key).toEqual(placeholders(e.en));
    }
  });
  it("wszystkie pliki mają znane id i te same parametry", () => {
    const byId = new Map(entries.map((e) => [e.id, e]));
    for (const [path, txt] of Object.entries(files)) {
      for (const [id, v] of parseValues(txt)) {
        const e = byId.get(id);
        expect(e, `${path} ${id}`).toBeTruthy();
        expect(placeholders(v), `${path} ${e!.key}`).toEqual(placeholders(e!.en));
        expect(v, `${path} ${e!.key}`).not.toMatch(/<[a-z/]/i);
      }
    }
  });
  it("lista języków obejmuje wszystkie języki GNOME", () => {
    expect(LANGS.length).toBe(GNOME_LOCALES.length);
    for (const [, tag] of GNOME_LOCALES) expect(KNOWN.some((k) => k.tag === tag), tag).toBe(true);
    expect(new Set(LANGS.map((l) => l.locale)).size).toBe(LANGS.length);
  });
  it("język, strefa i klawiatura wynikają z locale", () => {
    expect(defaultTz("de_DE.UTF-8")).toBe("Europe/Berlin");
    expect(defaultTz("ja_JP.UTF-8")).toBe("Asia/Tokyo");
    expect(defaultKeymap("pt_BR.UTF-8")).toBe("br");
    expect(defaultKeymap("pl_PL.UTF-8")).toBe("pl");
    expect(defaultKeymap("xx_XX.UTF-8")).toBe("us");
  });
  it("nazwy krajów, stref i układów liczy Intl w języku interfejsu", () => {
    expect(regionLabel(pl, REGIONS.find((r) => r.tz === "Europe/Warsaw")!)).toBe("Polska");
    expect(regionLabel(pl, REGIONS.find((r) => r.tz === "America/New_York")!)).toBe("Stany Zjednoczone · Nowy Jork");
    expect(regionLabel(en, REGIONS.find((r) => r.tz === "Europe/Berlin")!)).toBe("Germany");
    expect(zoneName(en, "Europe/Warsaw")).toMatch(/Central European/);
    expect(keyboardName(pl, KEYBOARDS.find((k) => k.keymap === "pl(qwertz)")!)).toBe("Polski (maszynistki)");
    expect(keyboardName(en, KEYBOARDS.find((k) => k.keymap === "gb")!)).toBe("English (United Kingdom)");
  });
  it("nazwa użytkownika bez znaków spoza ASCII", () => {
    expect(slug("Søren Æble")).toBe("sorenaeble");
    expect(slug("Müller")).toBe("muller");
  });
});
