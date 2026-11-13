// Builds src/assets/icons.svg (a <symbol> sprite) from the system Adwaita icon theme.
// Run on a machine with adwaita-icon-theme installed: `npm run icons [theme-dir]`.
// DESIGN.md › Ikony: only real Adwaita symbolic icons, never hand-drawn stand-ins.
import { readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";

const theme = process.argv[2] || "/usr/share/icons/Adwaita/symbolic";
const ICONS = [
  "go-previous-symbolic",
  "go-next-symbolic",
  "object-select-symbolic",
  "network-wireless-encrypted-symbolic",
  "help-about-symbolic",
  "dialog-warning-symbolic",
  "view-reveal-symbolic",
  "view-conceal-symbolic",
  // onboarding: duże znaki po lewej stronie (sieć, pliki, pendrive, przeglądarka, programy, wygląd)
  "network-wireless-signal-excellent-symbolic",
  "network-wired-symbolic",
  "folder-symbolic",
  "folder-documents-symbolic",
  "folder-pictures-symbolic",
  "folder-music-symbolic",
  "folder-videos-symbolic",
  "drive-removable-media-symbolic",
  "web-browser-symbolic",
  "view-app-grid-symbolic",
  "applications-games-symbolic",
  "x-office-document-symbolic",
  "preferences-desktop-appearance-symbolic",
  "emblem-ok-symbolic",
  // krok „Wygląd”: jasny, ciemny, światło nocne; aktualizacje w tle
  "weather-clear-symbolic",
  "weather-clear-night-symbolic",
  "software-update-available-symbolic",
];

const out = ['<svg xmlns="http://www.w3.org/2000/svg">'];
for (const name of ICONS) {
  const file = execSync(`find ${theme} -name ${name}.svg | head -1`).toString().trim();
  if (!file) throw new Error(`missing icon ${name} in ${theme}`);
  const src = readFileSync(file, "utf8");
  const viewBox = (src.match(/viewBox="([^"]+)"/) || [, "0 0 16 16"])[1];
  const inner = src
    .replace(/<\?xml[^>]*>/, "")
    .replace(/^[\s\S]*?<svg[^>]*>/, "")
    .replace(/<\/svg>\s*$/, "")
    // Symbolic icons are recoloured by GTK; in the browser they follow currentColor.
    .replace(/fill="#[0-9a-fA-F]{3,8}"/g, 'fill="currentColor"')
    .replace(/fill:#[0-9a-fA-F]{3,8}/g, "fill:currentColor")
    // color="#000" na grupie przykrywa currentColor (web-browser-symbolic był zawsze czarny).
    .replace(/\scolor="#[0-9a-fA-F]{3,8}"/g, "")
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/\s+/g, " ")
    .trim();
  out.push(`<symbol id="${name}" viewBox="${viewBox}">${inner}</symbol>`);
}
out.push("</svg>");
writeFileSync(new URL("../src/assets/icons.svg", import.meta.url), out.join("\n") + "\n");
console.log(`icons.svg: ${ICONS.length} icons from ${theme}`);
