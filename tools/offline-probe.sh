#!/bin/bash
# Sonda instalacji offline: co jest w oficjalnym obrazie Leap offline (repozytorium, podpis, rozmiar).
# Uruchamiana jako root w kontenerze openSUSE (.github/workflows/offline-probe.yml).
#   tools/offline-probe.sh LEAP_VERSION OUTDIR
set -uo pipefail
V=${1:-16.1}; OUT=${2:-out/offline}
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
exec > >(tee "$OUT/probe.txt") 2>&1
zypper -n in --no-recommends xorriso gawk zstd >/dev/null
ISO=/var/tmp/leap-offline.iso
curl -fsSL -o "$ISO" "https://download.opensuse.org/distribution/leap/$V/offline/Leap-$V-offline-installer-x86_64.install.iso"
ls -l "$ISO"
xorriso -indev "$ISO" -pvd_info 2>&1 | grep -i 'volume id\|Volume Set'
echo "== top"
xorriso -indev "$ISO" -lsl / 2>/dev/null
echo "== du (MiB) per top dir"
for d in $(xorriso -indev "$ISO" -ls / 2>/dev/null | tr -d "'"); do
  echo "$d $(xorriso -indev "$ISO" -du "/$d" 2>/dev/null | tail -1)"
done
echo "== repodata / content files"
xorriso -indev "$ISO" -find / -name repomd.xml\* 2>/dev/null
xorriso -indev "$ISO" -find / -maxdepth 2 -type f 2>/dev/null | head -60
echo "== media.1"
xorriso -indev "$ISO" -find / -path '*media.1*' 2>/dev/null | head
M=/var/tmp/leap-mnt; mkdir -p $M
xorriso -osirrox on -indev "$ISO" -extract /media.1 $M/media.1 >/dev/null 2>&1 && cat $M/media.1/* 2>/dev/null | head -20
r=$(xorriso -indev "$ISO" -find / -name repomd.xml 2>/dev/null | head -1 | tr -d "'")
echo "repomd: $r"
if [ -n "$r" ]; then
  d=$(dirname "$(dirname "$r")")
  xorriso -osirrox on -indev "$ISO" -extract "$d/repodata" $M/repodata >/dev/null 2>&1
  ls -l $M/repodata
  pk=$(ls $M/repodata/*primary.xml* | head -1)
  case $pk in *.zst) zstd -dc $pk ;; *.gz) zcat $pk ;; *) cat $pk ;; esac > $M/primary.xml
  echo "packages: $(grep -c '<package ' $M/primary.xml)"
  grep -o '<name>[^<]*</name>' $M/primary.xml | sed 's/<[^>]*>//g' | sort -u > "$OUT/offline-names.txt"
  for n in kernel-firmware-all gnome-shell gdm flatpak NetworkManager exfatprogs ntfsprogs steam-devices gamemode libwebkitgtk-6_0-4 gnome-shell-lang glibc-locale ffmpeg-7 gstreamer-plugins-bad sof-firmware fwupd ModemManager b43-fwcutter python3-nautilus inter-fonts google-noto-sans-cjk-fonts; do
    grep -qx "$n" "$OUT/offline-names.txt" && echo "HAS $n" || echo "MISSING $n"
  done
  grep -c -- '-lang$' "$OUT/offline-names.txt"
fi
