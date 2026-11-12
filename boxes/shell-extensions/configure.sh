# Fragment config.sh boxa shell-extensions (chroot obrazu; składa go shrimp-box).
# GNOME Shell extensions from extensions.gnome.org
# (Leap 16 does not ship Light Style in a package,
#  so they come from extensions.gnome.org as well)
SHELL_MAJOR="$(rpm -q --qf '%{VERSION}' gnome-shell | cut -d. -f1)"
EXT=/usr/share/gnome-shell/extensions
install_ext() {
  local uuid="$1" info url
  info="$(curl -fsSL "https://extensions.gnome.org/extension-info/?uuid=${uuid}&shell_version=${SHELL_MAJOR}")" || return 1
  url="$(echo "$info" | sed -n 's/.*"download_url": *"\([^"]*\)".*/\1/p')"
  [ -n "$url" ] || return 1
  curl -fsSL -o "/tmp/${uuid}.zip" "https://extensions.gnome.org${url}" || return 1
  rm -rf "${EXT:?}/${uuid}"
  mkdir -p "$EXT/$uuid"
  unzip -q -o "/tmp/${uuid}.zip" -d "$EXT/$uuid"
  rm -f "/tmp/${uuid}.zip"
  # Extension.getSettings() prefers the extension's own schemas/ directory, and an
  # override only applies to schemas compiled next to it (that is why Just Perfection
  # kept the Activities button). Compile our override into the local directory too.
  if [ -d "$EXT/$uuid/schemas" ]; then
    cp "$EXT/$uuid"/schemas/*.gschema.xml /usr/share/glib-2.0/schemas/
    cp /usr/share/glib-2.0/schemas/99_shrimp.gschema.override "$EXT/$uuid/schemas/"
    glib-compile-schemas "$EXT/$uuid/schemas" 2>&1 | grep -v 'No such schema' || true
  fi
  chmod -R a+rX "$EXT/$uuid"
  echo "extension installed: $uuid version $(echo "$info" | sed -n 's/.*"version": *\([0-9]*\).*/\1/p')"
}
for uuid in ding@rastersoft.com blur-my-shell@aunetx dash-to-dock@micxgx.gmail.com just-perfection-desktop@just-perfection; do
  [ -d "$EXT/$uuid" ] || install_ext "$uuid" || die "extension $uuid has no release for GNOME $SHELL_MAJOR"
done
# Light Style: optional, newer shells follow the light colour scheme on their own
uuid=light-style@gnome-shell-extensions.gcampax.github.com
if [ ! -d "$EXT/$uuid" ] && ! install_ext "$uuid"; then
  echo "WARNING: $uuid has no release for GNOME $SHELL_MAJOR; the shell follows color-scheme instead"
fi
ls "$EXT"
glib-compile-schemas /usr/share/glib-2.0/schemas
