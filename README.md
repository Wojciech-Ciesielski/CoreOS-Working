# CoreOS-Working

Repozytorium robocze obrazu **Shrimp**: ISO (KIWI na openSUSE Leap), pulpit GNOME, motywy, Linu(x), Setup i sesja live. Tu powstaje wydanie Shrimp (ISO).

## Repozytoria Shrimp

| Repozytorium | Co | Wydania |
|---|---|---|
| **CoreOS-Working** (to) | obraz ISO, pulpit, Setup | `beta-N` „Shrimp Beta N”: ISO w Cloudflare R2, w GitHub sumy i manifest |
| [Shrimps-Working](https://github.com/Wojciech-Ciesielski/Shrimps-Working) | Shrimp Apps, Cocktail, Panko | „Shrimps Beta”: tag, z którego CoreOS buduje aplikacje |
| [Install-Working](https://github.com/Wojciech-Ciesielski/Install-Working) | Shrimp Install (.exe) | „Shrimp Install Beta”: `.exe` |
| [Site-Working](https://github.com/Wojciech-Ciesielski/Site-Working) | strona shrimp-organization.dev | bez wydań (Cloudflare Pages) |

Jedyna zależność w kodzie: CoreOS bierze Shrimps z wersji przypiętej w `tools/shrimps.ref` (`tools/fetch-shrimps.sh`, wywoływany przez `tools/build-iso.sh` i workflow **Boxy**). Shrimp Install i CoreOS łączy tylko kontrakt `shrimp.json` (box `install-schema`, źródło w Install-Working) i adres `manifest.json` w R2.

Kopie wspólnych boxów (zmieniaj najpierw w źródle): `shrimp-box`, `i18n-core` → źródło tutaj; `install-schema` → Install-Working; `help-links` → Site-Working.

## Wydanie (R2)

Workflow **Release** buduje ISO (albo bierze gotowy przebieg Build ISO z tego samego commita), wysyła obraz, `SHA256SUMS` i `manifest.json` do Cloudflare R2 (`<tag>/…`, a `manifest.json` w korzeniu bucketu wskazuje ostatnie wydanie) i tworzy wydanie GitHub z sumami i manifestem. Wersja musi być równa `boxes/branding/root/usr/share/shrimp/version`. Ustawienia: sekrety `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, zmienne `R2_BUCKET`, `R2_PUBLIC_URL`.

## Struktura: boxy i kontenery

Cały kod leży w boxach (`boxes/<id>/` z plikiem `box.toml`: identyfikator, zależności, pakiety, budowa), a kontenery (`containers/`) mówią, które boxy tworzą produkt: `iso-live` (nośnik) i `desktop` (pulpit). Opis i zasady: [`boxes/README.md`](boxes/README.md). Sprawdzenie: `cd boxes/shrimp-box && cargo run -- check` (workflow **Boxy**).

Zasada: żaden PR nie dodaje zrzutów ekranu ani `test-results/`. Pilnuje tego workflow **Bez zrzutów** (`tools/bez-zrzutow.sh`, można uruchomić lokalnie przed pushem).

## ISO (Shrimp Beta, live)

- Opis obrazu KIWI składa `shrimp-box image iso-live` z boxów kontenera `iso-live`: szablon `boxes/image-base/config.kiwi` + pakiety boxów, `config.sh` z fragmentów `boxes/*/configure.sh`, nakładka z `boxes/*/root/`.
- Budowa: workflow **Build ISO** w GitHub Actions (artefakt `shrimp-live-iso`) albo lokalnie `sudo tools/build-iso.sh 16.1` (Docker/Podman).
- Użytkownik live: `shrimp`, hasło `shrimp` (logowanie automatyczne).
- Shrimp Setup jest częścią obrazu (box `setup` z argumentem `--live`): shrimp-box buduje okno w chroocie Leap (te same biblioteki co ISO) i instaluje Setup (`boxes/setup/install.sh --live`) do opisu KIWI w `out/desc`; Agama 24 serwuje interfejs na `127.0.0.1:80`, a okno startuje z autostartu sesji live.
- Menu systemowe: logo Shrimp z lewej strony górnego paska (box `shrimp-menu`, rozszerzenie `shrimp-menu@shrimp`): O tym komputerze, Ustawienia, Oprogramowanie (Shrimp Apps), Aktualizacje (Cocktail; do czasu jego dodania strona Zainstalowane w Shrimp Apps), Przywracanie systemu (Panko), narzędzia systemowe (Monitor systemu, Dyski, Cherry). Zasilanie, blokada i wylogowanie są tylko w menu systemowym GNOME (prawy róg); bez aplikacji Rozszerzenia. To samo rozszerzenie trzyma siatkę aplikacji w porządku alfabetycznym (foldery pierwsze) i przy pobranym instalatorze (.exe, .msi, .rpm, .deb, .AppImage…) pokazuje okno Shrimp Apps z podpowiedzią sklepu. Pozycje programów, których nie ma w obrazie, są ukryte. Teksty: tłumaczenia GNOME (gnome-shell, nazwy programów), a czego GNOME nie ma, to `lang/` w formacie `boxes/i18n-core` (wszystkie języki GNOME). Szybkie ustawienia (prawy róg) mają dźwięk (PipeWire), Bluetooth, tryb zasilania i baterię w procentach. Bez menu „Programy”: aplikacje są w siatce w doku.
- Grafiki Plymouth są renderowane z makiety `boot.html`: `node boxes/plymouth-theme/tools/render-boot-assets.js <shrimp-handoff> <out>`.

## Linu(x)

Standard Shrimp ponad zwykłego Linuksa (pełna nazwa i hasło tylko na stronie projektu). Użytkownik nie widzi nazwy w systemie: nie ma jej w `os-release`, `issue` ani `motd`. Pakiety są w jednym pliku, `boxes/linu-x/root/usr/share/shrimp/linux/drivers.conf`, wspólnym dla nośnika (skrypty `build/` uruchamiane przez `boxes/linu-x/configure.sh`) i zainstalowanego systemu (`shrimp-setup-apply`, krok „Obraz i dźwięk”).

- **Oficjalny sterownik NVIDIA** (`build/20-nvidia.sh`): otwarte moduły podpisane przez SUSE (Turing i nowsze, działają z Secure Boot) + biblioteki z repozytorium NVIDIA, które zostaje w systemie. Starsze karty zostają na nośniku przy nouveau: decyduje `/usr/libexec/shrimp-gpu-gate` (awaryjnie parametr jądra `shrimp.nouveau=1`). Opcje sterownika (Wayland, uśpienie): `/usr/lib/modprobe.d/61-shrimp-nvidia.conf`.
- **Kodeki przez PipeWire** (pakiety boxa `linu-x` + `build/10-codecs.sh`): PipeWire jako jedyny serwer dźwięku, GStreamer z wtyczką PipeWire i kodekami z Leap, H.264 przez openh264 od Cisco. Bez Packmana.
- **Wygląd libadwaita dla Qt, Electron i GTK3**: box `app-style` (zależność `linu-x`), patrz niżej.

Brak pakietu lub repozytorium nie przerywa budowy, tylko daje linię `WARNING: Linu(x): …` w logu (`tools/build-iso.sh` ją wypisuje).

## Setup (jeden kreator: instalacja i konfiguracja)

Jeden kreator GTK4 + libadwaita (Rust, `boxes/setup/shell`) prowadzi od języka po wygląd i instalację, a po restarcie
system jest gotowy: ekran logowania, bez drugiego kreatora, logowania automatycznego i strażnika.

- **Nośnik**: przed GDM `shrimp-live-session` wybiera sesję `shrimp-setup` (gnome-kiosk + sam kreator, usługa
  użytkownika `org.shrimp.Setup.service` z `Restart=always`). Dysk z etykietą `SHRIMP_TEST` daje zamiast niej pulpit
  GNOME (testy QEMU, pomoc techniczna), także gdy dysk pojawi się po starcie GDM (reguła udev). Kreator rozmawia z HTTP API Agamy (Leap 16.1) przez `http://127.0.0.1`
  (token z `/run/shrimp-setup/token`). Produkt Agamy: `boxes/setup/system/agama/shrimp.yaml`.
- **Kroki**: witaj, język, licencje, region, klawiatura, internet (tylko bez sieci), dysk, konto, pliki z Windows,
  pendrive, przeglądarka, programy, wygląd, podsumowanie, instalacja, restart. Wybory idą do `install.json`
  w systemie docelowym.
- **Po instalacji** (`post-install.sh`, Agama): `shrimp-setup --restore` (pliki z Shrimp Install do folderów konta
  albo jednego folderu), `shrimp-usb-backup stash` (kopia pendrive'a), `shrimp-setup --apply` (przeglądarka, wygląd,
  dok, plik `later`), sieć Wi-Fi z nośnika, aktualizacje w tle. Po pierwszym starcie `shrimp-setup-later.timer`
  instaluje kodeki, NVIDIA i programy, gdy jest internet, a `shrimp-usb-restore.timer` przywraca pendrive.
- **Pliki z Windows** (Shrimp Install): nośnik ma `shrimp-transfer/shrimp.json` i pliki użytkownika; kreator bierze
  z niego imię, język, strefę, klawiaturę, BitLocker, programy (`shell/data/windows-apps.json`).

Katalogi boxa `setup`: `shell` (Rust), `system` (Agama, sesja kiosku, polkit, usługi, skrypty), `i18n` (teksty), `tests`.
Podgląd na danych z makiety: `cargo run -- --demo --window` (w `boxes/setup/shell`, `SHRIMP_SETUP_STEP=Dysk` otwiera krok).
Budowa i instalacja do katalogu (pakiet/KIWI): `boxes/setup/install.sh DESTDIR [--live]`.

## Jeden wygląd aplikacji

`boxes/app-style/shrimp-app-style` ustawia globalne nadpisania Flatpaka, żeby aplikacje GTK3, Qt i Electron wyglądały i zachowywały się jak GTK4/libadwaita: motyw adw-gtk3 (jasny/ciemny według trybu użytkownika), styl Adwaita i pasek tytułu Adwaita dla Qt (`QT_WAYLAND_DECORATION=adwaita`), okna plików i ustawienia przez portal, Wayland dla Electron. Toolkitu aplikacji nie da się podmienić, więc pasek tytułu Electron zostaje własny. Uruchamia się samo po każdej instalacji aplikacji (`shrimp-app-style.path`) i przy zmianie trybu (`shrimp-app-style-user.service`).

## Aplikacje Shrimps

Shrimp Apps, Cocktail i Panko są w [Shrimps-Working](https://github.com/Wojciech-Ciesielski/Shrimps-Working); opis tam.
