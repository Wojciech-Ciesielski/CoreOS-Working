#!/usr/bin/env python3
"""Sprawdza, czy pliki używają wartości z shrimp.toml (jedno źródło wspólnych zmiennych).

Każdy wpis: klucz z shrimp.toml, plik (względem katalogu repozytorium) i tekst, w którym {v} to wartość.
Pliki, których tu nie ma (boxy z innych repozytoriów przed tools/fetch-shrimps.sh), są pomijane.
Użycie: tools/check-shrimp-toml.py   (kod 1 = któryś plik ma inną wartość)
"""
import pathlib, sys, tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
VARS = tomllib.loads((ROOT / "shrimp.toml").read_text())

USES = [
    ("release.version", "boxes/branding/root/usr/share/shrimp/version", "{v}\n"),
    ("base.leap", "tools/build-iso.sh", 'LEAP_VERSION:-{v}}'),
    ("base.leap", ".github/workflows/build-iso.yml", 'default: "{v}"'),
    ("base.leap", ".github/workflows/build-iso.yml", "leap_version || '{v}'"),
    ("base.repo", ".github/workflows/boxes.yml", "--repo {v}/x86_64"),
    ("base.repo", "boxes/image-base/config.kiwi", '<source path="{v}/x86_64/"/>'),
    ("base.repo", "boxes/setup/system/agama/shrimp.yaml", "url: {v}/$basearch"),
    ("site.help", "boxes/help-links/help.json", '"base": "{v}"'),
    ("install.min_gb", "boxes/setup/shell/src/data.rs", "pub const SHRIMP_MIN_GB: u64 = {v};"),
    ("install.windows_spare_gb", "boxes/setup/shell/src/data.rs", "pub const WINDOWS_SPARE_GB: u64 = {v};"),
    ("flathub.repo", "boxes/setup/system/share/post-install.sh", "{v}"),
    ("flathub.repo", "boxes/setup/system/libexec/shrimp-setup-apply", "{v}"),
    ("flathub.repo", "boxes/flathub/configure.sh", "{v}"),
]


def value(key):
    v = VARS
    for part in key.split("."):
        v = v[part]
    return str(v)


bad = 0
checked = 0
for key, rel, pattern in USES:
    path = ROOT / rel
    if not path.is_file():
        print(f"pominięto {rel} (brak pliku)")
        continue
    want = pattern.replace("{v}", value(key))
    checked += 1
    if want not in path.read_text(errors="replace"):
        bad += 1
        print(f"::error file={rel}::{rel}: oczekiwano {want.strip()!r} (shrimp.toml › {key})")
print(f"shrimp.toml: sprawdzono {checked} miejsc, niezgodnych {bad}")
sys.exit(1 if bad else 0)
