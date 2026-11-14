//! Czyste funkcje Setup: konto, podpowiedzi z Shrimp Install, przeglądarki i programy z Windows.
use crate::data::{Browser, Keyboard, Lang, Region, BROWSERS, KLID_XKB, LANGID_XKB, TZ_ALIAS};
use fancy_regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

/// Pierwsze słowo imienia i nazwiska („Cześć, Anna”).
pub fn first_name(full: &str) -> &str {
    full.split_whitespace().next().unwrap_or("")
}

/// Nazwa użytkownika: małe litery, bez polskich znaków, nie zaczyna się cyfrą, najwyżej 32 znaki (useradd).
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        let rep: &str = match c {
            'ł' => "l", 'ø' => "o", 'đ' | 'ð' => "d", 'ß' => "ss", 'æ' => "ae", 'œ' => "oe", 'ı' => "i", 'þ' => "th",
            'ą' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' => "a",
            'ć' | 'ç' | 'č' | 'ĉ' | 'ċ' => "c", 'ď' => "d",
            'ę' | 'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ě' => "e",
            'ğ' | 'ĝ' | 'ģ' => "g", 'ĥ' => "h",
            'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' => "i", 'ĵ' => "j", 'ķ' => "k", 'ĺ' | 'ļ' | 'ľ' => "l",
            'ń' | 'ñ' | 'ň' | 'ņ' => "n",
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'ō' | 'ő' => "o", 'ŕ' | 'ř' => "r",
            'ś' | 'š' | 'ş' | 'ș' | 'ŝ' => "s", 'ť' | 'ţ' | 'ț' => "t",
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'ų' => "u", 'ý' | 'ÿ' => "y",
            'ź' | 'ż' | 'ž' => "z",
            c if c.is_ascii_lowercase() || c.is_ascii_digit() => {
                out.push(c);
                continue;
            }
            _ => "",
        };
        out.push_str(rep);
    }
    out.trim_start_matches(|c: char| c.is_ascii_digit()).chars().take(32).collect()
}

/// Konta i grupy, które istnieją w systemie (Leap 16 + GNOME + Shrimp); konto użytkownika nie może się tak nazywać.
const RESERVED_USERS: &[&str] = &[
    "root", "bin", "daemon", "adm", "lp", "sync", "shutdown", "halt", "mail", "news", "uucp", "operator", "games", "ftp",
    "nobody", "man", "messagebus", "polkitd", "systemd", "dbus", "gdm", "rtkit", "pipewire", "pulse", "avahi", "colord",
    "geoclue", "flatpak", "nm", "dnsmasq", "chrony", "srvgeoclue", "sshd", "tss", "usbmux", "wheel", "users", "audio",
    "video", "input", "render", "kvm", "shadow", "sudo", "tty", "disk", "utmp", "admin", "shrimp", "agama", "snapper",
];

pub fn is_reserved_user(u: &str) -> bool {
    RESERVED_USERS.contains(&u)
}

/// Proponowana nazwa konta z imienia i nazwiska (albo z nazwy konta Windows). Imię bez liter łacińskich daje „user”;
/// nazwa zajęta przez system dostaje cyfrę („admin1”).
pub fn login_from(full: &str, windows_name: &str) -> String {
    let mut u = slug(windows_name);
    if u.is_empty() {
        u = slug(first_name(full));
    }
    if u.is_empty() && !full.trim().is_empty() {
        u = "user".into();
    }
    if is_reserved_user(&u) {
        u.truncate(31);
        u.push('1');
    }
    u
}

