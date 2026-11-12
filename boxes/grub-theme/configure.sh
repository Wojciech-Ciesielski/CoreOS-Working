# Fragment config.sh boxa grub-theme (chroot obrazu; składa go shrimp-box).
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
GRUBTHEME=/usr/share/grub2/themes/shrimp
# hanging chevron: 36 px glyph, 17 px gap to the text, item height 77, glyph top at 20 px
rsvg-convert -w 36 -h 36 --page-width 54 --page-height 77 --left 0 --top 20 \
  -s "$(css_color '#ffffff')" -o "$GRUBTHEME/select_w.png" "$next_svg"

# GRUB fonts (Inter → PF2) and theme font names
pf2_name() {
  # PF2: "FILE" len "PFF2" "NAME" <be32 len> <name\0>
  local len
  len=$(od -An -tu1 -j16 -N4 "$1" | awk '{print $1*16777216+$2*65536+$3*256+$4}')
  dd if="$1" bs=1 skip=20 count="$len" 2>/dev/null | tr -d '\0'
}
INTER=/usr/share/fonts/truetype/inter
grub2-mkfont -s 38 -o "$GRUBTHEME/inter-regular-38.pf2" "$INTER/Inter-Regular.ttf"
# SemiBold needs its own family name, otherwise GRUB names it "Inter Regular 38" too
grub2-mkfont -n "Inter SemiBold" -s 38 -o "$GRUBTHEME/inter-semibold-38.pf2" "$INTER/Inter-SemiBold.ttf"
grub2-mkfont -s 27 -o "$GRUBTHEME/inter-regular-27.pf2" "$INTER/Inter-Regular.ttf"
r38="$(pf2_name "$GRUBTHEME/inter-regular-38.pf2")"
s38="$(pf2_name "$GRUBTHEME/inter-semibold-38.pf2")"
r27="$(pf2_name "$GRUBTHEME/inter-regular-27.pf2")"
echo "GRUB fonts: '$r38' '$s38' '$r27'"
[ "$r38" != "$s38" ] || die "GRUB regular and semibold fonts have the same name"
for t in "$GRUBTHEME/theme.txt" "$GRUBTHEME/theme-live.txt"; do
  sed -i -e "s|@FONT_REGULAR_38@|$r38|g" -e "s|@FONT_SEMIBOLD_38@|$s38|g" -e "s|@FONT_REGULAR_27@|$r27|g" "$t"
done

# installed-system GRUB defaults (used once Shrimp is installed to disk)
if [ -f /etc/default/grub ]; then
  sed -i -e 's|^GRUB_DISTRIBUTOR=.*|GRUB_DISTRIBUTOR="Shrimp"|' \
         -e 's|^GRUB_TIMEOUT=.*|GRUB_TIMEOUT=5|' \
         -e 's|^GRUB_THEME=.*|GRUB_THEME=/boot/grub2/themes/shrimp/theme.txt|' \
         -e 's|^GRUB_GFXMODE=.*|GRUB_GFXMODE=1920x1080,auto|' /etc/default/grub
  grep -q '^GRUB_THEME=' /etc/default/grub || echo 'GRUB_THEME=/boot/grub2/themes/shrimp/theme.txt' >> /etc/default/grub
  grep -q '^GRUB_GFXMODE=' /etc/default/grub || echo 'GRUB_GFXMODE=1920x1080,auto' >> /etc/default/grub
fi
# menu names from specs/grub.md
[ -f /etc/grub.d/30_uefi-firmware ] && sed -i 's/UEFI Firmware Settings/Computer settings/' /etc/grub.d/30_uefi-firmware || true
for f in /etc/grub.d/80_suse_btrfs_snapshot /usr/lib/snapper/plugins/grub /etc/grub.d/*snapshot*; do
  [ -f "$f" ] && sed -i 's/Start bootloader from a read-only snapshot/Previous system versions/' "$f" || true
done
rm -f /tmp/icon.css
