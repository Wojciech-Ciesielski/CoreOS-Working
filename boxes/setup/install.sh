#!/bin/bash
# Buduje Setup Shrimp i instaluje go do DESTDIR (pakiet, obraz KIWI albo test).
#   boxes/setup/install.sh DESTDIR [--live]
# --live dodaje części tylko dla nośnika: autostart instalatora, interfejs w Agamie, token.
# Wymaga: node 22+, cargo, pakiety deweloperskie gtk4 i webkitgtk-6.0.
set -euo pipefail
D=${1:?użycie: install.sh DESTDIR [--live]}
LIVE=${2:-}
HERE=$(cd "$(dirname "$0")" && pwd)

(cd "$HERE/web" && npm ci && npm run build)
# SHRIMP_PREBUILT=1: okno Rust zbudowane wcześniej (krok chroot shrimp-box, te same biblioteki co obraz).
if [ -z "${SHRIMP_PREBUILT:-}" ]; then
  (cd "$HERE/shell" && cargo build --release --locked)
fi

install -Dm755 "$HERE/shell/target/release/shrimp-setup" "$D/usr/bin/shrimp-setup"
install -d "$D/usr/share/shrimp-setup/ui"
cp -a "$HERE/web/dist/." "$D/usr/share/shrimp-setup/ui/"
install -Dm755 "$HERE/system/share/post-install.sh" "$D/usr/share/shrimp-setup/post-install.sh"
install -Dm644 "$HERE/system/share/remove-packages.txt" "$D/usr/share/shrimp-setup/remove-packages.txt"
install -Dm755 "$HERE/system/libexec/shrimp-setup-apply" "$D/usr/libexec/shrimp-setup-apply"
install -Dm755 "$HERE/system/libexec/shrimp-usb-backup" "$D/usr/libexec/shrimp-usb-backup"
install -Dm644 "$HERE/system/polkit/org.shrimp.setup.policy" "$D/usr/share/polkit-1/actions/org.shrimp.setup.policy"
install -Dm644 "$HERE/system/polkit/50-shrimp-setup.rules" "$D/usr/share/polkit-1/rules.d/50-shrimp-setup.rules"
install -Dm644 "$HERE/system/autostart/shrimp-setup-first-boot.desktop" "$D/etc/xdg/autostart/shrimp-setup-first-boot.desktop"
install -Dm644 "$HERE/system/systemd/shrimp-nvidia-flatpak.service" "$D/usr/lib/systemd/system/shrimp-nvidia-flatpak.service"
install -Dm644 "$HERE/system/agama/shrimp.yaml" "$D/usr/share/agama/products.d/shrimp.yaml"
# Strażnik Setup (rozszerzenie GNOME Shell): w sesji live i w pierwszej konfiguracji nie ma przeglądu ani pulpitu.
install -d "$D/usr/share/gnome-shell/extensions"
cp -a "$HERE/system/gnome-shell/shrimp-setup-guard@shrimp" "$D/usr/share/gnome-shell/extensions/"
chmod -R a+rX "$D/usr/share/gnome-shell/extensions/shrimp-setup-guard@shrimp"
# Pakiety kodeków i NVIDIA (drivers.conf) daje box linu-x, wygląd aplikacji box app-style.

if [ "$LIVE" = --live ]; then
  install -Dm644 "$HERE/system/autostart/shrimp-setup-install.desktop" "$D/etc/xdg/autostart/shrimp-setup-install.desktop"
  install -Dm644 "$HERE/system/autostart/shrimp-setup-launcher.desktop" "$D/usr/share/applications/org.shrimp.Setup.desktop"
  install -Dm644 "$HERE/system/systemd/agama-web-server.service.d/50-shrimp.conf" "$D/usr/lib/systemd/system/agama-web-server.service.d/50-shrimp.conf"
  install -Dm644 "$HERE/system/systemd/shrimp-setup-token.service" "$D/usr/lib/systemd/system/shrimp-setup-token.service"
  # Spokojny start nośnika: czarne tło z logo, w sesji live tylko strażnik Setup (bez DING i doku).
  # Nazwa 99_shrimp_live sortuje się po 99_shrimp, więc ten plik wygrywa z ustawieniami zainstalowanego systemu.
  install -Dm644 "$HERE/system/live/shrimp-live-background.svg" "$D/usr/share/shrimp-setup/live/shrimp-live-background.svg"
  install -Dm644 "$HERE/system/live/99_shrimp_live.gschema.override" "$D/usr/share/glib-2.0/schemas/99_shrimp_live.gschema.override"
  # Wi-Fi w kroku „Połącz z internetem” bez pytania o hasło: na nośniku nie ma jeszcze konta.
  install -Dm644 "$HERE/system/live/49-shrimp-live-network.rules" "$D/usr/share/polkit-1/rules.d/49-shrimp-live-network.rules"
fi
echo "Setup Shrimp zainstalowany w $D"
