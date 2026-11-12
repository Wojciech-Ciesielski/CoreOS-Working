#!/bin/bash
# Dokłada do boxes/ aplikacje Shrimps (Shrimp Apps, Cocktail, Panko, Cherry i ich biblioteki) z repozytorium Shrimps-Working.
# Wersja jest przypięta w tools/shrimps.ref („właściciel/repo@tag-albo-commit”), więc ISO zawsze buduje się z tego samego kodu.
# Boxy, które CoreOS już ma (np. shrimp-box, i18n-core), zostają z CoreOS. Pobrane boxy tracą wpis `ci`
# (ich CI jest w Shrimps-Working) i są w .gitignore, więc nie trafiają do commita.
# Użycie: tools/fetch-shrimps.sh   (wymaga git; repozytorium publiczne, bez tokenu)
set -euo pipefail
cd "$(dirname "$0")/.."
spec=$(grep -v '^#' tools/shrimps.ref | head -n1 | tr -d '[:space:]')
repo=${spec%@*}; ref=${spec#*@}
[ -n "$repo" ] && [ -n "$ref" ] && [ "$repo" != "$spec" ] || { echo "tools/shrimps.ref: oczekiwano właściciel/repo@ref" >&2; exit 1; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
git -C "$tmp" init -q
git -C "$tmp" fetch -q --depth 1 "https://github.com/$repo" "$ref"
git -C "$tmp" -c advice.detachedHead=false checkout -q FETCH_HEAD
echo "Shrimps: $repo@$ref ($(git -C "$tmp" rev-parse --short HEAD))"
for d in "$tmp"/boxes/*/; do
  id=$(basename "$d")
  if [ -e "boxes/$id" ] && ! grep -qx "/boxes/$id/" .gitignore; then
    echo "  $id: zostaje wersja z CoreOS"; continue
  fi
  rm -rf "boxes/$id"; cp -a "$d" "boxes/$id"
  sed -i '/^ci *= *"/d' "boxes/$id/box.toml"
  echo "  $id: z Shrimps"
done
