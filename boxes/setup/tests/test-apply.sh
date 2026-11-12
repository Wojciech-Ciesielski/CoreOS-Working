#!/bin/bash
# Test shrimp-setup-apply na atrapach zypper/transactional-update/mokutil i sztucznym sysfs.
#   setup/tests/test-apply.sh
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
APPLY=$HERE/../system/libexec/shrimp-setup-apply
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
mkdir -p "$T/bin" "$T/state"
LOG=$T/log
# zypper: „search” znajduje pakiety z listy $T/avail, reszta poleceń jest zapisywana.
# „search -s” wypisuje tabelę wersji z $T/versions (nazwa wersja), „install meta = wersja” udaje się
# tylko dla wersji z $T/good (moduł SUSE w tej wersji jest w repozytorium).
cat > "$T/bin/zypper" <<Z
#!/bin/bash
if [[ " \$* " == *" search "* ]]; then
  if [[ " \$* " == *" -s "* ]]; then
    echo "S | Name | Type | Version | Arch | Repository"; echo "--+------+------+---------+------+-----------"
    awk -v n="\${@: -1}" '\$1 == n { print "  | " \$1 " | package | " \$2 " | x86_64 | NVIDIA" }' "$T/versions" 2>/dev/null
    exit 0
  fi
  grep -qx "\${@: -1}" "$T/avail"; exit
fi
echo "zypper \$*" >> "$LOG"
for a in "\$@"; do
  case "\$a" in *signed-kmp-meta\ =\ *) grep -qx "\${a##* }" "$T/good" 2>/dev/null || { echo "nothing provides kmp" >&2; exit 1; } ;; esac
done
exit 0
Z
cat > "$T/bin/transactional-update" <<Z
#!/bin/bash
echo "transactional-update \$1 \$2" >> "$LOG"
[ "\$2" = run ] && { shift 2; "\$@"; }
Z
cat > "$T/bin/systemctl" <<Z
#!/bin/bash
echo "systemctl \$*" >> "$LOG"
Z
cat > "$T/bin/mokutil" <<Z
#!/bin/bash
cat "$T/sb" 2>/dev/null
Z
chmod +x "$T/bin/"*
export SHRIMP_NVIDIA_LIB=$HERE/../../linu-x/root/usr/share/shrimp/linux/nvidia-open.sh
export PATH="$T/bin:$PATH" SHRIMP_AUTOSTART_DIR=$T/autostart SHRIMP_STATE=$T/state SHRIMP_SYSFS=$T/sys SHRIMP_DRIVERS_CONF=$HERE/../../linu-x/root/usr/share/shrimp/linux/drivers.conf

gpu() { rm -rf "$T/sys"; mkdir -p "$T/sys/bus/pci/devices/0000:01:00.0"; local d=$T/sys/bus/pci/devices/0000:01:00.0; echo 0x10de > "$d/vendor"; echo 0x030000 > "$d/class"; echo "$1" > "$d/device"; }
run() { : > "$LOG"; touch "$T/state/first-boot"; mkdir -p "$T/autostart"; touch "$T/autostart/shrimp-setup-first-boot.desktop"; "$APPLY" "$@" 2>>"$T/stderr"; }
fail() { echo "FAIL: $*"; echo "--- log"; cat "$LOG"; echo "--- stderr"; tail -5 "$T/stderr"; exit 1; }

# 1. RTX 3060 (Turing+), G07 dostępne: otwarty sterownik G07 + kodeki, jedna migawka, restart.
#    Najnowszy meta-pakiet NVIDIA (595.112.07) nie ma jeszcze modułu SUSE: bierze 595.99.02 (Beta 3.2).
gpu 0x2504
printf '%s\n' 'nvidia-open-driver-G07-signed-kmp-meta 595.112.07-lp161.44.1' 'nvidia-open-driver-G07-signed-kmp-meta 595.99.02-lp161.10.1' \
  'nvidia-open-driver-G06-signed-kmp-meta 580.178.04-lp161.58.1' > "$T/versions"
echo 595.99.02-lp161.10.1 > "$T/good"
printf '%s\n' openSUSE-repos-Leap-NVIDIA nvidia-open-driver-G07-signed-kmp-meta nvidia-video-G07 nvidia-gl-G07 nvidia-compute-utils-G07 \
  gstreamer-plugin-pipewire gstreamer-plugins-good mozilla-openh264 > "$T/avail"
