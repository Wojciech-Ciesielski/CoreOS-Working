#!/bin/bash
# Rozruch nośnika Shrimp bez GRUB-a (Beta 6): shim → systemd-boot → jądro, menu ukryte.
#
# KIWI (bootloader systemd_boot) kładzie systemd-boot, jądro i initrd tylko do obrazu partycji EFI
# doklejonego do ISO (El Torito), bez shim i bez plików w drzewie ISO. Shrimp Install kopiuje na pendrive
# PLIKI z ISO (wszystko poza LiveOS/ trafia na FAT32 SHRIMP_BOOT), więc bez tego kroku pendrive nie miałby
# czym wystartować. Ten skrypt:
#   1. wyjmuje zawartość partycji EFI z ISO,
#   2. podmienia program startowy na shim podpisany przez Microsoft (Secure Boot), a systemd-boot (podpisany
#      przez openSUSE) kładzie obok jako grub.efi, bo tak nazwany plik shim openSUSE uruchamia; tak samo
#      robi sdbootutil w zainstalowanym systemie,
#   3. ustawia menu: ukryte (timeout 0), bez edytora i bez automatycznych wpisów (Windows, UEFI),
#   4. kopiuje EFI/, loader/ i os/ do drzewa ISO (dla Shrimp Install) i dokleja nową partycję EFI
#      (start ISO wprost, np. w maszynie wirtualnej).
# Użycie: tools/iso-boot.sh <plik.iso> <image-root KIWI>
set -euo pipefail
ISO="$1"
ROOT="$2"
log() { echo "iso-boot: $*"; }
W=$(mktemp -d)
trap 'rm -rf "$W"' EXIT

# 1. Partycja EFI doklejona przez KIWI (typ 0xef w MBR albo ESP w GPT).
read -r START SECTORS < <(python3 - "$ISO" <<'PY'
import struct, sys
f = open(sys.argv[1], "rb")
mbr = f.read(512)
parts = [struct.unpack_from("<B3xB3xII", mbr, 446 + 16 * i) for i in range(4)]
esp = [(s, n) for (_, t, s, n) in parts if t == 0xEF and n]
if not esp and any(t == 0xEE for (_, t, _, _) in parts):
    f.seek(512); h = f.read(92)
    lba, count, size = struct.unpack_from("<QII", h, 72)
    f.seek(lba * 512)
    ESP = bytes.fromhex("28732ac11ff8d211ba4b00a0c93ec93b")
    for i in range(count):
        e = f.read(size)
        if e[:16] == ESP:
            first, last = struct.unpack_from("<QQ", e, 32)
            esp = [(first, last - first + 1)]; break
if esp:
    print(*esp[0])
PY
)
[ -n "${START:-}" ] || { log "BŁĄD: brak partycji EFI w $ISO"; exit 1; }
SECTOR=512
dd if="$ISO" of="$W/kiwi-efi.img" bs="$SECTOR" skip="$START" count="$SECTORS" status=none
mkdir -p "$W/esp"
mcopy -s -n -i "$W/kiwi-efi.img" '::*' "$W/esp/"
log "partycja EFI z KIWI:"; (cd "$W/esp" && find . -type f -printf '  %p %s\n' | sort)

# 2. shim + systemd-boot (podpisane pliki z obrazu, te same co w zainstalowanym systemie).
SHIM_DIR="$ROOT/usr/share/efi/x86_64"
SDBOOT="$ROOT/usr/lib/systemd-boot/systemd-bootx64.efi"
[ -f "$SDBOOT" ] || SDBOOT="$ROOT/usr/lib/systemd/boot/efi/systemd-bootx64.efi"
[ -f "$SHIM_DIR/shim.efi" ] || { log "BŁĄD: brak shim.efi w obrazie ($SHIM_DIR)"; exit 1; }
[ -f "$SDBOOT" ] || { log "BŁĄD: brak systemd-bootx64.efi w obrazie"; exit 1; }
rm -rf "$W/esp/EFI"
mkdir -p "$W/esp/EFI/BOOT"
install -m644 "$SHIM_DIR/shim.efi" "$W/esp/EFI/BOOT/bootx64.efi"
install -m644 "$SDBOOT" "$W/esp/EFI/BOOT/grub.efi"
[ -f "$SHIM_DIR/MokManager.efi" ] && install -m644 "$SHIM_DIR/MokManager.efi" "$W/esp/EFI/BOOT/MokManager.efi"
log "shim: $(basename "$SHIM_DIR")/shim.efi, systemd-boot: ${SDBOOT#"$ROOT"}"

