# Fragment config.sh boxa live-session (chroot obrazu; składa go shrimp-box).
# Sesja live: start graficzny, logowanie automatyczne jako „shrimp” (hasło: shrimp, potrzebne do ekranu blokady).
systemctl set-default graphical.target
systemctl enable NetworkManager.service
if [ -f /etc/sysconfig/displaymanager ]; then
  sed -i -e 's/^DISPLAYMANAGER=.*/DISPLAYMANAGER="gdm"/' \
         -e 's/^DISPLAYMANAGER_AUTOLOGIN=.*/DISPLAYMANAGER_AUTOLOGIN="shrimp"/' /etc/sysconfig/displaymanager
fi
# Leap 16 nie ma już update-alternatives w obrazie (display-manager.service wybiera DISPLAYMANAGER powyżej).
if command -v update-alternatives >/dev/null 2>&1; then
  update-alternatives --set default-displaymanager /usr/lib/X11/displaymanagers/gdm || true
fi
systemctl enable display-manager.service || systemctl enable gdm.service
mkdir -p /etc/gdm
cat > /etc/gdm/custom.conf <<'CONF'
[daemon]
AutomaticLoginEnable=True
AutomaticLogin=shrimp
WaylandEnable=true
CONF
echo 'shrimp ALL=(ALL) ALL' > /etc/sudoers.d/shrimp
chmod 0440 /etc/sudoers.d/shrimp
echo shrimp > /etc/hostname