OUT=$(run --codecs yes --nvidia nvidia)
[ "$OUT" = reboot ] || fail "1: brak reboot"
[ "$(grep -c '^transactional-update' "$LOG")" = 1 ] || fail "1: więcej niż jedna migawka"
grep -q 'install --no-recommends --auto-agree-with-licenses nvidia-open-driver-G07-signed-kmp-meta = 595.99.02-lp161.10.1$' "$LOG" || fail "1: G07 w wersji z modułem"
grep -q 'install --no-recommends --auto-agree-with-licenses nvidia-video-G07 = 595.99.02$' "$LOG" || fail "1: biblioteki w wersji modułu"
grep -q 'install --no-recommends --auto-agree-with-licenses nvidia-compute-utils-G07 = 595.99.02$' "$LOG" || fail "1: compute-utils"
grep -q 'install --no-recommends gstreamer-plugin-pipewire gstreamer-plugins-good mozilla-openh264' "$LOG" || fail "1: kodeki"
! grep -q 'addrepo.*openh264' "$LOG" || fail "1: repozytorium openh264 dodane, choć pakiet jest"
grep -q 'systemctl enable nvidia-suspend.service' "$LOG" || fail "1: usługi uśpienia NVIDIA"
[ -f "$T/state/nvidia-flatpak" ] && [ ! -f "$T/state/first-boot" ] || fail "1: znaczniki"
[ ! -e "$T/autostart/shrimp-setup-first-boot.desktop" ] || fail "1: autostart Setup został po konfiguracji"

# 2. G07 bez żadnej wersji z modułem, G06 ma moduł: wybiera G06.
printf '%s\n' nvidia-open-driver-G06-signed-kmp-meta nvidia-video-G06 nvidia-gl-G06 > "$T/avail"
echo 580.178.04-lp161.58.1 > "$T/good"
run --codecs no --nvidia nvidia >/dev/null
grep -q 'install --no-recommends --auto-agree-with-licenses nvidia-open-driver-G06-signed-kmp-meta = 580.178.04-lp161.58.1$' "$LOG" || fail "2: G06"
grep -q 'nvidia-gl-G06 = 580.178.04$' "$LOG" || fail "2: biblioteki G06"
! grep -qE 'nvidia-(video|gl)-G07' "$LOG" || fail "2: biblioteki G07 bez modułu"

# 2b. Żadna seria nie ma modułu: migawka się nie udaje, bez restartu, konfiguracja i tak kończy się.
: > "$T/good"
OUT=$(run --codecs no --nvidia nvidia)
[ -z "$OUT" ] && [ ! -f "$T/state/first-boot" ] || fail "2b: brak modułu"

# 3. GTX 1060 (Pascal) z Secure Boot: bez sterownika NVIDIA, bez migawki.
gpu 0x1c03; echo "SecureBoot enabled" > "$T/sb"
OUT=$(run --codecs no --nvidia nvidia)
[ -z "$OUT" ] && ! grep -q transactional-update "$LOG" || fail "3: starsza karta z SB"

# 4. GTX 1060 bez Secure Boot: zamknięty G06.
echo "SecureBoot disabled" > "$T/sb"
printf '%s\n' nvidia-driver-G06-kmp-default nvidia-video-G06 nvidia-gl-G06 nvidia-compute-utils-G06 > "$T/avail"
run --codecs no --nvidia nvidia >/dev/null
grep -q 'install --auto-agree-with-licenses nvidia-driver-G06-kmp-default' "$LOG" || fail "4: legacy"

# 5. Wybór „Sterownik otwarty”: nic z NVIDIA.
gpu 0x2504
OUT=$(run --codecs no --nvidia open)
[ -z "$OUT" ] && ! grep -q nvidia "$LOG" || fail "5: otwarty"
# 6. Kodeki bez openh264 w repozytoriach: dodaje repozytorium Cisco.
rm -rf "$T/sys"; printf '%s\n' gstreamer-plugins-good > "$T/avail"
run --codecs yes --nvidia none >/dev/null
grep -q 'zypper --non-interactive addrepo --refresh http://codecs.opensuse.org/openh264/' "$LOG" || fail "6: repozytorium openh264"
grep -q 'install --no-recommends gstreamer-plugins-good$' "$LOG" || fail "6: kodeki"
echo "test-apply: 7/7 OK"
