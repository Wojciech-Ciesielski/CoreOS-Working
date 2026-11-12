#!/bin/bash
# Static checks of a built Shrimp live ISO: mounts ISO -> squashfs -> rootfs and reports
# extensions (versions, shell compatibility), schema defaults and boot config.
# Usage: sudo tools/inspect-iso.sh <iso>
set -uo pipefail
iso="$1"; m=$(mktemp -d)
mkdir -p "$m"/iso "$m"/sq "$m"/root
mount -o loop,ro "$iso" "$m/iso"
sq=$(find "$m/iso" -name squashfs.img -print -quit)
mount -o loop,ro "$sq" "$m/sq"
img=$(find "$m/sq" -name rootfs.img -print -quit)
if [ -n "$img" ]; then mount -o loop,ro "$img" "$m/root"; r="$m/root"; else r="$m/sq"; fi
echo "=== os-release"; cat "$r/etc/os-release"
echo "=== gnome-shell"; ls "$r/usr/share/gnome-shell/extensions"
for d in "$r"/usr/share/gnome-shell/extensions/*/; do
  python3 -c "import json,sys;d=json.load(open(sys.argv[1]));print(d['uuid'],'version',d.get('version'),d.get('version-name',''),'shell',d.get('shell-version'))" "$d/metadata.json"
done
rpm --root "$r" -q gnome-shell gnome-shell-extensions-common gdm plymouth grub2 \
  pipewire wireplumber bluez upower gnome-bluetooth tuned-ppd power-profiles-daemon iio-sensor-proxy \
  gnome-software gnome-system-monitor gnome-disk-utility gnome-extensions baobab gnome-logs 2>/dev/null || ls "$r"/usr/lib64/gnome-shell 2>/dev/null
echo "=== typelibs"; ls "$r"/usr/lib64/girepository-1.0/ | grep -i -E 'gnomedesktop|gnomebg' || echo "no GnomeDesktop typelib"
echo "=== schema defaults"
S="$r/usr/share/glib-2.0/schemas"
for k in "org.gnome.shell enabled-extensions" "org.gnome.shell.extensions.just-perfection activities-button" \
         "org.gnome.desktop.background picture-uri" "org.gnome.desktop.interface color-scheme" \
         "org.gnome.shell.extensions.dash-to-dock dock-position" \
         "org.gnome.shell.extensions.blur-my-shell.overview blur" "org.gnome.shell.extensions.blur-my-shell.panel blur"; do
  echo "$k = $(GSETTINGS_SCHEMA_DIR=$S gsettings get $k 2>&1)"
done
for d in "$r"/usr/share/gnome-shell/extensions/*/schemas; do
  [ -f "$d/gschemas.compiled" ] || continue
  echo "local $(basename "$(dirname "$d")"): just-perfection activities-button = $(GSETTINGS_SCHEMA_DIR=$d gsettings get org.gnome.shell.extensions.just-perfection activities-button 2>&1 | head -1)"
  case "$d" in *blur-my-shell*)
    echo "local blur-my-shell: overview blur = $(GSETTINGS_SCHEMA_DIR=$d gsettings get org.gnome.shell.extensions.blur-my-shell.overview blur 2>&1 | head -1), panel blur = $(GSETTINGS_SCHEMA_DIR=$d gsettings get org.gnome.shell.extensions.blur-my-shell.panel blur 2>&1 | head -1)";; esac
done
echo "=== libraries"; ls -l "$r"/usr/lib64/libz.so* "$r"/lib64/libz.so* 2>&1
ls "$r"/usr/share/locale/pl/LC_MESSAGES/ | grep -E '^(gvfs|NetworkManager)\.mo$' || echo "no gvfs/NetworkManager Polish translations"
ls "$r"/usr/libexec/gvfsd-trash 2>&1
echo "=== shell theme"
for c in $(gresource list "$r/usr/share/gnome-shell/gnome-shell-theme.gresource" | grep -E '\.css$|shrimp'); do
  echo "$c: $(gresource extract "$r/usr/share/gnome-shell/gnome-shell-theme.gresource" "$c" | grep -c 'Shrimp additions') shrimp blocks"
done
echo "=== override warnings"; glib-compile-schemas --dry-run --strict "$S" 2>&1 | head -20
echo "=== gdm"; cat "$r/etc/gdm/custom.conf"; ls -l "$r"/etc/systemd/system/display-manager.service 2>&1
echo "=== grub.cfg (EFI)"; f=$(find "$m/iso" -path '*EFI*' -name grub.cfg -print -quit); [ -n "$f" ] && cat "$f"
echo "=== grub.cfg (BIOS)"; f=$(find "$m/iso/boot" -name grub.cfg -print -quit); [ -n "$f" ] && cat "$f"
echo "=== theme"; ls -la "$m/iso/boot/grub2/themes/shrimp" 2>&1
umount "$m/root" 2>/dev/null; umount "$m/sq"; umount "$m/iso"
