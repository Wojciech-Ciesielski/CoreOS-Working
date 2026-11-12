#!/bin/bash
# Sonda instalacji offline: jakie obrazy offline ma Leap i ile waży zestaw pakietów produktu Shrimp.
# Uruchamiana jako root w kontenerze openSUSE (.github/workflows/offline-probe.yml).
#   tools/offline-probe.sh LEAP_VERSION OUTDIR
set -uo pipefail
V=${1:-16.1}; OUT=${2:-out/offline}
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
exec > >(tee "$OUT/probe.txt") 2>&1
for v in "$V" 16.0; do
  for d in "" iso/ offline/ installer/ agama/ appliances/; do
    u="https://download.opensuse.org/distribution/leap/$v/$d"
    echo "== $u"
    curl -fsSL "$u" 2>/dev/null | grep -oE 'href="[^"?]+"' | sed 's/href=//;s/"//g' | grep -vE '^(/|\.\./|https?:)' | head -60
  done
done
for v in "$V" 16.0; do
  u="https://download.opensuse.org/distribution/leap/$v/offline/"
  for f in $(curl -fsSL "$u" 2>/dev/null | grep -oE 'href="[^"?]+\.iso"' | sed 's/href=//;s/"//g' | sort -u); do
    echo "SIZE $u$f $(curl -fsSLI "$u$f" | grep -i '^content-length' | tail -1)"
  done
done
base="https://download.opensuse.org/distribution/leap/$V/repo/oss"
REPO=""
for c in "$base/x86_64" "$base"; do curl -fsSLI "$c/repodata/repomd.xml" >/dev/null 2>&1 && { REPO=$c; break; }; done
echo "repo: $REPO"
R=/var/tmp/off-root
Z=(zypper -n --root "$R" --gpg-auto-import-keys)
rm -rf "$R"; mkdir -p "$R"
"${Z[@]}" ar -f "$REPO" leap >/dev/null; "${Z[@]}" ref >/dev/null
PK="Leap_immutable-release NetworkManager openSUSE-repos-Leap sudo-policy-wheel-auth-self flatpak gdm libwebkitgtk-6_0-4 parted dosfstools exfatprogs ntfsprogs steam-devices gamemode os-prober kernel-default grub2-x86_64-efi shim"
echo "== product set (patterns immutable_base gnome + packages)"
"${Z[@]}" in --dry-run --download-only --auto-agree-with-licenses pattern:immutable_base pattern:gnome $PK 2>&1 | tail -8
echo "== firmware / microcode packages"
"${Z[@]}" se -t package 'kernel-firmware*' 'ucode-*' 2>/dev/null | awk -F'|' 'NR>4 {gsub(/ /,"",$2); print $2}' | sort -u | tr '\n' ' '; echo
"${Z[@]}" in --dry-run --download-only --auto-agree-with-licenses pattern:immutable_base pattern:gnome $PK 'kernel-firmware*' 'ucode-*' 2>&1 | tail -4
echo "== repo total"
"${Z[@]}" se -t package 2>/dev/null | wc -l