# 3. Menu: jeden wpis Shrimp, ukryty (wciśnięty klawisz przy starcie je pokazuje).
ENTRIES="$W/esp/loader/entries"
ENTRY=$(find "$ENTRIES" -name '*.conf' -printf '%f\n' 2>/dev/null | sort | head -n1)
[ -n "$ENTRY" ] || { log "BŁĄD: brak wpisu systemd-boot (loader/entries) z KIWI"; exit 1; }
for f in "$ENTRIES"/*.conf; do
  [ "$(basename "$f")" = "$ENTRY" ] || rm -f "$f"
done
sed -i -e 's/^title .*/title Shrimp/' "$ENTRIES/$ENTRY"
# KIWI 11 zostawia w wpisie systemd-boot puste „options” dla ISO live. Uzupełniamy je tak, jak KIWI robi to
# w menu GRUB (bootloader/config/grub2.py: root=live:CDLABEL, rd.live.image, trwała nakładka) i dokładamy
# kernelcmdline z config.kiwi.
if ! grep -q '^options .*root=live:' "$ENTRIES/$ENTRY"; then
  CFG=${KIWI_CONFIG:-out/desc/config.kiwi}; [ -f "$CFG" ] || CFG=boxes/image-base/config.kiwi
  OPTS=$(python3 - "$CFG" <<'PY'
import sys, xml.etree.ElementTree as ET
t = ET.parse(sys.argv[1]).getroot().find("preferences/type")
o = [f"root=live:CDLABEL={t.get('volid', 'CDROM')}", "rd.live.image"]
if t.get("hybridpersistent") == "true":
    o.append("rd.live.overlay.persistent")
    if t.get("hybridpersistent_filesystem"):
        o.append(f"rd.live.overlay.cowfs={t.get('hybridpersistent_filesystem')}")
o += (t.get("kernelcmdline") or "").split()
print(" ".join(o))
PY
)
  OLD=$(sed -n 's/^options *//p' "$ENTRIES/$ENTRY")
  sed -i '/^options/d' "$ENTRIES/$ENTRY"
  echo "options $OPTS${OLD:+ $OLD}" >> "$ENTRIES/$ENTRY"
  log "options uzupełnione z config.kiwi"
fi
grep -q '^options .*root=live:' "$ENTRIES/$ENTRY" || { log "BŁĄD: wpis bez root=live: ($ENTRY)"; cat "$ENTRIES/$ENTRY"; exit 1; }
cat > "$W/esp/loader/loader.conf" <<EOF
default $ENTRY
timeout 0
editor no
auto-entries no
auto-firmware no
console-mode keep
EOF
rm -f "$W/esp/loader/random-seed"
log "wpis $ENTRY:"; sed 's/^/  /' "$ENTRIES/$ENTRY"

# 4. Nowa partycja EFI (zawartość + 16 MiB zapasu) i te same pliki w drzewie ISO.
KB=$(( $(du -sk "$W/esp" | cut -f1) + 16384 ))
mkfs.vfat -n SHRIMP_EFI -C "$W/efi.img" "$KB" >/dev/null
mcopy -s -i "$W/efi.img" "$W/esp/"* ::/
OUT="$ISO.boot"
xorriso -indev "$ISO" -outdev "$OUT" -compliance iso_9660_level=3 -joliet on \
  -map "$W/esp/EFI" /EFI -map "$W/esp/loader" /loader -map "$W/esp/os" /os \
  -boot_image any replay \
  -append_partition 2 0xef "$W/efi.img" \
  -boot_image any efi_path=--interval:appended_partition_2:all:: 2>&1 | grep -v '^xorriso : UPDATE' || true
[ -s "$OUT" ] || { log "BŁĄD: xorriso nie zapisał obrazu"; exit 1; }
for f in /EFI/BOOT/bootx64.efi /EFI/BOOT/grub.efi /loader/loader.conf "/loader/entries/$ENTRY"; do
  xorriso -indev "$OUT" -ls "$f" >/dev/null 2>&1 || { log "BŁĄD: brak $f w nowym ISO"; exit 1; }
done
xorriso -indev "$OUT" -toc 2>/dev/null | grep -q Joliet || { log "BŁĄD: obraz bez drzewa Joliet"; exit 1; }
mv "$OUT" "$ISO"
xorriso -indev "$ISO" -report_el_torito plain 2>/dev/null | grep -i 'El Torito\|boot img' || true
log "gotowe: $(du -h "$ISO" | cut -f1)"
