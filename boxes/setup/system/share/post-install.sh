#!/bin/bash
# Skrypt Agamy po instalacji (scripts.post, chroot: false). System docelowy jest w /mnt.
# Ustawia pierwsze uruchomienie Shrimp: jednorazowe logowanie automatyczne nowego konta,
# znacznik dla shrimp-setup --first-boot, pliki przeniesione przez Shrimp Install i jego wpis UEFI.
set -euo pipefail
T=/mnt
STATE=$T/var/lib/shrimp-setup
SRC=/usr/share/shrimp-setup
log() { echo "shrimp-post-install: $*"; }

mkdir -p "$STATE"

# DNS w chroot: system docelowy nie ma jeszcze własnego resolv.conf (zwykle dowiązanie do /run, którego
# w /mnt nie ma), więc zypper i flatpak w chroot nie widziały sieci („Could not resolve hostname”).
# Na czas skryptu kładziemy kopię z sesji live, na końcu przywracamy to, co było.
RESOLV=$T/etc/resolv.conf
RESOLV_LINK=""
if [ -L "$RESOLV" ]; then RESOLV_LINK=$(readlink "$RESOLV"); rm -f "$RESOLV"
elif [ -e "$RESOLV" ]; then mv "$RESOLV" "$RESOLV.shrimp-orig"; fi
cp -L /etc/resolv.conf "$RESOLV" 2>/dev/null || log "brak /etc/resolv.conf w sesji live"
restore_resolv() {
  rm -f "$RESOLV"
  if [ -n "$RESOLV_LINK" ]; then ln -s "$RESOLV_LINK" "$RESOLV"
  elif [ -e "$RESOLV.shrimp-orig" ]; then mv "$RESOLV.shrimp-orig" "$RESOLV"; fi
}
trap restore_resolv EXIT

USER_NAME=$(sed -n 's/.*"userName":"\([^"]*\)".*/\1/p' "$STATE/install.json")
[ -n "$USER_NAME" ] || { log "brak userName w install.json"; exit 1; }

# Dopóki pakietu shrimp-setup nie ma w repozytorium, kopiujemy Setup z nośnika do systemu (box setup
# jest tylko w kontenerze iso-live). Programy pulpitu (Shrimp Apps, Cocktail, Panko, wygląd aplikacji)
# przychodzą niżej z manifestu kontenera desktop.
if ! chroot "$T" rpm -q shrimp-setup >/dev/null 2>&1; then
  log "kopiuję Setup z nośnika"
  install -Dm755 /usr/bin/shrimp-setup "$T/usr/bin/shrimp-setup"
  install -Dm755 /usr/libexec/shrimp-setup-apply "$T/usr/libexec/shrimp-setup-apply"
  install -Dm755 /usr/libexec/shrimp-usb-backup "$T/usr/libexec/shrimp-usb-backup"
  mkdir -p "$T$SRC" && cp -a "$SRC/ui" "$T$SRC/"
  install -Dm644 /usr/share/polkit-1/actions/org.shrimp.setup.policy "$T/usr/share/polkit-1/actions/org.shrimp.setup.policy"
  install -Dm644 /usr/share/polkit-1/rules.d/50-shrimp-setup.rules "$T/usr/share/polkit-1/rules.d/50-shrimp-setup.rules"
  install -Dm644 /etc/xdg/autostart/shrimp-setup-first-boot.desktop "$T/etc/xdg/autostart/shrimp-setup-first-boot.desktop"
  install -Dm644 /usr/lib/systemd/system/shrimp-nvidia-flatpak.service "$T/usr/lib/systemd/system/shrimp-nvidia-flatpak.service"
  chroot "$T" systemctl enable shrimp-nvidia-flatpak.service || true
fi
# Administratorzy (wheel) widzą migawki w Panko bez stanu „Brak dostępu”.
SNAPCONF=$T/etc/snapper/configs/root
if [ -f "$SNAPCONF" ]; then
  sed -i -e 's/^ALLOW_GROUPS=.*/ALLOW_GROUPS="wheel"/' -e 's/^SYNC_ACL=.*/SYNC_ACL="yes"/' "$SNAPCONF"
  grep -q '^ALLOW_GROUPS=' "$SNAPCONF" || echo 'ALLOW_GROUPS="wheel"' >> "$SNAPCONF"
  grep -q '^SYNC_ACL=' "$SNAPCONF" || echo 'SYNC_ACL="yes"' >> "$SNAPCONF"
fi

