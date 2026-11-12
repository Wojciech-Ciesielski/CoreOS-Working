#!/bin/bash
# Which desktop applications the openSUSE Leap GNOME patterns bring in.
# Runs as root in an openSUSE container (see .github/workflows/gnome-apps.yml).
#   tools/gnome-apps-inventory.sh LEAP_VERSION OUTDIR
set -uo pipefail
V=${1:-16.1}; OUT=${2:-out/gnome-apps}
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
base="https://download.opensuse.org/distribution/leap/$V/repo/oss"
REPO=""
for c in "$base/x86_64" "$base"; do
  curl -fsSLI "$c/repodata/repomd.xml" >/dev/null 2>&1 && { REPO=$c; break; }
done
[ -n "$REPO" ] || { echo "no Leap $V repository" >&2; exit 1; }
R=/var/tmp/gnome-root
Z=(zypper -n --root "$R" --gpg-auto-import-keys)
rm -rf "$R"; mkdir -p "$R"
"${Z[@]}" ar -f "$REPO" leap >/dev/null
"${Z[@]}" ref >/dev/null

newpkgs() { # package names a dry run would install
  "${Z[@]}" in --dry-run --recommends --auto-agree-with-licenses "$@" 2>&1 \
    | awk '/NEW package/ {f=1; next} f && /^$/ {f=0} f' | tr ' ' '\n' | sed '/^$/d' | sort -u
}
newpkgs pattern:gnome_basic > "$OUT/pattern-gnome_basic.txt"
newpkgs pattern:immutable_base pattern:gnome > "$OUT/pattern-gnome.txt"
comm -13 "$OUT/pattern-gnome_basic.txt" "$OUT/pattern-gnome.txt" > "$OUT/gnome-minus-basic.txt"
"${Z[@]}" info -t pattern gnome gnome_basic > "$OUT/pattern-info.txt" 2>&1

# Install the full pattern set with recommends (like Agama does; the container zypp.conf has onlyRequires) and read the visible .desktop files.
"${Z[@]}" in --recommends --auto-agree-with-licenses pattern:immutable_base pattern:gnome > "$OUT/install.log" 2>&1 || true
{
  for f in "$R"/usr/share/applications/*.desktop; do
    grep -qiE '^NoDisplay=true' "$f" && continue
    grep -qE '^Type=Application' "$f" || continue
    name=$(sed -n 's/^Name=//p' "$f" | head -1)
    pkg=$(rpm --root "$R" -qf --qf '%{NAME}' "${f#"$R"}" 2>/dev/null)
    printf '%s\t%s\t%s\n' "$pkg" "$(basename "$f")" "$name"
  done
} | sort > "$OUT/desktop-apps.tsv"
grep -h -E 'NoDisplay=true' -l "$R"/usr/share/applications/*.desktop 2>/dev/null | xargs -r -n1 basename | sort > "$OUT/hidden-desktop.txt"

# Packages Shrimp may need for Steam (Flatpak) and the browser
for p in steam-devices steam MozillaFirefox MozillaFirefox-branding-openSUSE myrlyn gnome-tour gnome-initial-setup epiphany \
         opensuse-welcome opensuse-welcome-launcher; do
  if "${Z[@]}" se -x "$p" >/dev/null 2>&1; then echo "$p: available"; else echo "$p: missing"; fi
done > "$OUT/availability.txt"
"${Z[@]}" se -s 'steam*' 'game-devices*' 'gamemode*' > "$OUT/search-steam.txt" 2>&1 || true

{
  echo "Leap $V ($REPO)"
  echo "gnome_basic: $(wc -l < "$OUT/pattern-gnome_basic.txt") packages; gnome+immutable_base: $(wc -l < "$OUT/pattern-gnome.txt")"
  echo "=== visible desktop applications (package, desktop file, name)"
  cat "$OUT/desktop-apps.tsv"
  echo "=== availability"
  cat "$OUT/availability.txt"
} > "$OUT/summary.txt"
