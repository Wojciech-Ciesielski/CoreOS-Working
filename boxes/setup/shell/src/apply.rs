//! `shrimp-setup --apply [--root /mnt]` (post-install.sh, root): wybory z kreatora (install.json) w systemie
//! docelowym. Po restarcie nie ma już pierwszej konfiguracji: system startuje do ekranu logowania gotowy.
//!   - przeglądarka domyślna (/etc/xdg/mimeapps.list) i dok,
//!   - wygląd: tryb, kolor, rozmiar tekstu, światło nocne (99_shrimp_user.gschema.override, wygrywa z 99_shrimp),
//!   - kodeki, NVIDIA, przeglądarka i programy z Windows do doinstalowania, gdy jest internet
//!     ($STATE/later, shrimp-setup-later.timer),
//!   - pendrive z Shrimp Install: przywrócenie albo folder Pendrive po pierwszym uruchomieniu (shrimp-usb-restore.timer).
use crate::system::{dock, mimeapps_with, valid_app_id, BROWSER_TYPES};
use serde::Deserialize;
use std::fs;
use std::path::Path;

const ACCENTS: [&str; 9] = ["blue", "teal", "green", "yellow", "orange", "red", "pink", "purple", "slate"];
pub const STATE: &str = "var/lib/shrimp-setup";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Handoff {
    pub user_name: String,
    pub proprietary: bool,
    pub files: String,
    pub files_folder: String,
    pub usb: String,
    pub browser: String,
    pub apps: Vec<String>,
    pub color_scheme: String,
    pub accent: String,
    pub text_scale: f64,
    pub night_light: bool,
    pub auto_updates: bool,
}

impl Default for Handoff {
    fn default() -> Self {
        Handoff {
            user_name: String::new(),
            proprietary: true,
            files: "folders".into(),
            files_folder: "Windows".into(),
            usb: "restore".into(),
            browser: "org.mozilla.firefox".into(),
            apps: vec![],
            color_scheme: "default".into(),
            accent: String::new(),
            text_scale: 1.0,
            night_light: false,
            auto_updates: true,
        }
    }
}

pub fn read(root: &Path) -> Result<Handoff, String> {
    let p = root.join(STATE).join("install.json");
    serde_json::from_str(&fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?).map_err(|e| e.to_string())
}

fn gv_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Zawartość 99_shrimp_user.gschema.override.
pub fn schema_override(h: &Handoff, filer: bool) -> String {
    let browser = if valid_app_id(&h.browser) { h.browser.as_str() } else { "org.mozilla.firefox" };
    let scheme = if h.color_scheme == "prefer-dark" { "prefer-dark" } else { "default" };
    let scale = if h.text_scale.is_finite() { h.text_scale.clamp(0.8, 1.6) } else { 1.0 };
    let mut s = format!("# Wybory z Shrimp Setup (shrimp-setup --apply)\n[org.gnome.desktop.interface]\ncolor-scheme={}\ntext-scaling-factor={scale:.2}\n", gv_str(scheme));
    if ACCENTS.contains(&h.accent.as_str()) {
        s += &format!("accent-color={}\n", gv_str(&h.accent));
    }
    s += &format!(
        "\n[org.gnome.settings-daemon.plugins.color]\nnight-light-enabled={}\nnight-light-schedule-automatic=true\n",
        h.night_light
    );
    let fav: Vec<String> = dock(browser, filer).iter().map(|d| gv_str(d)).collect();
    s += &format!("\n[org.gnome.shell]\nfavorite-apps=[{}]\n", fav.join(", "));
    s
}

/// Zawartość $STATE/later (czyta ją shrimp-setup-apply --later jako skrypt powłoki).
pub fn later(h: &Handoff) -> String {
    let mut apps: Vec<&str> = Vec::new();
    for a in std::iter::once(&h.browser).chain(h.apps.iter()) {
        if valid_app_id(a) && !apps.contains(&a.as_str()) {
            apps.push(a);
        }
    }
    let yes = if h.proprietary { "yes" } else { "no" };
    let nv = if h.proprietary { "nvidia" } else { "none" };
    format!("CODECS={yes}\nNVIDIA={nv}\nAPPS='{}'\n", apps.join(" "))
}

pub fn apply(root: &Path) -> Result<(), String> {
    let h = read(root)?;
    let w = |rel: &str, text: &str| -> Result<(), String> {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
    };
    // Przeglądarka domyślna dla całego systemu (działa, gdy tylko pojawi się jej plik .desktop).
    let browser = if valid_app_id(&h.browser) { h.browser.clone() } else { "org.mozilla.firefox".into() };
    let mime = fs::read_to_string(root.join("etc/xdg/mimeapps.list")).unwrap_or_default();
    w("etc/xdg/mimeapps.list", &mimeapps_with(&mime, &BROWSER_TYPES, &format!("{browser}.desktop")))?;
    let filer = root.join("usr/share/applications/org.shrimp.Filer.desktop").exists();
    w("usr/share/glib-2.0/schemas/99_shrimp_user.gschema.override", &schema_override(&h, filer))?;
    w(&format!("{STATE}/later"), &later(&h))?;
    // Pendrive: wybór czeka obok kopii (shrimp-usb-backup stash zrobił ją przed tym krokiem).
    if root.join(STATE).join("usb/pending").exists() {
        let choice = if h.usb == "home" { "home" } else { "restore" };
        w(&format!("{STATE}/usb/choice"), &format!("{choice}\n{}\n", h.user_name))?;
    }
    println!("shrimp-setup: wybory z Setup zapisane (przeglądarka {browser}, programy {}, pendrive {})", h.apps.len(), h.usb);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_choices_into_target() {
        let root = std::env::temp_dir().join(format!("shrimp-apply-{}", std::process::id()));
        fs::create_dir_all(root.join(STATE).join("usb")).unwrap();
        fs::write(root.join(STATE).join("usb/pending"), "").unwrap();
        fs::write(
            root.join(STATE).join("install.json"),
            r#"{"userName":"jan","proprietary":false,"browser":"com.brave.Browser","apps":["com.spotify.Client","x; rm -rf /","com.brave.Browser"],
               "colorScheme":"prefer-dark","accent":"teal","textScale":1.2,"nightLight":true,"usb":"home"}"#,
        )
        .unwrap();
        apply(&root).unwrap();
        let o = fs::read_to_string(root.join("usr/share/glib-2.0/schemas/99_shrimp_user.gschema.override")).unwrap();
        assert!(o.contains("color-scheme='prefer-dark'") && o.contains("accent-color='teal'") && o.contains("text-scaling-factor=1.20"));
        assert!(o.contains("night-light-enabled=true") && o.contains("favorite-apps=['com.brave.Browser.desktop'"));
        assert_eq!(fs::read_to_string(root.join(STATE).join("later")).unwrap(), "CODECS=no\nNVIDIA=none\nAPPS='com.brave.Browser com.spotify.Client'\n");
        assert!(fs::read_to_string(root.join("etc/xdg/mimeapps.list")).unwrap().contains("x-scheme-handler/https=com.brave.Browser.desktop"));
        assert_eq!(fs::read_to_string(root.join(STATE).join("usb/choice")).unwrap(), "home\njan\n");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn defaults_without_choices() {
        let h: Handoff = serde_json::from_str(r#"{"userName":"jan"}"#).unwrap();
        assert!(h.proprietary && h.auto_updates && h.browser == "org.mozilla.firefox");
        assert!(later(&h).starts_with("CODECS=yes\nNVIDIA=nvidia\n"));
        assert!(!schema_override(&h, false).contains("accent-color"));
    }
}
