#!/bin/bash
# Repozytorium pakietów na nośniku: instalacja Shrimp bez internetu (Beta 5).
# Uruchamiany przez tools/build-iso.sh w kontenerze Tumbleweed, po KIWI.
#   tools/build-offline-repo.sh LEAP_VERSION ISO LIVE_PACKAGES REPO_URL
#
# 1. install/: oficjalne repozytorium z obrazu „Leap offline installer” (podpisane przez openSUSE, to samo,
#    z którego instaluje się Leap bez sieci). Z niego Agama instaluje system (shrimp-offline-repo).
# 2. extra/: pakiety sesji live, których nie ma w install/ (pulpit Shrimp, czcionki, kodeki, sterowniki):
#    post-install.sh doinstalowuje je w chroot bez internetu.
# Oba katalogi idą do LiveOS/shrimp-repo.squashfs wewnątrz ISO. Shrimp Install kopiuje LiveOS/ na exFAT,
# więc ten plik (> 3 GB) nie trafia na FAT32.
set -euo pipefail
V=$1; ISO=$2; LIVE_PKGS=$3; REPO=$4
W=/var/tmp/shrimp-offline
rm -rf "$W"; mkdir -p "$W/stage"
log() { echo "offline-repo: $*"; }

base="https://download.opensuse.org/distribution/leap/$V/offline"
name="Leap-$V-offline-installer-x86_64.install.iso"
LEAP="$W/$name"
# Pamięć podręczna (actions/cache w Build ISO): rozpakowane install/ z poprzedniej budowy, jeśli suma obrazu
# openSUSE się nie zmieniła. Oszczędza pobranie 4,5 GB i rozpakowanie przy każdej budowie.
CACHE=${SHRIMP_CACHE:-cache}/leap-$V
INST=$CACHE/install
sum="" alg=""
for a in sha256 sha512; do
  if s=$(curl -fsSL "$base/$name.$a" 2>/dev/null | awk '{print $1; exit}') && [ -n "$s" ]; then sum=$s; alg=$a; break; fi
done
if [ -n "$sum" ] && [ "$(cat "$CACHE/iso.sum" 2>/dev/null)" = "$sum" ] && [ -f "$INST/repodata/repomd.xml.asc" ]; then
  log "install/ z pamięci podręcznej ($name, $alg ${sum:0:12}…)"
else
  rm -rf "$CACHE"; mkdir -p "$CACHE"
  for _ in 1 2 3; do curl -fsSL --retry 3 -o "$LEAP" "$base/$name" && break; sleep 20; done
  if [ -n "$sum" ]; then echo "$sum  $LEAP" | "${alg}sum" -c - || { log "zła suma $alg"; exit 1; }
  else log "UWAGA: brak sumy kontrolnej obrazu $name"; fi
  xorriso -osirrox on -indev "$LEAP" -extract /install "$INST" >/dev/null 2>&1
  chmod -R u+w,a+rX "$INST"
  rm -f "$LEAP"
  [ -f "$INST/repodata/repomd.xml.asc" ] || { log "brak podpisanego repodata w $name"; exit 1; }
  [ -n "$sum" ] && echo "$sum" > "$CACHE/iso.sum"
fi
chmod -R a+rX "$CACHE"
log "install/: $(find "$INST" -name '*.rpm' | wc -l) pakietów, $(du -sh "$INST" | cut -f1)"

# Pakiety live spoza install/ (nazwy; wersje z repozytorium Leap, z którego zbudowano obraz).
find "$INST" -name '*.rpm' ! -name '*.src.rpm' -print0 | xargs -0 rpm -qp --nosignature --qf '%{NAME}\n' 2>/dev/null | sort -u > "$W/have.txt"
grep -v '^gpg-pubkey$' "$LIVE_PKGS" | sort -u | comm -23 - "$W/have.txt" > "$W/missing.txt"
log "extra/: $(wc -l < "$W/missing.txt") pakietów spoza install/"
R="$W/root"; mkdir -p "$R"
Z=(zypper -n --root "$R" --gpg-auto-import-keys)
"${Z[@]}" ar -f "$REPO" leap >/dev/null
"${Z[@]}" ref >/dev/null
mkdir -p "$W/stage/extra"
# zypper download bez zależności (zależności pakietów live też są w live, więc są w install/ albo tutaj)
if [ -s "$W/missing.txt" ]; then
  xargs -a "$W/missing.txt" "${Z[@]}" download >/dev/null 2>&1 || {
    while read -r p; do "${Z[@]}" download "$p" >/dev/null 2>&1 || log "niedostępny: $p"; done < "$W/missing.txt"
  }
  find "$R/var/cache/zypp/packages" -name '*.rpm' -exec mv -t "$W/stage/extra" {} +
fi
createrepo_c -q "$W/stage/extra"
# extra/ podpisane jednorazowym kluczem tej budowy: Agama potrzebuje z niego pakietów, które sama
# dokłada (np. exfatprogs, bo pendrive ma partycję exFAT), a niepodpisane repozytorium kończy się
# pytaniem, na które nikt nie odpowiada. shrimp-offline-repo ufa temu kluczowi (repomd.xml.key).
export GNUPGHOME="$W/gnupg"; mkdir -m 700 "$GNUPGHOME"
gpg --batch --pinentry-mode loopback --passphrase '' --quick-gen-key "Shrimp offline repository (build $(date -u +%Y%m%d))" rsa3072 sign 1y
gpg --batch --yes --armor --detach-sign -o "$W/stage/extra/repodata/repomd.xml.asc" "$W/stage/extra/repodata/repomd.xml"
gpg --batch --armor --export > "$W/stage/extra/repodata/repomd.xml.key"
unset GNUPGHOME
log "extra/: $(find "$W/stage/extra" -name '*.rpm' | wc -l) pakietów, $(du -sh "$W/stage/extra" | cut -f1)"
rm -rf "$R"

# Paczki RPM są już skompresowane: squashfs bez ponownej kompresji danych (szybciej, ten sam rozmiar).
SQ=out/shrimp-repo.squashfs
rm -f "$SQ"
# Dwa źródła: install/ prosto z pamięci podręcznej (bez kopiowania 3,5 GB) i extra/ z tej budowy.
mksquashfs "$INST" "$W/stage/extra" "$SQ" -noappend -no-xattrs -all-root -noD -quiet
rm -rf "$W"
log "squashfs: $(du -h "$SQ" | cut -f1)"

# Do ISO jako LiveOS/shrimp-repo.squashfs; rozruch (El Torito, EFI, hybryda MBR/GPT) bez zmian.
# Poziom 3 ISO 9660: plik może mieć ponad 4 GiB.
OUT="$ISO.offline"
xorriso -indev "$ISO" -outdev "$OUT" -compliance iso_9660_level=3 \
  -map "$SQ" /LiveOS/shrimp-repo.squashfs -boot_image any replay
mv "$OUT" "$ISO"
rm -f "$SQ"
xorriso -indev "$ISO" -lsl /LiveOS 2>/dev/null
log "ISO: $(du -h "$ISO" | cut -f1)"
