#!/bin/bash
# Linu(x) 2/3: obraz i dźwięk działają od razu. Wszystko idzie przez PipeWire
# (pipewire-pulseaudio i pipewire-alsa przejmują PulseAudio i ALSA), GStreamer dostaje
# wtyczkę PipeWire i kodeki z repozytoriów Leap, H.264 przez openh264 od Cisco.
# Aplikacje z Flathub mają własne kodeki (org.freedesktop.Platform.codecs-extra).
set -uo pipefail
. /usr/share/shrimp/linux/build/common
ensure_repos || exit 0
# shellcheck disable=SC2086
install_each $CODEC_PACKAGES

# openh264 (Cisco): osobne repozytorium, bo binaria muszą pochodzić od Cisco.
if "${ZYP[@]}" addrepo --refresh "$OPENH264_REPO_URL" "$OPENH264_REPO_ALIAS" >/dev/null 2>&1 \
   && "${ZYP[@]}" refresh "$OPENH264_REPO_ALIAS" >/dev/null 2>&1; then
  # shellcheck disable=SC2086
  install_each $OPENH264_PACKAGES
else
  "${ZYP[@]}" removerepo "$OPENH264_REPO_ALIAS" >/dev/null 2>&1 || true
  warn "repozytorium openh264 ($OPENH264_REPO_URL) niedostępne"
fi

# PipeWire jako jedyny serwer dźwięku dla każdego użytkownika.
systemctl --global enable pipewire.socket pipewire-pulse.socket wireplumber.service 2>/dev/null \
  || warn "nie udało się włączyć gniazd PipeWire (może są już domyślnie włączone)"
exit 0
