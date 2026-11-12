#!/bin/bash
# Instaluje „jeden wygląd aplikacji” (shrimp-app-style i jego usługi) do DESTDIR (obraz KIWI, pakiet, test).
#   boxes/app-style/install.sh DESTDIR
# Usługi włącza shrimp-box ([image] enable w box.toml), nadpisania globalne configure.sh.
set -euo pipefail
D=${1:?użycie: install.sh DESTDIR}
HERE=$(cd "$(dirname "$0")" && pwd)
install -Dm755 "$HERE/shrimp-app-style" "$D/usr/bin/shrimp-app-style"
install -Dm644 "$HERE/shrimp-app-style.service" "$D/usr/lib/systemd/system/shrimp-app-style.service"
install -Dm644 "$HERE/shrimp-app-style.path" "$D/usr/lib/systemd/system/shrimp-app-style.path"
install -Dm644 "$HERE/shrimp-app-style-user.service" "$D/usr/lib/systemd/user/shrimp-app-style-user.service"
echo "shrimp-app-style zainstalowany w $D"
