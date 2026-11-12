#!/bin/bash
# Linu(x) 2b: sterowniki i firmware urządzeń (lista i powody w drivers.conf, DRIVER_PACKAGES).
# Brak pakietu w repozytorium to linia WARNING, nie przerwana budowa.
set -uo pipefail
. /usr/share/shrimp/linux/build/common
ensure_repos || exit 0
# shellcheck disable=SC2086
install_each $DRIVER_PACKAGES
# Modemy i Wi-Fi USB przełączane z trybu „płyty CD” od razu po podłączeniu.
systemctl enable ModemManager.service 2>/dev/null || warn "nie udało się włączyć ModemManager"
exit 0