/// Dane z Shrimp Install (shrimp.json na nośniku), tylko pola, których używa Setup.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Transfer {
    #[serde(default)]
    pub user: TransferUser,
    #[serde(default)]
    pub locale: TransferLocale,
    #[serde(default)]
    pub bitlocker: bool,
    /// Pliki do przeniesienia (bajty): tyle miejsca więcej potrzebuje Shrimp.
    #[serde(default)]
    pub bytes: u64,
    /// Nazwy programów z Windows i pakiety Microsoft Store.
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub store_apps: Vec<String>,
    /// Domyślna przeglądarka Windows (ProgId).
    #[serde(default)]
    pub default_browser: Option<String>,
    /// Foldery z plikami: (nazwa XDG, pliki, bajty).
    #[serde(default)]
    pub folders: Vec<(String, u64, u64)>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferUser {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub full_name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TransferLocale {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub keyboards: Vec<String>,
}

/// Język z Windows (pl-PL, sr-Latn-RS, de…) → indeks na liście języków albo None.
pub fn lang_index(raw: &str, langs: &[Lang]) -> Option<usize> {
    if raw.is_empty() {
        return None;
    }
    let lc = raw.replacen('-', "_", 1);
    if let Some(i) = langs.iter().position(|l| l.locale.starts_with(&format!("{lc}."))) {
        return Some(i);
    }
    // Zapis BCP 47 ze skryptem (sr-Latn-RS, zh-Hant-TW): najpierw „język-skrypt”, jeśli jest taki tag.
    let parts: Vec<String> = raw.to_lowercase().split(['-', '_']).map(String::from).collect();
    let scripted = parts.iter().skip(1).find(|p| p.len() == 4 && p.chars().all(|c| c.is_ascii_alphabetic())).map(|s| format!("{}-{s}", parts[0]));
    let tag = match scripted.filter(|t| langs.iter().any(|l| l.tag == *t)) {
        Some(t) => t,
        None => shrimp_i18n::tag_for(raw),
    };
    if tag == "en" && !raw.to_lowercase().starts_with("en") {
        return None;
    }
    let country = raw.split(['-', '_']).skip(1).find(|p| p.len() == 2).map(|c| c.to_uppercase());
    if let Some(c) = &country {
        if let Some(i) = langs.iter().position(|l| l.tag == tag && l.locale.split(['.', '@']).next().unwrap_or("").ends_with(&format!("_{c}"))) {
            return Some(i);
        }
    }
    langs.iter().position(|l| l.tag == tag)
}

/// Domyślny język instalacji: z Windows (shrimp.json), na końcu angielski. Nigdy język sesji live.
pub fn install_lang(t: Option<&Transfer>, langs: &[Lang]) -> usize {
    let mut cands: Vec<&str> = Vec::new();
    if let Some(t) = t {
        cands.extend(t.locale.language.as_deref());
        cands.extend(t.locale.format.as_deref());
    }
    cands.extend(["en-US", "en"]);
    cands.iter().find_map(|r| lang_index(r, langs)).unwrap_or(0)
}

/// Układ Windows (KLID, np. „00000415”) → xkb, jeśli Setup go zna.
pub fn klid_to_xkb(klid: &str, known: &[&str]) -> Option<&'static str> {
    let k = klid.to_lowercase();
    let x = KLID_XKB.iter().find(|(a, _)| *a == k).map(|(_, b)| *b).or_else(|| {
        k.strip_prefix("0000").and_then(|lang| LANGID_XKB.iter().find(|(a, _)| *a == lang).map(|(_, b)| *b))
    })?;
    known.contains(&x).then_some(x)
}

#[derive(Debug, Default, PartialEq)]
pub struct Prefill {
    pub lang: Option<usize>,
    pub region: Option<usize>,
    pub kb: Option<usize>,
    pub full: String,
    pub user: String,
}

pub fn prefill(t: &Transfer, langs: &[Lang], regs: &[Region], kbs: &[Keyboard]) -> Prefill {
    let raw = t.locale.language.clone().or(t.locale.format.clone()).unwrap_or_default();
    let lang = lang_index(&raw, langs);
    let tz = match &t.locale.timezone {
        Some(z) => Some(TZ_ALIAS.iter().find(|(a, _)| a == z).map(|(_, b)| b.to_string()).unwrap_or_else(|| z.clone())),
        None => lang.and_then(|i| crate::data::default_tz(&langs[i].locale)).map(String::from),
    };
    let region = tz.and_then(|z| regs.iter().position(|r| r.tz == z));
    let keys: Vec<&str> = kbs.iter().map(|k| k.keymap).collect();
    let kb = t.locale.keyboards.iter().find_map(|k| klid_to_xkb(k, &keys)).and_then(|x| keys.iter().position(|k| *k == x));
    let full = t.user.full_name.trim().to_string();
    let user = login_from(&full, &t.user.name);
    Prefill { lang, region, kb, full, user }
}

