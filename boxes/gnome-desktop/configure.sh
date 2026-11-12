# Fragment config.sh boxa gnome-desktop (chroot obrazu; składa go shrimp-box).
# System menu and system tools (packages that may be missing or renamed in Leap 16.1).
# config.kiwi fails the whole build on an unknown name, so these are optional: the first
# available name of each group is installed (with its -lang package), a missing group is a WARNING.
# The build repository is not part of the image (--set-repo), so it is added here and removed after.
VERSION_ID="$(. /etc/os-release && echo "$VERSION_ID")"
LEAP="https://download.opensuse.org/distribution/leap/${VERSION_ID}/repo/oss"
OPT_REPO=""
for candidate in "$LEAP/x86_64" "$LEAP"; do
  if curl -fsSLI "$candidate/repodata/repomd.xml" >/dev/null 2>&1; then OPT_REPO="$candidate"; break; fi
done
opt_pkg() {
  local p
  for p in "$@"; do
    if zypper -n --gpg-auto-import-keys install --no-recommends "$p" >/dev/null 2>&1; then
      zypper -n --gpg-auto-import-keys install --no-recommends "$p-lang" >/dev/null 2>&1 || true
      echo "optional package installed: $p"
      return 0
    fi
  done
  echo "WARNING: none of these packages is available: $*"
}
if [ -n "$OPT_REPO" ]; then
  zypper -n --gpg-auto-import-keys addrepo -f "$OPT_REPO" shrimp-build-oss
  zypper -n --gpg-auto-import-keys refresh shrimp-build-oss
  opt_pkg 'typelib(GLibUnix)' typelib-1_0-GLibUnix-2_0  # DING: GLibUnix 2.0 (GLib 2.86)
  opt_pkg gnome-bluetooth                       # Quick Settings: Bluetooth toggle
  opt_pkg typelib-1_0-GnomeBluetooth-3_0
  opt_pkg tuned-ppd power-profiles-daemon       # Quick Settings: power mode
  opt_pkg iio-sensor-proxy                      # Quick Settings: auto-rotate (2-in-1 laptops)
  opt_pkg gnome-system-monitor                  # Shrimp menu: System Monitor
  opt_pkg gnome-disk-utility                    # Shrimp menu: Disks
  opt_pkg gnome-extensions                      # Shrimp menu: Extensions
  opt_pkg baobab                                # app grid: disk usage
  opt_pkg gnome-logs                            # app grid: system log
  opt_pkg gvfs-lang                             # translated "Trash" on the desktop (trash:// display name)
  opt_pkg NetworkManager-lang                   # translated "Wired" in Quick Settings (libnm device names)
  # File search (localsearch-extractor-3) links against libz.so.1, which no installed package provides
  # in Leap 16.1: without it the extractor crash-loops once a second for the whole session.
  if ! ldconfig -p | grep -q 'libz\.so\.1 '; then
    opt_pkg libz1 libz-ng-compat1 zlib-ng-compat
  fi
  zypper -n removerepo shrimp-build-oss
  zypper -n clean -a
else
  echo "WARNING: Leap ${VERSION_ID} repository not reachable, optional system tools skipped"
fi
if [ -x /usr/libexec/localsearch-extractor-3 ] && ldd /usr/libexec/localsearch-extractor-3 | grep -q 'not found'; then
  echo "WARNING: localsearch-extractor-3 is missing libraries: $(ldd /usr/libexec/localsearch-extractor-3 | grep 'not found' | tr -s ' \t' ' ')"
fi
for unit in bluetooth.service tuned.service tuned-ppd.service power-profiles-daemon.service; do
  [ -f "/usr/lib/systemd/system/$unit" ] && systemctl enable "$unit" || true
done
# Shrimp Menu logo (symbolic, coloured by the top bar)
gtk-update-icon-cache -f /usr/share/icons/hicolor || true
