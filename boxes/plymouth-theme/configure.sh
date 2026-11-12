# Fragment config.sh boxa plymouth-theme (chroot obrazu; składa go shrimp-box).
# Ikony motywu: prawdziwe symboliczne ikony Adwaita (DESIGN.md §10), przebarwione.
adw_icon() {
  local f
  for name in "$@"; do
    f="$(find /usr/share/icons/Adwaita -name "${name}.svg" -print -quit)"
    [ -n "$f" ] && { echo "$f"; return 0; }
  done
  return 1
}
css_color() { printf '* { fill: %s !important; color: %s !important; }\n' "$1" "$1" > /tmp/icon.css; echo /tmp/icon.css; }

next_svg="$(adw_icon go-next-symbolic)" || die "go-next-symbolic not found"
lock_svg="$(adw_icon changes-prevent-symbolic system-lock-screen-symbolic channel-secure-symbolic)" || die "lock icon not found"

PLY=/usr/share/plymouth/themes/shrimp
# lock: 1.6% of width (30.72 px at 1920), colour dim #8c8c8c; arrow: same size, black on the white circle
rsvg-convert -w 31 -h 31 -s "$(css_color '#8c8c8c')" -o "$PLY/lock.png" "$lock_svg"
rsvg-convert -w 62 -h 62 -s "$(css_color '#8c8c8c')" -o "$PLY/lock@2x.png" "$lock_svg"
rsvg-convert -w 31 -h 31 -s "$(css_color '#000000')" -o "$PLY/arrow.png" "$next_svg"
rsvg-convert -w 62 -h 62 -s "$(css_color '#000000')" -o "$PLY/arrow@2x.png" "$next_svg"

plymouth-set-default-theme shrimp
rm -f /tmp/icon.css