fn re(pattern: &str) -> Option<Regex> {
    Regex::new(&format!("(?i){pattern}")).ok()
}

fn matches(pattern: &str, s: &str) -> bool {
    re(pattern).is_some_and(|r| r.is_match(s.trim()).unwrap_or(false))
}

/// Przeglądarki w kroku „Przeglądarka”: pięć stałych i te z Windows spoza listy (Opera, Vivaldi).
pub fn browser_choices(names: &[String], prog_id: Option<&str>) -> Vec<Browser> {
    BROWSERS
        .iter()
        .filter(|b| !b.extra || names.iter().any(|n| matches(b.re, n)) || prog_id.is_some_and(|p| matches(b.prog, p)))
        .copied()
        .collect()
}

/// Przeglądarka z Windows: domyślna (ProgId), a bez niej pierwsza zainstalowana; Edge liczy się tylko jako domyślna.
pub fn windows_browser(list: &[Browser], names: &[String], prog_id: Option<&str>) -> Option<usize> {
    if let Some(p) = prog_id {
        if let Some(i) = list.iter().position(|b| matches(b.prog, p)) {
            return Some(i);
        }
    }
    list.iter().position(|b| b.id != "com.microsoft.Edge" && names.iter().any(|n| matches(b.re, n)))
}

pub fn is_browser(id: &str) -> bool {
    BROWSERS.iter().any(|b| b.id == id)
}

/// Program z Windows i jego odpowiednik.
#[derive(Clone, Debug, PartialEq)]
pub struct WinApp {
    pub windows: String,
    pub name: String,
    pub id: String,
}

