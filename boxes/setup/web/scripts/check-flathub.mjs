// Sprawdza, czy wszystkie identyfikatory Flathub z katalogu (windows-apps.json) i z listy APPS istnieją.
// Uruchamiane w CI: node scripts/check-flathub.mjs. Bez dostępu do sieci kończy się ostrzeżeniem, nie błędem.
import { readFileSync } from "node:fs";

const catalog = JSON.parse(readFileSync(new URL("../src/windows-apps.json", import.meta.url), "utf8"));
const data = readFileSync(new URL("../src/data.ts", import.meta.url), "utf8");
const ids = new Map();
for (const a of catalog.apps) if (a.linux) ids.set(a.linux.id, a.windows);
for (const m of data.matchAll(/id:\s*"([A-Za-z0-9_.-]+\.[A-Za-z0-9_.-]+\.[A-Za-z0-9_.-]+)"/g)) ids.set(m[1], "data.ts");

let known;
try {
  const r = await fetch("https://flathub.org/api/v2/appstream", { headers: { "User-Agent": "shrimp-ci" } });
  if (!r.ok) throw new Error("HTTP " + r.status);
  known = new Set(await r.json());
} catch (e) {
  console.log(`::warning::Nie można pobrać listy Flathub (${e.message}), pomijam sprawdzenie.`);
  process.exit(0);
}
const missing = [...ids].filter(([id]) => !known.has(id));
console.log(`Sprawdzono ${ids.size} identyfikatorów, brakuje ${missing.length}.`);
for (const [id, who] of missing) {
  const word = id.split(".").pop().toLowerCase().replace(/[^a-z0-9]/g, "");
  const near = [...known].filter((k) => k.toLowerCase().replace(/[^a-z0-9.]/g, "").includes(word)).slice(0, 5);
  console.log(`::error::Brak we Flathub: ${id} (${who})${near.length ? "; podobne: " + near.join(", ") : ""}`);
}
process.exit(missing.length ? 1 : 0);
