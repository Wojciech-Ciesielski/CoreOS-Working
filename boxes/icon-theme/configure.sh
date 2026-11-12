# Fragment config.sh boxa icon-theme (chroot obrazu; składa go shrimp-box).
# MoreWaita: Shrimp's main icon theme (Adwaita style, adds the app icons Adwaita lacks).
# Not packaged for Leap, so it comes from GitHub: the latest release, or the main branch
# when the API gives no release. MOREWAITA_REF pins a tag/branch. Set as default in
# 99_shrimp.gschema.override (icon-theme='MoreWaita') and for GDM in dconf gdm.d.
MW_DIR=/usr/share/icons/MoreWaita
MW_REF="${MOREWAITA_REF:-}"
if [ -z "$MW_REF" ]; then
  MW_REF="$(curl -fsSL --retry 5 --retry-all-errors --retry-delay 3 https://api.github.com/repos/somepaulo/MoreWaita/releases/latest 2>/dev/null \
            | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n1 || true)"
fi
MW_REF="${MW_REF:-main}"
mw_ok=0
# github.com/archive gives 503 now and then; codeload.github.com serves the same archive, and the
# default branch is the last resort when the release tag cannot be fetched at all.
for url in "https://github.com/somepaulo/MoreWaita/archive/${MW_REF}.zip" \
           "https://codeload.github.com/somepaulo/MoreWaita/zip/refs/tags/${MW_REF}" \
           "https://codeload.github.com/somepaulo/MoreWaita/zip/refs/heads/main"; do
  if curl -fsSL --retry 4 --retry-all-errors --retry-delay 5 -o /tmp/morewaita.zip "$url"; then mw_ok=1; echo "MoreWaita downloaded from $url"; break; fi
  echo "WARNING: MoreWaita download failed: $url"
done
[ "$mw_ok" = 1 ] || die "MoreWaita ${MW_REF}: download failed"
rm -rf /tmp/morewaita "$MW_DIR"
mkdir -p /tmp/morewaita
unzip -q /tmp/morewaita.zip -d /tmp/morewaita
MW_SRC="$(dirname "$(find /tmp/morewaita -maxdepth 2 -name index.theme | head -n1)")"
[ -f "$MW_SRC/index.theme" ] || die "MoreWaita ${MW_REF}: no index.theme in the archive"
mkdir -p "$MW_DIR"
cp -a "$MW_SRC"/. "$MW_DIR"/
# keep only the theme itself (no installer scripts, docs, git metadata)
rm -rf "$MW_DIR"/.git* "$MW_DIR"/*.sh "$MW_DIR"/*.md "$MW_DIR"/*.txt "$MW_DIR"/LICENSE* "$MW_DIR"/meson* "$MW_DIR"/screenshots
grep -q '^Name=MoreWaita' "$MW_DIR/index.theme" || echo "WARNING: MoreWaita index.theme has an unexpected Name= line"
chmod -R a+rX "$MW_DIR"
gtk-update-icon-cache -f "$MW_DIR" || true
rm -rf /tmp/morewaita /tmp/morewaita.zip
echo "icon theme installed: MoreWaita ${MW_REF} ($(find "$MW_DIR" -name '*.svg' | wc -l) icons)"
