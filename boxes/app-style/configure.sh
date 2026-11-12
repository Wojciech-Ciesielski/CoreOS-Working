# Fragment config.sh boxa app-style (chroot obrazu; składa go shrimp-box).
# Nadpisania globalne flatpak zapisujemy już w obrazie (bez sieci; aplikacji jeszcze nie ma).
# Usługi włącza shrimp-box z [image] enable w box.toml (w obrazie i po instalacji).
/usr/bin/shrimp-app-style || echo "WARNING: shrimp-app-style zakończył się błędem"
flatpak override --system --show >&2 || true
