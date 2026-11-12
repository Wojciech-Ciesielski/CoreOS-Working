# Fragment config.sh boxa branding (chroot obrazu; składa go shrimp-box). os-release (Ustawienia › O systemie) i ikony logo.
if [ -f /usr/lib/os-release ]; then
  /usr/libexec/shrimp-os-release   # nazwa Shrimp, bez adresów openSUSE (ten sam skrypt w post-install)
  # Linu(x) (box linu-x, /usr/share/shrimp/linux) stays out of os-release:
  # the user does not see the name anywhere in the system.
fi
gtk-update-icon-cache -f /usr/share/icons/hicolor || true
