#!/bin/bash
# KIWI editbootconfig hook (runs on the build host, cwd = image root).
# $1 = "iso:<media dir>". Shapes the live ISO GRUB menu to the design:
#   Shrimp / Computer settings (UEFI firmware, translated), Shrimp theme fonts, 1920x1080.
# A normal start shows no menu and no text: hidden 1 s timeout (Esc, F4 or F8 open the menu;
# under BIOS holding Shift does too), no "Loading kernel/initrd..." or "press 't'" messages.
# Menu language: boot/grub2/shrimp-lang.cfg on the media (written by Shrimp Install from the Windows
# language) sets shrimp_lang; themes/shrimp/lang/<tag>.cfg (tools/render-grub-i18n.js) has the entry
# name and whether it must be an image; no file or unknown language means English.
set -euo pipefail
media="${1#iso:}"
theme_txt="$(find "$media" -path '*/themes/shrimp/theme.txt' -print -quit)"
[ -n "$theme_txt" ] || { echo "edit_boot_config: theme not found in $media" >&2; exit 1; }
theme_dir="$(dirname "$theme_txt")"

# two-entry live menu layout
cp "$theme_dir/theme-live.txt" "$theme_txt"
# Secure Boot (lockdown) variant: text as images, see theme-live-sb.txt
cp "$theme_dir/theme-live-sb.txt" "$theme_dir/theme-sb.txt"
# one theme per language: only the countdown text differs (theme text cannot be translated at boot)
[ -f "$theme_dir/lang/timeouts.tsv" ] || { echo "edit_boot_config: $theme_dir/lang/timeouts.tsv missing" >&2; exit 1; }
while IFS=$'\t' read -r tag text; do
  [ -n "$tag" ] || continue
  for pair in "theme.txt:theme-$tag.txt" "theme-sb.txt:theme-sb-$tag.txt"; do
    awk -v t="$text" '/^  text = / { print "  text = \"" t "\""; next } { print }' \
      "$theme_dir/${pair%%:*}" > "$theme_dir/${pair##*:}"
  done
done < "$theme_dir/lang/timeouts.tsv"

# The text theme has icon_width/height 0, but icons/ holds the Secure Boot entry images (os.png,
# efi-<tag>.png): GRUB would scale them to 0x0 and print "zero dimension" when the menu is opened.
# So with the text theme the entry classes get a "txt-" prefix that matches no icon file.
# Under Secure Boot (lockdown) the signed GRUB refuses unsigned font files
# ("prohibited by secure boot policy"), so fonts and the text theme are used only outside lockdown;
# with lockdown (or for scripts Inter has no glyphs for, shrimp_img=1) the image-based
# theme-sb-<tag>.txt is used (no fonts needed).
fontlines=""
for f in "$theme_dir"/*.pf2; do
  fontlines="${fontlines}        loadfont (\$root)/boot/grub2/themes/shrimp/$(basename "$f")"$'\n'
done

patched=0
# BIOS (boot/grub2) and EFI (EFI/BOOT, EFI/Linux) copies of the menu
while IFS= read -r cfg; do
  grep -q '^menuentry ' "$cfg" || continue
  # drop "Boot from Hard Disk" and "Failsafe", add the firmware settings entry after the first menuentry
  awk '
    /^menuentry "(Boot from Hard Disk|Failsafe)/ { skip = 1 }
    skip { if ($0 ~ /^}/) { skip = 0 } ; next }
    { print }
    /^menuentry / && !added { inentry = 1 }
    inentry && /^}/ {
      inentry = 0; added = 1
      print "if [ \"${grub_platform}\" = \"efi\" ]; then"
      print "    menuentry \"${shrimp_fw}\" --class ${shrimp_ic}efi-${shrimp_lang} --unrestricted {"
      print "        fwsetup"
      print "    }"
      print "fi"
    }
  ' "$cfg" > "$cfg.new"
  # silent start: no echo lines (an empty if-body is a syntax error, so they become "true"),
  # hidden menu; Shift via keystatus only under BIOS (EFI firmware often has no key state and
  # a missing module would print an error on screen)
  awk '
    /^[ \t]*echo[ \t]/ { match($0, /^[ \t]*/); print substr($0, 1, RLENGTH) "true"; next }
    /^[ \t]*set timeout_style=/ { next }
    /^[ \t]*set timeout=/ {
      print "set timeout=1"
      print "set timeout_style=hidden"
      print "if [ \"${grub_platform}\" = \"pc\" ]; then"
      print "    insmod keystatus"
      print "    if keystatus --shift; then"
      print "        set timeout=-1"
      print "        set timeout_style=menu"
      print "    fi"
      print "fi"
      next
    }
    { print }
  ' "$cfg.new" > "$cfg.new2" && mv "$cfg.new2" "$cfg.new"
  grep -q '^set timeout_style=hidden' "$cfg.new" || { echo "edit_boot_config: no timeout in $cfg" >&2; exit 1; }
  # language, theme fonts, and the 1920x1080 mode the theme is drawn for
  sed -i -e "s|^\(\s*\)set gfxmode=.*|\1set gfxmode=1920x1080,auto|" "$cfg.new"
  FONTLINES="$fontlines" awk '
    /^[ \t]*set theme=.*themes\/shrimp\/theme\.txt/ {
      pre = $0; sub(/^[ \t]*set theme=/, "", pre); sub(/theme\.txt$/, "", pre)
      print "if [ -f ($root)/boot/grub2/shrimp-lang.cfg ]; then"
      print "    source ($root)/boot/grub2/shrimp-lang.cfg"
      print "fi"
      print "if [ -f ($root)/boot/grub2/themes/shrimp/lang/${shrimp_lang}.cfg ]; then"
      print "    source ($root)/boot/grub2/themes/shrimp/lang/${shrimp_lang}.cfg"
      print "else"
      print "    set shrimp_lang=en"
      print "fi"
      print "if [ \"${lockdown}\" != \"y\" -a \"${shrimp_img}\" != \"1\" ]; then"
      printf "%s", ENVIRON["FONTLINES"]
      print "    set theme=" pre "theme-${shrimp_lang}.txt"
      print "    set shrimp_ic=txt-"
      print "else"
      print "    set theme=" pre "theme-sb-${shrimp_lang}.txt"
      print "fi"
      next
    }
    NR == 1 { print "set shrimp_lang=en"; print "set shrimp_fw=\"Computer settings\""; print "set shrimp_img=0"; print "set shrimp_ic=" }
    /^menuentry "Shrimp" --class os / { sub(/--class os /, "--class ${shrimp_ic}os ") }
    { print }
  ' "$cfg.new" > "$cfg.new2" && mv "$cfg.new2" "$cfg.new"
  mv "$cfg.new" "$cfg"
  patched=$((patched + 1))
  echo "edit_boot_config: patched $cfg"
  grep -n 'menuentry\|loadfont\|gfxmode\|theme\|shrimp_' "$cfg" || true
done < <(find "$media" -name grub.cfg)
[ "$patched" -gt 0 ] || { echo "edit_boot_config: no grub.cfg with menu entries in $media" >&2; exit 1; }
