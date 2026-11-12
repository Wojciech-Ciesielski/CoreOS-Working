# shellcheck shell=bash
# Linu(x): instalacja otwartego modułu NVIDIA (wspólna dla budowy nośnika, build/20-nvidia.sh,
# i zainstalowanego systemu, shrimp-setup-apply). Plik jest dołączany przez „.”, nic nie robi sam.
#
# Dlaczego nie po prostu „zypper install nvidia-open-driver-G07-signed-kmp-meta”:
# meta-pakiet jest w repozytorium NVIDIA i wymaga modułu podpisanego przez SUSE w DOKŁADNIE tej
# samej wersji (nvidia-open-driver-G07-signed-kmp = 595.112.07). NVIDIA wydaje nową wersję wcześniej,
# niż SUSE zbuduje do niej moduł, więc najnowszy meta-pakiet często nie ma modułu i instalacja padała
# (Beta 3.2: 595.112.07 bez modułu, w Beta 3.1 była 595.99.02 z modułem). Bierzemy najnowszą wersję,
# która w repozytoriach ma też moduł.
#
# Wymaga: ZYP (tablica polecenia zypper) albo zwykłego zypper z --non-interactive.

if ! declare -p ZYP >/dev/null 2>&1; then ZYP=(zypper --non-interactive); fi

# nvidia_open_versions S: wersje (wersja-wydanie) meta-pakietu serii S w repozytoriach, od najnowszej.
nvidia_open_versions() {
  local name="nvidia-open-driver-$1-signed-kmp-meta"
  "${ZYP[@]}" -q search -s -x -t package "$name" 2>/dev/null \
    | awk -F'|' -v n="$name" '{ for (i = 1; i <= NF; i++) gsub(/^[ \t]+|[ \t]+$/, "", $i) }
                              $2 == n && $4 != "" { print $4 }' \
    | sort -u -V -r
}

# nvidia_open_install S: instaluje moduł serii S w najnowszej wersji, która się rozwiązuje (moduł SUSE
# + biblioteki NVIDIA w tej samej wersji). Wypisuje zainstalowaną wersję (bez wydania). Gdy repozytoria
# nie mają meta-pakietu, próbuje samego modułu nvidia-open-driver-S-signed-kmp-default.
nvidia_open_install() {
  local S=$1 v out tried=0
  for v in $(nvidia_open_versions "$S"); do
    tried=$((tried + 1))
    # nieudana próba niczego nie zmienia: zypper najpierw rozwiązuje zależności
    if out=$("${ZYP[@]}" install --no-recommends --auto-agree-with-licenses \
               "nvidia-open-driver-$S-signed-kmp-meta = $v" 2>&1); then
      echo "${v%-*}"
      return 0
    fi
    echo "Linu(x): NVIDIA $S $v bez pasującego modułu, próbuję starszej wersji" >&2
    echo "$out" | grep -iE 'nothing provides|problem' | head -3 | sed 's/^/Linu(x): zypper: /' >&2
    [ "$tried" -ge 8 ] && break
  done
  # Repozytoria w ogóle bez meta-pakietu serii: sam moduł SUSE, biblioteki w jego wersji dobiera
  # wywołujący. Gdy meta-pakiety są, ale żaden nie ma modułu, sam moduł nic nie da (bez bibliotek).
  [ "$tried" -eq 0 ] || return 1
  local kmp="nvidia-open-driver-$S-signed-kmp-default" ver
  "${ZYP[@]}" install --no-recommends --auto-agree-with-licenses "$kmp" >/dev/null 2>&1 || return 1
  ver=$(rpm -q --qf '%{VERSION}\n' "$kmp" 2>/dev/null | head -n1 | sed 's/_k.*//')
  [ -n "$ver" ] || return 1
  echo "$ver"
}