# Pakiety GNOME zastąpione przez Shrimp Apps, Cocktail i Flatpaki z zestawu podstawowego: usuwamy je teraz,
# póki system docelowy jest jeszcze zapisywalny, i blokujemy, żeby aktualizacje ich nie przywracały.
# Firefox i aplikacje GNOME wracają jako Flatpaki ze sklepu Shrimp przy pierwszej konfiguracji.
LIST=$SRC/remove-packages.txt
if [ -f "$LIST" ]; then
  while IFS= read -r pkg; do
    case "$pkg" in ''|'#'*) continue;; esac
    chroot "$T" zypper -n --no-refresh rm --no-confirm "$pkg" >/dev/null 2>&1 || true
    chroot "$T" zypper -n al "$pkg" >/dev/null 2>&1 || true
  done < "$LIST"
  log "usunięto zastąpione pakiety GNOME"
fi
# Firefox z Flatpaka jest domyślną przeglądarką, zanim się zainstaluje (wpis nieszkodliwy bez niego).
mkdir -p "$T/etc/xdg"
cat > "$T/etc/xdg/mimeapps.list" <<'MIME'
[Default Applications]
x-scheme-handler/http=org.mozilla.firefox.desktop
x-scheme-handler/https=org.mozilla.firefox.desktop
text/html=org.mozilla.firefox.desktop
application/xhtml+xml=org.mozilla.firefox.desktop
MIME

# Cichy start zainstalowanego systemu: menu GRUB ukryte (Esc, F4 lub F8 w ciągu sekundy je pokazuje,
# pod BIOS także wciśnięty Shift), bez komunikatów „Loading Linux/initial ramdisk” i bez logów jądra.
GRUB_REGEN=0
DEF=$T/etc/default/grub
if [ -f "$DEF" ]; then
  setkey() {
    if grep -q "^$1=" "$DEF"; then sed -i "s|^$1=.*|$1=$2|" "$DEF"; else echo "$1=$2" >> "$DEF"; fi
  }
  setkey GRUB_TIMEOUT 1
  setkey GRUB_TIMEOUT_STYLE hidden
  setkey GRUB_HIDDEN_TIMEOUT_QUIET true
  CMD=$(sed -n 's/^GRUB_CMDLINE_LINUX_DEFAULT="\(.*\)"$/\1/p' "$DEF" | head -n1)
  for o in quiet splash=silent loglevel=3 rd.udev.log_level=3 udev.log_level=3 systemd.show_status=false rd.systemd.show_status=false vt.global_cursor_default=0; do
    case " $CMD " in *" ${o%%=*}="*|*" $o "*) ;; *) CMD="$CMD $o";; esac
  done
  setkey GRUB_CMDLINE_LINUX_DEFAULT "\"${CMD# }\""
  GRUB_REGEN=1
  log "GRUB: ukryte menu, cichy start"
fi
# 10_linux wypisuje echo przed jądrem i initrd; plik jest %config, więc aktualizacja zostawi .rpmnew.
if [ -f "$T/etc/grub.d/10_linux" ]; then
  sed -i "/^[[:space:]]*echo[[:space:]]*'\$(echo \"\$message\" | grub_quote)'/d" "$T/etc/grub.d/10_linux"
  GRUB_REGEN=1
fi

# Nazwa Shrimp w os-release (O systemie) zamiast openSUSE, bez adresów openSUSE.
if [ -x /usr/libexec/shrimp-os-release ]; then
  /usr/libexec/shrimp-os-release "$T" || log "os-release: nie udało się"
  install -Dm755 /usr/libexec/shrimp-os-release "$T/usr/libexec/shrimp-os-release"
fi

# Pulpit Shrimp jak na nośniku. Agama instaluje z repozytorium tylko wzorce i pakiety produktu, więc
# brakowało pakietów, które boxy pulpitu dokładają do obrazu (np. typelib GnomeDesktop dla DING, gvfs,
# wtyczki Plymouth), plików z ich nakładek root/ (motyw Plymouth, czcionki, ustawienia GDM, Linu(x))
# i z ich install.sh (Shrimp Apps, Cocktail, Panko). Listy składa shrimp-box przy budowie obrazu:
# /usr/share/shrimp/containers/desktop.{packages,files,units}.
MAN=/usr/share/shrimp/containers
if [ -f "$MAN/desktop.packages" ]; then
  MISSING=""
  while IFS= read -r p; do
    [ -n "$p" ] || continue
    chroot "$T" rpm -q --whatprovides "$p" >/dev/null 2>&1 || MISSING="$MISSING $p"
  done < "$MAN/desktop.packages"
  if [ -n "$MISSING" ]; then
    log "doinstalowuję pakiety pulpitu:$MISSING"
    # najpierw razem; jeśli któregoś nie ma w repozytorium, po jednym, żeby reszta weszła
    if ! chroot "$T" zypper -n --gpg-auto-import-keys install --no-recommends $MISSING >/dev/null 2>&1; then
      for p in $MISSING; do
        chroot "$T" zypper -n --gpg-auto-import-keys install --no-recommends "$p" >/dev/null 2>&1 || log "pakiet niedostępny: $p"
      done
    fi
  fi
