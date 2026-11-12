# Fragment config.sh boxa linu-x (chroot obrazu; składa go shrimp-box).
# Linu(x): oficjalny sterownik NVIDIA i kodeki przez PipeWire. Skrypty są w /usr/share/shrimp/linux/build
# (pakiety w drivers.conf obok); błąd pakietu albo repozytorium to linia WARNING, nie przerwana budowa.
# build/common dodaje repozytorium budowy z repo.url, gdy KIWI nie zostawiło w obrazie żadnego.
# Trzecia część Linu(x), wygląd aplikacji z Flathub, to box app-style.
if [ -n "${SHRIMP_REPO:-}" ]; then echo "$SHRIMP_REPO/" > /usr/share/shrimp/linux/build/repo.url; fi
for s in /usr/share/shrimp/linux/build/*.sh; do
  echo "config.sh: Linu(x) $(basename "$s")"
  bash "$s"
done
if [ -f /tmp/shrimp-build-oss ]; then
  zypper --non-interactive removerepo shrimp-build-oss || true
fi
rm -rf /usr/share/shrimp/linux/build /tmp/shrimp-build-oss