#[derive(Deserialize)]
struct CatalogJson {
    apps: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
struct CatalogEntry {
    windows: String,
    #[serde(default)]
    top: Option<u32>,
    re: String,
    #[serde(default)]
    store: Option<String>,
    linux: Option<Linux>,
    #[serde(default)]
    base: bool,
    #[serde(default)]
    skip: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Linux {
    name: String,
    id: String,
}

struct CatalogApp {
    windows: String,
    re: Option<Regex>,
    store: Option<Regex>,
    linux: Option<Linux>,
    base: bool,
    skip: bool,
}

/// Katalog programów Windows (data/windows-apps.json) w kolejności podpowiedzi: najpierw 100 najpopularniejszych
/// (wg `top`), potem reszta w kolejności wpisów.
fn catalog() -> &'static [CatalogApp] {
    static C: OnceLock<Vec<CatalogApp>> = OnceLock::new();
    C.get_or_init(|| {
        let j: CatalogJson = serde_json::from_str(include_str!("../data/windows-apps.json")).expect("windows-apps.json");
        let mut top: Vec<&CatalogEntry> = j.apps.iter().filter(|a| a.top.is_some()).collect();
        top.sort_by_key(|a| a.top);
        top.into_iter()
            .chain(j.apps.iter().filter(|a| a.top.is_none()))
            .map(|a| CatalogApp {
                windows: a.windows.clone(),
                re: re(&a.re),
                store: a.store.as_deref().and_then(re),
                linux: a.linux.clone(),
                base: a.base,
                skip: a.skip.is_some(),
            })
            .collect()
    })
}

fn hit(m: &CatalogApp, names: &[String], store: &[String]) -> bool {
    m.re.as_ref().is_some_and(|r| names.iter().any(|n| r.is_match(n.trim()).unwrap_or(false)))
        || m.store.as_ref().is_some_and(|r| store.iter().any(|s| r.is_match(s).unwrap_or(false)))
}

/// Programy z shrimp.json → odpowiedniki z Flathub, bez powtórzeń, bez nieznanych i bez tych, które system ma od razu.
pub fn map_windows_apps(names: &[String], store: &[String]) -> Vec<WinApp> {
    let mut out: Vec<WinApp> = Vec::new();
    for m in catalog() {
        let Some(l) = &m.linux else { continue };
        if m.base || out.iter().any(|a| a.id == l.id) {
            continue;
        }
        if hit(m, names, store) {
            out.push(WinApp { windows: m.windows.clone(), name: l.name.clone(), id: l.id.clone() });
        }
    }
    out
}

/// Programy z Windows bez odpowiednika (wpis bez `linux` i bez `skip`), bez powtórzeń.
pub fn no_alternative(names: &[String], store: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in catalog() {
        if m.linux.is_some() || m.skip || out.contains(&m.windows) {
            continue;
        }
        if hit(m, names, store) {
            out.push(m.windows.clone());
        }
    }
    out
}

/// Ile programów z Windows pokazujemy w kroku „Programy” (najpopularniejsze najpierw).
pub const MAX_WINDOWS_APPS: usize = 12;

/// „Twoje programy z Windows”: bez przeglądarek (mają własny krok).
pub fn windows_list(apps: &[WinApp]) -> Vec<WinApp> {
    apps.iter().filter(|a| !is_browser(&a.id)).take(MAX_WINDOWS_APPS).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{langs, KEYBOARDS, REGIONS};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn account_names() {
        assert_eq!(slug("Łukasz"), "lukasz");
        assert_eq!(slug("Żaneta-Ćwik"), "zanetacwik");
        assert_eq!(first_name("  Anna Kowalska "), "Anna");
        assert_eq!(slug("2pac"), "pac");
        assert_eq!(slug(&"x".repeat(40)).len(), 32);
        assert_eq!(login_from("李明", ""), "user");
        assert_eq!(login_from("  ", ""), "");
        assert_eq!(login_from("Admin Kowalski", ""), "admin1");
        assert_eq!(login_from("", "root"), "root1");
        assert!(is_reserved_user("gdm") && !is_reserved_user("anna"));
    }

    #[test]
    fn windows_programs() {
        let w = map_windows_apps(&s(&["Spotify", "Microsoft 365 Apps for business - pl-pl", "Microsoft Office Proofing Tools 2016 - Polski", "Nieznany program"]), &[]);
        assert_eq!(w.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["com.spotify.Client", "org.libreoffice.LibreOffice"]);
        let w = map_windows_apps(
            &s(&["Adobe Photoshop 2024", "Adobe Photoshop Lightroom Classic", "Zoom Workplace", "Epic Games Launcher", "GOG GALAXY", "Google Chrome"]),
            &s(&["SpotifyAB.SpotifyMusic", "Microsoft.WindowsCalculator"]),
        );
        assert_eq!(w.iter().map(|x| x.windows.as_str()).collect::<Vec<_>>(), ["Google Chrome", "Spotify", "Zoom", "Epic Games Launcher", "Adobe Photoshop", "Adobe Lightroom"]);
        assert!(map_windows_apps(&s(&["Microsoft Office File Validation Add-In"]), &[]).is_empty());
        let w = map_windows_apps(&s(&["Google Chrome", "Spotify"]), &[]);
        assert_eq!(windows_list(&w).iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["com.spotify.Client"]);
        let got = map_windows_apps(&s(&["Zotero", "Discord", "Google Chrome", "Spotify"]), &[]);
        assert_eq!(got.iter().take(3).map(|a| a.windows.as_str()).collect::<Vec<_>>(), ["Google Chrome", "Spotify", "Discord"]);
        assert!(map_windows_apps(&s(&["NVIDIA Graphics Driver 560.94", "Microsoft Visual C++ 2015-2022 Redistributable (x64)", "Avast Free Antivirus", "7-Zip 23.01"]), &[]).is_empty());
        assert_eq!(no_alternative(&s(&["TeamViewer 15", "Spotify", "Microsoft Visio - pl-pl", "TeamViewer"]), &[]), ["Microsoft Visio", "TeamViewer"]);
    }

    #[test]
    fn catalog_is_sane() {
        let c = catalog();
        assert!(c.len() >= 500);
        assert!(c.iter().all(|a| a.re.is_some()), "wzorzec, którego nie rozumie fancy-regex");
        let own = c.iter().filter(|a| a.re.as_ref().unwrap().is_match(&a.windows).unwrap_or(false)).count();
        assert!(own * 10 > c.len() * 9);
        for a in c {
            let r = a.re.as_ref().unwrap();
            assert!(!r.is_match("").unwrap() && !r.is_match("Nieznany program 1.0").unwrap(), "{}", a.windows);
        }
    }

    #[test]
    fn browsers() {
        let names = s(&["Opera Stable 114.0", "Microsoft Edge", "Google Chrome"]);
        let list = browser_choices(&names, None);
        assert_eq!(list.iter().map(|b| b.name).collect::<Vec<_>>(), ["Firefox", "Google Chrome", "Chromium", "Brave", "Microsoft Edge", "Opera"]);
        assert_eq!(list[windows_browser(&list, &names, Some("MSEdgeHTM")).unwrap()].name, "Microsoft Edge");
        assert_eq!(list[windows_browser(&list, &names, None).unwrap()].name, "Google Chrome");
        assert_eq!(windows_browser(&browser_choices(&[], None), &s(&["Microsoft Edge"]), None), None);
        assert_eq!(browser_choices(&[], Some("FirefoxURL-308046B0AF4A39CB")).len(), 5);
    }

    #[test]
    fn shrimp_install_hints() {
        let known: Vec<&str> = KEYBOARDS.iter().map(|k| k.keymap).collect();
        assert_eq!(klid_to_xkb("00000415", &known), Some("pl"));
        assert_eq!(klid_to_xkb("00010415", &known), Some("pl(qwertz)"));
        assert_eq!(klid_to_xkb("0000040C", &known), Some("fr"));
        assert_eq!(klid_to_xkb("00000816", &known), None);
        assert_eq!(klid_to_xkb("E0010411", &known), None);
        let l = langs();
        let t: Transfer = serde_json::from_value(serde_json::json!({
            "user": { "name": "Jan", "fullName": "Jan Kowalski" },
            "locale": { "language": "de-DE", "timezone": "Europe/Kiev", "keyboards": ["E0010411", "00000407"] }
        })).unwrap();
        let p = prefill(&t, &l, REGIONS, KEYBOARDS);
        assert_eq!(l[p.lang.unwrap()].locale, "de_DE.UTF-8");
        assert_eq!(REGIONS[p.region.unwrap()].tz, "Europe/Kyiv");
        assert_eq!(KEYBOARDS[p.kb.unwrap()].keymap, "de");
        assert_eq!((p.full.as_str(), p.user.as_str()), ("Jan Kowalski", "jan"));
        let t: Transfer = serde_json::from_value(serde_json::json!({ "locale": { "language": "pl-PL", "timezone": null }, "user": { "fullName": "Łucja Nowak" } })).unwrap();
        let p = prefill(&t, &l, REGIONS, KEYBOARDS);
        assert_eq!(REGIONS[p.region.unwrap()].tz, "Europe/Warsaw");
        assert_eq!(p.user, "lucja");
        let t: Transfer = serde_json::from_value(serde_json::json!({ "locale": { "language": "xx-XX" } })).unwrap();
        let p = prefill(&t, &l, REGIONS, KEYBOARDS);
        assert_eq!((p.lang, p.region, p.kb), (None, None, None));
    }

    #[test]
    fn install_language() {
        let l = langs();
        let tr = |v: serde_json::Value| serde_json::from_value::<Transfer>(v).unwrap();
        assert_eq!(l[install_lang(Some(&tr(serde_json::json!({ "locale": { "language": "de-DE" } }))), &l)].locale, "de_DE.UTF-8");
        assert_eq!(l[install_lang(None, &l)].locale, "en_US.UTF-8");
        assert_eq!(l[install_lang(Some(&tr(serde_json::json!({ "locale": { "language": "xx-YY" } }))), &l)].locale, "en_US.UTF-8");
        assert_eq!(l[install_lang(Some(&tr(serde_json::json!({ "locale": { "language": "pl-PL" } }))), &l)].locale, "pl_PL.UTF-8");
        assert_eq!(l[install_lang(Some(&tr(serde_json::json!({ "locale": { "language": "sr-Latn-RS" } }))), &l)].tag, "sr-latn");
    }
}
