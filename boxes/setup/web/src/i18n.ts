// Język Setup: tłumaczenia w boxes/setup/i18n, ładowane na żądanie (jeden plik na język), bo języków jest ponad sto.
import { build, negotiate, textFallback, type I18n } from "../../../i18n-core/core";
import keys from "../../i18n/_keys.tsv?raw";

const files = import.meta.glob("../../i18n/*.txt", { query: "?raw", import: "default" }) as Record<string, () => Promise<string>>;
const byTag = new Map(Object.entries(files).map(([path, load]) => [/([^/]+)\.txt$/.exec(path)![1], load]));

/** Tłumacz dla pierwszego z żądanych języków, który mamy (bez dopasowania: angielski; brakujące teksty z języka zapasowego). */
export async function load(requested: string[]): Promise<I18n> {
  const tag = negotiate(requested);
  const read = async (t?: string) => {
    const f = t ? byTag.get(t) : undefined;
    try { return f ? await f() : undefined; } catch { return undefined; /* brak pliku: angielski */ }
  };
  const [txt, fbTxt] = await Promise.all([read(tag), read(textFallback(tag))]);
  return build(keys, tag, txt, fbTxt);
}

/** Język startowy: ?lang= z adresu, potem języki przeglądarki (system nośnika). */
export function requestedLanguages(q: URLSearchParams): string[] {
  const forced = q.get("lang");
  return [...(forced ? [forced] : []), ...(navigator.languages || [])];
}
