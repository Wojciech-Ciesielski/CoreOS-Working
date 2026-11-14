#!/bin/bash
# Buduje Setup Shrimp i instaluje go do DESTDIR (pakiet, obraz KIWI albo test).
#   boxes/setup/install.sh DESTDIR [--live]
# --live dodaje części tylko dla nośnika: sesję kiosku z kreatorem, token Agamy, repozytorium offline.
# Wymaga: cargo, pakiety deweloperskie gtk4 i libadwaita, box shrimp-i18n obok (tools/fetch-shrimps.sh).
set -euo pipefail
D=${1:?użycie: install.sh DESTDIR [--live]}
LIVE=${2:-}
HERE=$(cd "$(dirname "$0")" && pwd)

# SHRIMP_PREBUILT=1: program zbudowany wcześniej (krok chroot shrimp-box, te same biblioteki co obraz).
if [ -z "${SHRIMP_PREBUILT:-}" ]; then
  (cd "$HERE/shell" && cargo build --release --locked)
fi

install -Dm755 "$HERE/shell/target/release/shrimp-setup" "$D/usr/bin/shrimp-setup"
install -Dm755 "$HERE/system/share/post-install.sh" "$D/usr/share/shrimp-setup/post-install.sh"
install -Dm644 "$HERE/system/share/remove-packages.txt" "$D/usr/share/shrimp-setup/remove-packages.txt"
install -Dm755 "$HERE/system/libexec/shrimp-setup-apply" "$D/usr/libexec/shrimp-setup-apply"
install -Dm755 "$HERE/system/libexec/shrimp-usb-backup" "$D/usr/libexec/shrimp-usb-backup"
install -Dm644 "$HERE/system/polkit/50-shrimp-setup.rules" "$D/usr/share/polkit-1/rules.d/50-shrimp-setup.rules"
install -Dm644 "$HERE/system/systemd/shrimp-nvidia-flatpak.service" "$D/usr/lib/systemd/system/shrimp-nvidia-flatpak.service"
# Pendrive z instalacji wraca po pierwszym starcie (wybór z kroku Pendrive).
install -Dm644 "$HERE/system/systemd/shrimp-usb-restore.service" "$D/usr/lib/systemd/system/shrimp-usb-restore.service"
install -Dm644 "$HERE/system/systemd/shrimp-usb-restore.timer" "$D/usr/lib/systemd/system/shrimp-usb-restore.timer"
# Aktualizacje w tle bez restartu (Setup i Cocktail: shrimp-auto-updates on|off).
install -Dm755 "$HERE/system/libexec/shrimp-auto-updates" "$D/usr/libexec/shrimp-auto-updates"
install -Dm644 "$HERE/system/systemd/shrimp-flatpak-update.service" "$D/usr/lib/systemd/system/shrimp-flatpak-update.service"
install -Dm644 "$HERE/system/systemd/shrimp-flatpak-update.timer" "$D/usr/lib/systemd/system/shrimp-flatpak-update.timer"
# Kodeki, NVIDIA i programy z kreatora po pierwszym starcie, gdy jest internet. Karta Wi-Fi, która nie wstała (MT7921).
install -Dm755 "$HERE/system/libexec/shrimp-setup-later" "$D/usr/libexec/shrimp-setup-later"
install -Dm644 "$HERE/system/systemd/shrimp-setup-later.service" "$D/usr/lib/systemd/system/shrimp-setup-later.service"
install -Dm644 "$HERE/system/systemd/shrimp-setup-later.timer" "$D/usr/lib/systemd/system/shrimp-setup-later.timer"
install -Dm755 "$HERE/system/libexec/shrimp-wifi-revive" "$D/usr/libexec/shrimp-wifi-revive"
install -Dm644 "$HERE/system/systemd/shrimp-wifi-revive.service" "$D/usr/lib/systemd/system/shrimp-wifi-revive.service"
install -Dm644 "$HERE/system/modprobe/50-shrimp-wifi.conf" "$D/usr/lib/modprobe.d/50-shrimp-wifi.conf"
install -Dm644 "$HERE/system/agama/shrimp.yaml" "$D/usr/share/agama/products.d/shrimp.yaml"
# Pakiety kodeków i NVIDIA (drivers.conf) daje box linu-x, wygląd aplikacji box app-style.