fi
# Wyszukiwanie plików (localsearch-extractor) potrzebuje libz.so.1, którego Leap 16.1 sam nie dokłada.
if [ -x "$T/usr/libexec/localsearch-extractor-3" ] && ! chroot "$T" ldconfig -p | grep -q 'libz\.so\.1 '; then
  for p in libz1 libz-ng-compat1 zlib-ng-compat; do
    chroot "$T" zypper -n --gpg-auto-import-keys install --no-recommends "$p" >/dev/null 2>&1 && { log "doinstalowano $p (libz.so.1)"; break; }
  done
fi
# Pliki boxów pulpitu: nakładki root/ i wszystko, co położyły ich install.sh (programy, usługi, polkit).
if [ -f "$MAN/desktop.files" ]; then
  n=0
  while IFS= read -r f; do
    [ -f "$f" ] || continue
    install -D -m "$(stat -c %a "$f")" "$f" "$T$f" && n=$((n + 1))
  done < "$MAN/desktop.files"
  mkdir -p "$T$MAN" && cp -a "$MAN"/. "$T$MAN"/
  log "pliki pulpitu z nośnika: $n"
fi
# Usługi boxów pulpitu ([image] enable / user_enable w box.toml), jak na nośniku.
if [ -f "$MAN/desktop.units" ]; then
  while read -r kind unit; do
    [ -n "$unit" ] || continue
    if [ "$kind" = user ]; then chroot "$T" systemctl --global enable "$unit" || log "nie można włączyć $unit"
    else chroot "$T" systemctl enable "$unit" || log "nie można włączyć $unit"; fi
  done < "$MAN/desktop.units"
fi
# Pliki, które boxy generują przy budowie obrazu (nie ma ich w nakładkach root/).
for d in /usr/share/plymouth/themes/shrimp /usr/share/fonts/truetype/inter /usr/share/grub2/themes/shrimp; do
  [ -d "$d" ] && { mkdir -p "$T$d"; cp -a "$d"/. "$T$d"/; }
done
# Motyw powłoki i ekranu logowania (gnome-shell-theme.gresource z dodatkami Shrimp) tylko przy tej
# samej wersji gnome-shell, bo zasób z innej wersji psuje powłokę.
RES=/usr/share/gnome-shell/gnome-shell-theme.gresource
if [ -f "$RES" ] && [ "$(rpm -q gnome-shell)" = "$(chroot "$T" rpm -q gnome-shell 2>/dev/null)" ]; then
  cp -a "$RES" "$T$RES" && log "motyw powłoki Shrimp"
else
  log "motyw powłoki: inna wersja gnome-shell w systemie, zostaje domyślny"
fi
chroot "$T" fc-cache -f >/dev/null 2>&1 || true
chroot "$T" dconf update >/dev/null 2>&1 || true
# DING (ikony na pulpicie) wymaga GLibUnix 2.0 z GLib 2.86; bez niego proces DING pada w pętli.
if ! ls "$T"/usr/lib64/girepository-1.0/GLibUnix-2.0.typelib >/dev/null 2>&1; then
  chroot "$T" zypper -n --gpg-auto-import-keys install --no-recommends 'typelib(GLibUnix)' >/dev/null 2>&1 \
    && log "doinstalowano typelib GLibUnix (DING)" || log "brak typelib GLibUnix: DING może nie działać"
fi
# Menu GRUB Shrimp: motyw w /boot (jak na nośniku), GRUB_THEME i nowy grub.cfg.
if [ -d "$T/usr/share/grub2/themes/shrimp" ]; then
  mkdir -p "$T/boot/grub2/themes/shrimp" && cp -a "$T/usr/share/grub2/themes/shrimp"/. "$T/boot/grub2/themes/shrimp"/
  G="$T/etc/default/grub"
  if [ -f "$G" ]; then
    sed -i -e 's|^GRUB_DISTRIBUTOR=.*|GRUB_DISTRIBUTOR="Shrimp"|' -e '/^GRUB_THEME=/d' "$G"
    echo 'GRUB_THEME=/boot/grub2/themes/shrimp/theme.txt' >> "$G"
    grep -q '^GRUB_GFXMODE=' "$G" || echo 'GRUB_GFXMODE=1920x1080,auto' >> "$G"
  fi
  if chroot "$T" grub2-mkconfig -o /boot/grub2/grub.cfg >/dev/null 2>&1; then log "GRUB: motyw shrimp"
  else log "GRUB: grub2-mkconfig nie powiódł się, motyw wejdzie przy następnej aktualizacji GRUB"; fi
