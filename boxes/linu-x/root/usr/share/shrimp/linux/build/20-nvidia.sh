#!/bin/bash
# Linu(x) 1/3: oficjalny sterownik NVIDIA na nośniku i w zainstalowanym systemie.
# Moduł jądra: otwarte moduły NVIDIA zbudowane i podpisane przez SUSE (repozytorium Leap),
# więc ładują się przy włączonym Secure Boot bez MOK. Biblioteki (GL, Vulkan, VA-API/NVDEC,
# CUDA): repozytorium NVIDIA dla openSUSE, które zostaje w systemie i daje aktualizacje.
# Karty sprzed Turinga zostają przy nouveau: decyduje /usr/libexec/shrimp-gpu-gate.
set -uo pipefail
. /usr/share/shrimp/linux/build/common
ensure_repos || { warn "sterownik NVIDIA pominięty"; exit 0; }

. /usr/lib/os-release
repo=""; repo_cfg=""
for v in "$VERSION_ID" "${VERSION_ID%%.*}.0"; do
  url="$NVIDIA_REPO_BASE/$v/"
  if curl -fsSLI "${url}repodata/repomd.xml" >/dev/null 2>&1; then
    repo="$url"
    # repozytorium dla tej samej wersji: adres z $releasever, żeby szło za aktualizacją Leap
    if [ "$v" = "$VERSION_ID" ]; then repo_cfg="$NVIDIA_REPO_BASE/\$releasever/"; else repo_cfg="$url"; fi
    break
  fi
done
if [ -z "$repo" ]; then
  warn "repozytorium NVIDIA dla Leap $VERSION_ID niedostępne; sterownik NVIDIA pominięty"
  exit 0
fi
"${ZYP[@]}" addrepo --refresh --priority 90 "$repo_cfg" NVIDIA
"${ZYP[@]}" refresh NVIDIA || { warn "nie można odświeżyć repozytorium NVIDIA"; exit 0; }
note "repozytorium NVIDIA: $repo"

. /usr/share/shrimp/linux/nvidia-open.sh

installed=""
for S in $NVIDIA_OPEN_SERIES; do
  # Najpierw sam moduł (meta-pakiet wymaga nvidia-userspace-meta w DOKŁADNIE tej samej wersji, więc
  # zypper dobiera biblioteki pasujące do modułu), w najnowszej wersji, do której SUSE ma podpisany
  # moduł: repozytorium NVIDIA bywa o wersję przed SUSE (nvidia-open.sh).
  note "seria $S: instaluję otwarty moduł (wersje: $(nvidia_open_versions "$S" | head -n3 | tr '\n' ' '))"
  if ! ver=$(nvidia_open_install "$S"); then
    warn "seria $S nie zainstalowała się, próbuję następnej"
    continue
  fi
  note "seria $S: moduł $ver"
  # brakujące biblioteki z listy, w wersji modułu
  for p in $(echo "$NVIDIA_OPEN_PACKAGES" | sed "s/@S@/$S/g"); do
    case "$p" in *kmp*) continue ;; esac
    rpm -q "$p" >/dev/null 2>&1 && continue
    "${ZYP[@]}" install --no-recommends --auto-agree-with-licenses "$p = $ver" >/dev/null 2>&1 \
      || warn "seria $S: nie udało się doinstalować $p $ver"
  done
  installed=$S; break
done
if [ -z "$installed" ]; then
  warn "żadna seria otwartego sterownika NVIDIA ($NVIDIA_OPEN_SERIES) się nie zainstalowała"
  exit 0
fi
note "sterownik NVIDIA $installed zainstalowany: $(rpm -qa 'nvidia-open-driver*' 'nvidia-userspace*' 'nvidia-gl*' 'nvidia-video*' 'nvidia-compute*' | tr '\n' ' ')"

# Pakiety NVIDIA wyłączają nouveau całkowicie (blacklist). Na nośniku dla każdego sprzętu
# o nouveau decyduje shrimp-gpu-gate (Turing+ → nvidia, starsze → nouveau), więc blacklist
# zamieniamy na tę bramkę. Pliki z /usr/lib nadpisujemy kopią w /etc o tej samej nazwie.
for f in /etc/modprobe.d/*.conf /usr/lib/modprobe.d/*.conf; do
  [ -f "$f" ] || continue
  grep -qE '^[[:space:]]*blacklist[[:space:]]+nouveau' "$f" || continue
  dst="/etc/modprobe.d/$(basename "$f")"
  [ "$f" = "$dst" ] || cp "$f" "$dst"
  sed -i -E 's/^([[:space:]]*blacklist[[:space:]]+nouveau.*)$/# \1  (Shrimp: decyduje shrimp-gpu-gate)/' "$dst"
  sed -i -E 's/^([[:space:]]*options[[:space:]]+nouveau[[:space:]]+modeset=0.*)$/# \1/' "$dst"
  note "nouveau odblokowane w $dst"
done
cat > /etc/modprobe.d/60-shrimp-gpu.conf <<'CONF'
# Shrimp Linu(x): karty NVIDIA od Turinga (GTX 16xx, RTX 20xx i nowsze) dostają oficjalny
# sterownik, starsze nouveau. Wymuszenie nouveau: parametr jądra shrimp.nouveau=1.
# Opcje sterownika: /usr/lib/modprobe.d/61-shrimp-nvidia.conf.
install nouveau /usr/libexec/shrimp-gpu-gate
CONF
# GDM włącza Wayland na NVIDIA tylko z usługami uśpienia sterownika.
for u in $NVIDIA_SERVICES; do
  systemctl enable "$u" 2>/dev/null || warn "nie można włączyć $u"
done
echo "$installed" > /usr/share/shrimp/linux/nvidia-series
exit 0
