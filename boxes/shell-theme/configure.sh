# Fragment config.sh boxa shell-theme (chroot obrazu; składa go shrimp-box).
# GNOME Shell theme: GDM background + lock screen (appended to every shell stylesheet)
RES=/usr/share/gnome-shell/gnome-shell-theme.gresource
[ -f "$RES" ] || die "$RES not found"
work="$(mktemp -d)"
for r in $(gresource list "$RES"); do
  mkdir -p "$work$(dirname "$r")"
  gresource extract "$RES" "$r" > "$work$r"
done
prefix=/org/gnome/shell/theme
ls "$work$prefix"
found=0
for css in "$work$prefix"/gnome-shell*.css; do
  [ -f "$css" ] || continue
  cat /usr/share/shrimp/shell/shrimp.css >> "$css"
  found=1
done
[ "$found" = 1 ] || die "no gnome-shell*.css in $RES"
cp /usr/share/shrimp/shell/gdm-background.jpg "$work$prefix/shrimp-gdm-background.jpg"
{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo "<gresources><gresource prefix=\"$prefix\">"
  (cd "$work$prefix" && find . -type f | sed 's|^\./||' | sort | sed 's|.*|<file>&</file>|')
  echo '</gresource></gresources>'
} > "$work/theme.gresource.xml"
glib-compile-resources --sourcedir="$work$prefix" --target="$RES" "$work/theme.gresource.xml"
rm -rf "$work"