fi
# Ekran startu Shrimp: motyw Plymouth i nowy initrd (motyw jest w initrd). plymouth-set-default-theme
# bywa nieobecny w systemie (osobny pakiet skryptów), wtedy wpis w plymouthd.conf.
if [ -d "$T/usr/share/plymouth/themes/shrimp" ]; then
  if ! chroot "$T" plymouth-set-default-theme shrimp >/dev/null 2>&1; then
    mkdir -p "$T/etc/plymouth"
    printf '[Daemon]\nTheme=shrimp\n' > "$T/etc/plymouth/plymouthd.conf"
  fi
  if chroot "$T" dracut -f --regenerate-all >/dev/null 2>&1; then log "Plymouth: motyw shrimp"
  else log "Plymouth: dracut nie powiódł się, motyw wejdzie przy następnej aktualizacji jądra"; fi
else
  log "Plymouth: brak motywu shrimp na nośniku"
fi

# Pulpit Shrimp jak na nośniku: rozszerzenia (Menu Shrimp, dok…), ustawienia domyślne
# (99_shrimp.gschema.override), tapeta i logo. Leap nie ma tych rozszerzeń w pakietach.
log "kopiuję wygląd pulpitu Shrimp"
EXT=/usr/share/gnome-shell/extensions
SCH=/usr/share/glib-2.0/schemas
for d in "$EXT"/*/; do
  uuid=$(basename "$d")
  [ -e "$T$EXT/$uuid" ] || cp -a "$d" "$T$EXT/$uuid"
done
for f in "$SCH"/org.gnome.shell.extensions.*.gschema.xml; do
  [ -f "$f" ] && { [ -e "$T$f" ] || install -Dm644 "$f" "$T$f"; }
done
install -Dm644 "$SCH/99_shrimp.gschema.override" "$T$SCH/99_shrimp.gschema.override"
for p in /usr/share/backgrounds/shrimp /usr/share/gnome-background-properties/shrimp.xml /usr/share/shrimp/logo \
         /usr/share/icons/hicolor/scalable/apps/shrimp-logo.svg /usr/share/icons/hicolor/scalable/apps/shrimp-logo-dark.svg \
         /usr/share/icons/hicolor/scalable/apps/shrimp-logo-symbolic.svg /usr/share/icons/MoreWaita; do
  [ -e "$p" ] && { mkdir -p "$T$(dirname "$p")"; cp -a "$p" "$T$(dirname "$p")/"; }
done
chroot "$T" glib-compile-schemas "$SCH" || log "glib-compile-schemas nie powiodło się"
chroot "$T" gtk-update-icon-cache -f /usr/share/icons/hicolor >/dev/null 2>&1 || true
chroot "$T" gtk-update-icon-cache -f /usr/share/icons/MoreWaita >/dev/null 2>&1 || true

# GRUB w języku konta: nazwa wpisu „Computer settings” (boxes/grub-theme/i18n, motyw Shrimp: lang/<znacznik>.cfg).
# Wpis firmware GRUB-a w docelowym systemie pochodzi z pakietu (angielski), więc podmieniamy go tutaj.
grub_tag() {
  local l="${1%%.*}" mod="" p r
  case "$l" in *@*) mod="${l#*@}"; l="${l%%@*}";; esac
  p="${l%%_*}"; r=""; case "$l" in *_*) r="${l#*_}";; esac
  case "$p" in
    zh) case "$r" in TW|HK|MO) echo zh-hant;; *) echo zh-hans;; esac;;
    pt) [ "$r" = PT ] && echo pt-pt || echo pt-br;;
    sr) [ "$mod" = latin ] && echo sr-latn || echo sr-cyrl;;
    uz) [ "$mod" = cyrillic ] && echo uz-cyrl || echo uz;;
    be) [ "$mod" = latin ] && echo be-latn || echo be;;
    ca) [ "$mod" = valencia ] && echo ca-valencia || echo ca;;
    no) echo nb;;
    *) echo "$p";;
  esac
}
SYS_LANG=$(sed -n 's/^LANG=//p' "$T/etc/locale.conf" 2>/dev/null | tr -d '"' | head -n1)
TAG=$(grub_tag "${SYS_LANG:-en}")
FW=$(sed -n 's/^set shrimp_fw="\(.*\)"$/\1/p' "/usr/share/grub2/themes/shrimp/lang/$TAG.cfg" 2>/dev/null | head -n1)
if [ -n "$FW" ] && [ -f "$T/etc/grub.d/30_uefi-firmware" ]; then
  sed -i "s|UEFI Firmware Settings|$FW|" "$T/etc/grub.d/30_uefi-firmware"
  log "GRUB: wpis firmware w języku $TAG"
  GRUB_REGEN=1
fi
if [ "$GRUB_REGEN" = 1 ]; then
  chroot "$T" grub2-mkconfig -o /boot/grub2/grub.cfg >/dev/null 2>&1 || log "grub2-mkconfig nie powiodło się (zmiany GRUB wejdą przy jego następnej aktualizacji)"
fi

# Pierwsza konfiguracja: znacznik + jednorazowe logowanie automatyczne (zdejmuje je shrimp-setup-apply).
touch "$STATE/first-boot"
if [ -f "$T/etc/sysconfig/displaymanager" ]; then
  sed -i "s/^DISPLAYMANAGER_AUTOLOGIN=.*/DISPLAYMANAGER_AUTOLOGIN=\"$USER_NAME\"/" "$T/etc/sysconfig/displaymanager"
