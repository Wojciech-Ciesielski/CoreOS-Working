import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "./style.css";
import { start } from "./app";
import { load, requestedLanguages } from "./i18n";
import { AgamaBackend } from "./backend/agama";
import { ShrimpFirstBoot } from "./backend/firstboot";
import { demoInstall, demoFirstBoot } from "./backend/demo";
import type { Backends } from "./backend/types";

// Tryb wybiera shrimp-setup przez adres: ?mode=install (nośnik) albo ?mode=firstboot (po restarcie).
// Bez parametru: podgląd na danych z makiety.
const mode = (new URLSearchParams(location.search).get("mode") || "demo") as Backends["mode"];
const be: Backends =
  // Na nośniku z „części 2” działa tylko sieć (lokalny serwer shrimp-setup, krok „Połącz z internetem”).
  mode === "install" ? { mode, install: new AgamaBackend(), firstboot: window.SHRIMP_LOCAL ? new ShrimpFirstBoot(window.SHRIMP_LOCAL.url, window.SHRIMP_LOCAL.key) : demoFirstBoot }
  : mode === "firstboot" ? { mode, install: demoInstall, firstboot: new ShrimpFirstBoot() }
  : { mode: "demo", install: demoInstall, firstboot: demoFirstBoot };

load(requestedLanguages(new URLSearchParams(location.search))).then((L) => start(document.getElementById("app")!, be, L));

// Pierwsze kliknięcie, dotknięcie albo stuknięcie touchpada w dzienniku (journalctl --user -b | grep shrimp-setup):
// po nim widać, że wejście dochodzi do okna Setup (Beta 3.2: kursor się ruszał, kliknięcia nie działały).
// Ten sam wpis sprawdza test QEMU ze strażnikiem Setup.
addEventListener("pointerdown", (e) => console.log(`shrimp-setup: pierwsze kliknięcie (${e.pointerType}) w ${mode}`), { once: true, capture: true });
