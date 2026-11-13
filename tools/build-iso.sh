#!/bin/bash
# Builds the Shrimp live ISO: container iso-live (containers/iso-live.toml) → KIWI, inside an openSUSE
# Tumbleweed container. shrimp-box (boxes/shrimp-box) assembles the KIWI description from the boxes
# (config.kiwi packages, config.sh fragments, root/ overlays), builds the Rust boxes in a Leap chroot
# and installs every box into the image root; kiwi-ng then builds the ISO.
# Usage: tools/build-iso.sh [leap-version]   (default 16.1). Needs Docker or Podman and ~20 GB free.
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION="${1:-${LEAP_VERSION:-16.1}}"
ENGINE="$(command -v podman || command -v docker)"

base="https://download.opensuse.org/distribution/leap/${VERSION}/repo/oss"
REPO=""
for candidate in "$base/x86_64" "$base"; do
  if curl -fsSLI "$candidate/repodata/repomd.xml" >/dev/null 2>&1; then REPO="$candidate"; break; fi
done
[ -n "$REPO" ] || { echo "openSUSE Leap ${VERSION} repository not found under $base" >&2; exit 1; }
echo "Using repository: $REPO"

# Shrimps (Shrimp Apps, Cocktail, Panko) z wersji przypiętej w tools/shrimps.ref
tools/fetch-shrimps.sh

mkdir -p out
"$ENGINE" run --rm --privileged -v /dev:/dev -v "$PWD:/work" -w /work \
  docker.io/opensuse/tumbleweed:latest bash -euxc "
    zypper -n --gpg-auto-import-keys ref
    # Tumbleweed mirrors lag behind a new snapshot (404 on single RPMs, e.g. binutils, for hours):
    # after a failure, fetch straight from the origin server (downloadcontent.opensuse.org) instead.
    ok=0
    for i in 1 2 3; do
      zypper -n in --no-recommends python3-kiwi kiwi-systemdeps-iso-media kiwi-systemdeps-bootloaders kiwi-systemdeps-filesystems binutils findutils gawk sed coreutils \
        nodejs-default npm-default tar gzip rustup gcc createrepo_c squashfs xorriso gpg2 && { ok=1; break; }
      sed -i -E 's#https?://(download|cdn)\.opensuse\.org#https://downloadcontent.opensuse.org#' /etc/zypp/repos.d/*.repo
      zypper -n clean -a; sleep 20; zypper -n --gpg-auto-import-keys ref -f
    done
    [ \$ok = 1 ]
    # shrimp-box itself (boxes/shrimp-box), then the whole container: description + chroot builds + installs
    export CARGO_HOME=/root/.cargo RUSTUP_HOME=/root/.rustup
    rustup toolchain list | grep -q stable || rustup default stable
    cargo build --release --locked --manifest-path boxes/shrimp-box/Cargo.toml --target-dir out/shrimp-box
    SB=out/shrimp-box/release/shrimp-box
    rm -rf out/build
    \$SB image iso-live out/desc --repo '$REPO'
    kiwi-ng --logfile out/kiwi.log --color-output system build \
      --description out/desc --set-repo '$REPO' --target-dir out/build
    mv out/build/*.iso out/
    # Adwaita icon sources used by the boot themes, printed for design previews
    for n in go-next-symbolic changes-prevent-symbolic system-lock-screen-symbolic network-wireless-signal-excellent-symbolic \\
             audio-volume-high-symbolic battery-level-100-symbolic system-shutdown-symbolic; do
      f=\$(find out/build/build/image-root/usr/share/icons/Adwaita -name \"\$n.svg\" -print -quit)
      [ -n \"\$f\" ] && echo \"ICON-SVG \$n \$(base64 -w0 \"\$f\")\" || true
    done
    # pakiety sesji live (dla repozytorium offline: extra/)
    chroot out/build/build/image-root rpm -qa --qf '%{NAME}\\n' | sort -u > out/live-packages.txt
    grep -h 'GRUB fonts\|WARNING\|extension installed\|optional package installed\|config.sh\|Linu(x)' out/kiwi.log | tail -40 || true
    rm -rf out/build out/desc
    # Instalacja bez internetu: repozytorium pakietów w ISO (LiveOS/shrimp-repo.squashfs).
    # SHRIMP_OFFLINE_REPO=0 pomija ten krok (szybsza budowa do testów samego nośnika).
    if [ \"${SHRIMP_OFFLINE_REPO:-1}\" != 0 ]; then
      tools/build-offline-repo.sh '$VERSION' \$(ls out/*.iso | head -n1) out/live-packages.txt '$REPO'
    fi
  "
ls -lh out/*.iso

# What the image was built from and what the build skipped (next to the ISO, read by Release):
# build-info.txt (commit, base image, repositories, versions fetched at build time) and
# build-warnings.txt (every WARNING line from config.sh: NVIDIA, codecs, MoreWaita, extensions).
{
  echo "commit: $(git -c safe.directory='*' rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "leap repository: $REPO"
  echo "tumbleweed image: $("$ENGINE" image inspect --format '{{index .RepoDigests 0}}' docker.io/opensuse/tumbleweed:latest 2>/dev/null || echo unknown)"
  grep -hoE 'MoreWaita downloaded from [^ ]+|extension installed: .*|Linu\(x\): repozytorium NVIDIA: .*|optional package installed: .*' out/kiwi.log \
    | sed 's/\x1b\[[0-9;]*m//g' | sort -u || true
} > out/build-info.txt
grep -hoE 'WARNING: .*' out/kiwi.log | sed -e 's/\x1b\[[0-9;]*m//g' -e 's/^WARNING: //' -e 's/^Linu(x): //' | sort -u > out/build-warnings.txt || true
cat out/build-info.txt
if [ -s out/build-warnings.txt ]; then
  echo "Build warnings:"; cat out/build-warnings.txt
  # GitHub Actions: each one as an annotation on the run
  if [ -n "${GITHUB_ACTIONS:-}" ]; then sed 's/^/::warning title=Build ISO::/' out/build-warnings.txt; fi
fi
