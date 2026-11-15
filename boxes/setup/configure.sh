# Fragment config.sh boxa setup (chroot obrazu; składa go shrimp-box). BOX_ARGS = argumenty z kontenera.
[ -x /usr/bin/shrimp-setup ] || die "shrimp-setup missing (box setup was not installed)"
# Karta Wi-Fi, która nie wstała po restarcie z Windows (MT7921): na nośniku i w zainstalowanym systemie.
systemctl enable shrimp-wifi-revive.service
if [[ " $BOX_ARGS " == *" --live "* ]]; then
  # Kreator na nośniku: Agama (zaplecze instalacji), token dla użytkownika live, sesja kiosku wybrana przed GDM.
  [ -f /usr/share/gnome-session/sessions/shrimp-setup.session ] || die "Setup kiosk session missing"
  systemctl enable agama.service agama-web-server.service shrimp-setup-token.service shrimp-offline-repo.service \
    shrimp-live-session.service
  # gnome-kiosk i tłumaczenia nazw krajów/języków (iso-codes-lang): miękko, z repozytorium Leap. Bez gnome-kiosk
  # shrimp-live-session wybiera pulpit GNOME, a kreator startuje z autostartu (WARNING w dzienniku budowy).
  VERSION_ID="$(. /etc/os-release && echo "$VERSION_ID")"
  LEAP="https://download.opensuse.org/distribution/leap/${VERSION_ID}/repo/oss"
  SETUP_REPO=""
  for candidate in "${SHRIMP_REPO:-}" "$LEAP/x86_64" "$LEAP"; do
    [ -n "$candidate" ] || continue
    if curl -fsSLI "$candidate/repodata/repomd.xml" >/dev/null 2>&1; then SETUP_REPO="$candidate"; break; fi
  done
  if [ -n "$SETUP_REPO" ]; then
    zypper -n --gpg-auto-import-keys addrepo -f "$SETUP_REPO" shrimp-build-setup
    zypper -n --gpg-auto-import-keys refresh shrimp-build-setup
    for p in gnome-kiosk iso-codes-lang; do
      if rpm -q "$p" >/dev/null 2>&1 || zypper -n --gpg-auto-import-keys install --no-recommends "$p" </dev/null >/dev/null 2>&1; then
        echo "optional package installed: $p"
      else
        echo "WARNING: Setup: package $p not available"
      fi
    done
    zypper -n removerepo shrimp-build-setup || true
    zypper -n clean -a
  else
    echo "WARNING: Setup: Leap ${VERSION_ID} repository not reachable, gnome-kiosk skipped"
  fi
  # Ustawienia live (czarne tło z logo, pulpit testowy bez rozszerzeń): 99_shrimp_live.gschema.override.
  [ -f /usr/share/glib-2.0/schemas/99_shrimp_live.gschema.override ] || die "live schema override missing"
  glib-compile-schemas /usr/share/glib-2.0/schemas
fi
