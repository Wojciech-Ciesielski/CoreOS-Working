#!/bin/bash
# Test shrimp-usb-backup bez prawdziwego pendrive'a: kopia, sklejanie części, przywrócenie do katalogu, wykrycie uszkodzenia.
#   setup/tests/test-usb-backup.sh
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
B=$HERE/../system/libexec/shrimp-usb-backup
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
fail() { echo "FAIL: $*"; exit 1; }
export SHRIMP_STATE=$T/state SHRIMP_USB_IDENTITY=$'SN123\t16000000000'
mkdir -p "$T/state" "$T/root/var/lib/shrimp-setup"

# „Nośnik” po Shrimp Install: shrimp-backup z opisem, plik podzielony na 3 części, pusty folder, nazwa ze spacją.
S=$T/media/shrimp-backup; mkdir -p "$S/files/DCIM" "$S/files/Pusty folder"
echo "notatka" >"$S/files/notatka.txt"
head -c 3000 /dev/urandom >"$S/files/DCIM/a.bin"
head -c 3000 /dev/urandom >"$T/big"
split -b 1000 -d -a 3 "$T/big" "$S/files/big.iso.shrimp-part"
sum() { sha256sum <"$1" | cut -d' ' -f1; }
{
  echo '#shrimp-backup 1'; printf 'label\tMOJ USB\nfs\tFAT32\nscheme\tmbr\n'
  printf 'd\tDCIM\nd\tPusty folder\n'
  printf 'f\t%s\t8\t1700000000\t0\tnotatka.txt\n' "$(sum "$S/files/notatka.txt")"
  printf 'f\t%s\t3000\t1700000100\t0\tDCIM/a.bin\n' "$(sum "$S/files/DCIM/a.bin")"
  printf 'f\t%s\t3000\t1700000200\t3\tbig.iso\n' "$(sum "$T/big")"
} >"$S/manifest.tsv"

# 1. kopia na dysk komputera
SHRIMP_USB_SRC=$S "$B" stash --root "$T/root" 2>"$T/err" || { cat "$T/err"; fail "stash"; }
D=$T/root$T/state/usb
cmp "$D/files/big.iso" "$T/big" || fail "części nie zostały sklejone"
[ -d "$D/files/Pusty folder" ] || fail "brak pustego folderu"
grep -qP '^f\t.*\t0\tbig.iso$' "$D/manifest.tsv" || fail "manifest po sklejeniu"

# 2. uszkodzone źródło: nic nie powstaje
cp -a "$S" "$T/bad"; echo x >>"$T/bad/files/notatka.txt"
rm -rf "$T/root$T/state"; SHRIMP_USB_SRC=$T/bad "$B" stash --root "$T/root" 2>/dev/null && fail "uszkodzone źródło przyjęte"
[ ! -e "$T/root$T/state/usb" ] || fail "po błędzie została kopia"

# 3. przywrócenie (stan = kopia z kroku 1)
rm -rf "$T/state"; SHRIMP_USB_SRC=$S "$B" stash --root "$T/root" 2>/dev/null; mv "$T/root$T/state" "$T/state"
"$B" status | grep -q '"pending":true' || fail "status"
# uszkodzona kopia na dysku komputera: odmowa, zanim cokolwiek dotknie pendrive'a
echo x >>"$T/state/usb/files/notatka.txt"
mkdir "$T/usb1"; SHRIMP_USB_TESTDIR=$T/usb1 "$B" restore /dev/null 2>/dev/null && fail "uszkodzona kopia przyjęta"
[ -z "$(ls -A "$T/usb1")" ] || fail "pendrive tknięty mimo uszkodzonej kopii"
echo "notatka" >"$T/state/usb/files/notatka.txt"
mkdir "$T/usb2"; SHRIMP_USB_TESTDIR=$T/usb2 "$B" restore /dev/null >/dev/null 2>"$T/err" || { cat "$T/err"; fail "restore"; }
cmp "$T/usb2/big.iso" "$T/big" || fail "big.iso"
[ -d "$T/usb2/Pusty folder" ] && [ "$(stat -c %Y "$T/usb2/DCIM/a.bin")" = 1700000100 ] || fail "struktura/czas"
[ ! -e "$T/state/usb" ] && [ ! -e "$T/state/usb-summary.json" ] || fail "kopia nie została usunięta po udanym przywróceniu"
"$B" status | grep -q '"pending":false' || fail "status po przywróceniu"

echo OK