if [ "$LIVE" = --live ]; then
  S=$HERE/system/session
  # Sesja kiosku: gnome-kiosk i sam kreator (bez powłoki GNOME, pulpitu i przeglądu). Zastępuje strażnika Setup.
  install -Dm644 "$S/shrimp-setup.session" "$D/usr/share/gnome-session/sessions/shrimp-setup.session"
  install -Dm644 "$S/shrimp-setup-wayland.desktop" "$D/usr/share/wayland-sessions/shrimp-setup.desktop"
  install -Dm644 "$S/org.shrimp.Setup.desktop" "$D/usr/share/applications/org.shrimp.Setup.desktop"
  install -Dm644 "$S/org.shrimp.Setup.service" "$D/usr/lib/systemd/user/org.shrimp.Setup.service"
  install -Dm644 "$S/session.conf" "$D/usr/lib/systemd/user/gnome-session@shrimp-setup.target.d/session.conf"
  # Wybór sesji przed GDM: kiosk albo pulpit GNOME z dyskiem SHRIMP_TEST (testy QEMU, pomoc techniczna).
  install -Dm755 "$S/shrimp-live-session" "$D/usr/libexec/shrimp-live-session"
  install -Dm644 "$S/shrimp-live-session.service" "$D/usr/lib/systemd/system/shrimp-live-session.service"
  install -Dm644 "$S/shrimp-live-session-switch.service" "$D/usr/lib/systemd/system/shrimp-live-session-switch.service"
  install -Dm644 "$S/90-shrimp-live-test.rules" "$D/usr/lib/udev/rules.d/90-shrimp-live-test.rules"
  install -Dm644 "$S/shrimp-setup-test.desktop" "$D/etc/xdg/autostart/shrimp-setup-test.desktop"
  # Agama słucha tylko lokalnie; token czytelny dla użytkownika live (kreator i test instalacji).
  install -Dm644 "$HERE/system/systemd/agama-web-server.service.d/50-shrimp.conf" "$D/usr/lib/systemd/system/agama-web-server.service.d/50-shrimp.conf"
  install -Dm644 "$HERE/system/systemd/shrimp-setup-token.service" "$D/usr/lib/systemd/system/shrimp-setup-token.service"
  # Instalacja bez internetu: repozytorium pakietów z nośnika (LiveOS/shrimp-repo.squashfs).
  install -Dm755 "$HERE/system/libexec/shrimp-offline-repo" "$D/usr/libexec/shrimp-offline-repo"
  install -Dm644 "$HERE/system/systemd/shrimp-offline-repo.service" "$D/usr/lib/systemd/system/shrimp-offline-repo.service"
  # Tło jak ekran startu i pulpit testowy bez rozszerzeń. 99_shrimp_live sortuje się po 99_shrimp, więc wygrywa.
  install -Dm644 "$HERE/system/live/shrimp-live-background.svg" "$D/usr/share/shrimp-setup/live/shrimp-live-background.svg"
  install -Dm644 "$HERE/system/live/99_shrimp_live.gschema.override" "$D/usr/share/glib-2.0/schemas/99_shrimp_live.gschema.override"
  # Wi-Fi w kroku Internet bez pytania o hasło: na nośniku nie ma jeszcze konta.
  install -Dm644 "$HERE/system/live/49-shrimp-live-network.rules" "$D/usr/share/polkit-1/rules.d/49-shrimp-live-network.rules"
fi
echo "Setup Shrimp zainstalowany w $D"
