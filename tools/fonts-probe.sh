#!/bin/bash
# Which font packages Leap offers for the scripts of all GNOME languages (names for boxes/fonts/box.toml).
# Runs as root in an openSUSE container (see .github/workflows/fonts-probe.yml).
#   tools/fonts-probe.sh LEAP_VERSION OUTDIR
set -uo pipefail
V=${1:-16.1}; OUT=${2:-out/fonts}
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
base="https://download.opensuse.org/distribution/leap/$V/repo/oss"
REPO=""
for c in "$base/x86_64" "$base"; do
  curl -fsSLI "$c/repodata/repomd.xml" >/dev/null 2>&1 && { REPO=$c; break; }
done
[ -n "$REPO" ] || { echo "no Leap $V repository" >&2; exit 1; }
R=/var/tmp/fonts-root
Z=(zypper -n --root "$R" --gpg-auto-import-keys)
rm -rf "$R"; mkdir -p "$R"
"${Z[@]}" ar -f "$REPO" leap >/dev/null
"${Z[@]}" ref >/dev/null
# every font package in the repository (name only)
"${Z[@]}" se -t package --match-substrings -s fonts 2>/dev/null | awk -F'|' 'NR>4 {gsub(/ /,"",$2); print $2}' | sort -u > "$OUT/all-fonts.txt"
for p in $(sed -E 's/^\s+//' <<'LIST'
google-noto-sans-fonts google-noto-serif-fonts google-noto-sans-cjk-fonts google-noto-sans-arabic-fonts
google-noto-sans-hebrew-fonts google-noto-sans-devanagari-fonts google-noto-sans-bengali-fonts google-noto-sans-tamil-fonts
google-noto-sans-telugu-fonts google-noto-sans-kannada-fonts google-noto-sans-malayalam-fonts google-noto-sans-gujarati-fonts
google-noto-sans-gurmukhi-fonts google-noto-sans-oriya-fonts google-noto-sans-sinhala-fonts google-noto-sans-thai-fonts
google-noto-sans-lao-fonts google-noto-sans-khmer-fonts google-noto-sans-myanmar-fonts google-noto-sans-tibetan-fonts
google-noto-sans-ethiopic-fonts google-noto-sans-georgian-fonts google-noto-sans-armenian-fonts google-noto-sans-olchiki-fonts
google-noto-sans-ol-chiki-fonts google-noto-sans-cherokee-fonts google-noto-sans-canadianaboriginal-fonts
google-noto-sans-canadian-aboriginal-fonts google-noto-sans-symbols-fonts google-noto-sans-symbols2-fonts
google-noto-naskh-arabic-fonts google-noto-kufi-arabic-fonts google-noto-nastaliq-urdu-fonts
google-noto-sans-ui-fonts noto-sans-fonts noto-sans-arabic-fonts
LIST
); do
  if "${Z[@]}" info "$p" >/dev/null 2>&1; then echo "$p: available"; else echo "$p: missing"; fi
done > "$OUT/availability.txt"
# what the live image already gets: gnome_basic plus suggestions
"${Z[@]}" in --dry-run --recommends --auto-agree-with-licenses pattern:gnome_basic 2>&1 | grep -o '[a-z0-9-]*font[a-z0-9-]*' | sort -u > "$OUT/in-gnome_basic.txt"
{ echo "available: $(grep -c ': available' "$OUT/availability.txt")  missing: $(grep -c ': missing' "$OUT/availability.txt")"; echo "font packages in repo: $(wc -l < "$OUT/all-fonts.txt")"; } > "$OUT/summary.txt"
cat "$OUT/summary.txt" "$OUT/availability.txt"