fi
mkdir -p "$T/etc/gdm"
cat > "$T/etc/gdm/custom.conf" <<CONF
# Shrimp: logowanie automatyczne tylko na pierwszą konfigurację (zdejmuje je shrimp-setup-apply).
[daemon]
AutomaticLoginEnable=True
AutomaticLogin=$USER_NAME
CONF

# Bez gnome-initial-setup: jego kroki robi Setup Shrimp.
HOME_DIR=$(chroot "$T" getent passwd "$USER_NAME" | cut -d: -f6)
OWNER=$(chroot "$T" id -u "$USER_NAME"):$(chroot "$T" id -g "$USER_NAME")
install -d -o "${OWNER%:*}" -g "${OWNER#*:}" "$T$HOME_DIR/.config"
touch "$T$HOME_DIR/.config/gnome-initial-setup-done"
chown "$OWNER" "$T$HOME_DIR/.config/gnome-initial-setup-done"

# Bez okna powitalnego openSUSE (zasłaniało Setup przy pierwszym uruchomieniu, test w QEMU).
for f in "$T"/usr/etc/xdg/autostart/*welcome*.desktop "$T"/etc/xdg/autostart/*welcome*.desktop; do
  [ -f "$f" ] || continue
  n=$(basename "$f")
  printf '[Desktop Entry]\nType=Application\nName=%s\nHidden=true\n' "${n%.desktop}" > "$T/etc/xdg/autostart/$n"
  log "ukryto autostart $n"
done

# Flathub dla całego systemu (instalacja programów w kroku Programy).
chroot "$T" flatpak remote-add --system --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo || \
  log "Flathub zostanie dodany przy pierwszej konfiguracji"

# Pliki i lista programów z Shrimp Install (shrimp-transfer/ na nośniku): foldery Windows trafiają do
# folderów XDG nowego konta w jego języku, pliki podzielone (> 4 GB) są sklejane, shrimp.json idzie do $STATE.
if /usr/bin/shrimp-setup --restore --root "$T" --user "$USER_NAME"; then :; else
  log "bez plików z Shrimp Install"
fi

# Dawna zawartość pendrive'a (shrimp-backup/ na nośniku): sprawdzona kopia na dysku komputera. Przywróci ją
# pierwsza konfiguracja (formatuje pendrive i wgrywa pliki z powrotem). Brak shrimp-backup to nie błąd.
/usr/libexec/shrimp-usb-backup stash --root "$T" || log "bez kopii zawartości pendrive'a (kod $?)"

# Jednorazowy wpis UEFI „Shrimp Install” (BootNext z Windows) nie jest już potrzebny.
if command -v efibootmgr >/dev/null && [ -d /sys/firmware/efi ]; then
  for B in $(efibootmgr | sed -n 's/^Boot\([0-9A-Fa-f]\{4\}\)[* ]*Shrimp Install\b.*/\1/p'); do
    efibootmgr -q -b "$B" -B && log "usunięto wpis UEFI Boot$B (Shrimp Install)"
  done
fi
log "gotowe"
