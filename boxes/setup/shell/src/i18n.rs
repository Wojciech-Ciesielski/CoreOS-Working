//! Teksty Setup z `boxes/setup/i18n/` (wspólny format Shrimp, zob. boxes/shrimp-i18n). Język zmienia się
//! w trakcie działania (krok „Wybierz język”), więc bieżący znacznik jest tu, a nie w zmiennych środowiska.
use std::sync::RwLock;

const NS: &str = "setup";

shrimp_i18n::texts!();
include!(concat!(env!("OUT_DIR"), "/langs.rs"));

fn ns() -> &'static str {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| shrimp_i18n::register(NS, SHRIMP_I18N_KEYS, SHRIMP_I18N_LANGS));
    NS
}

static LANG: RwLock<String> = RwLock::new(String::new());

/// Znacznik języka interfejsu (np. „pl”, „pt-br”); pusty = angielski.
pub fn lang() -> String {
    LANG.read().map(|l| if l.is_empty() { "en".to_string() } else { l.clone() }).unwrap_or_else(|_| "en".into())
}

/// Ustawia język interfejsu na znacznik z langs.ts; zwraca, czy pismo idzie od prawej do lewej.
pub fn set_lang(tag: &str) -> bool {
    if let Ok(mut l) = LANG.write() {
        *l = tag.to_string();
    }
    GNOME_LANGS.iter().any(|(_, t, _, rtl)| *t == tag && *rtl)
}

pub fn t(key: &'static str) -> &'static str {
    shrimp_i18n::tr_in(ns(), &lang(), key)
}

/// `t` z podstawieniem `{name}` itd.
pub fn tf(key: &'static str, args: &[(&str, &str)]) -> String {
    shrimp_i18n::fill(t(key), args)
}

/// Rozmiar w GB z jednym miejscem po przecinku poniżej 10 GB („3,2 GB” po polsku).
pub fn gb(bytes_or_gb: f64) -> String {
    let v = if bytes_or_gb >= 10.0 { format!("{:.0}", bytes_or_gb) } else { format!("{:.1}", bytes_or_gb) };
    let comma = matches!(lang().as_str(), "pl" | "de" | "fr" | "es" | "it" | "pt-br" | "pt-pt" | "ru" | "uk" | "cs" | "sk" | "nl" | "sv" | "nb" | "nn" | "da" | "fi" | "hu" | "ro" | "bg" | "hr" | "sl" | "lt" | "lv" | "et" | "tr" | "el" | "ca" | "id" | "vi");
    format!("{} GB", if comma { v.replace('.', ",") } else { v })
}

/// „Zalecane” bez nawiasów.
pub fn recommended() -> String {
    t("recommended").trim_matches(|c| matches!(c, '(' | ')' | '（' | '）')).to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn translations_consistent() {
        let bad = shrimp_i18n::check(super::ns());
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[test]
    fn languages_from_langs_ts() {
        assert!(super::GNOME_LANGS.iter().any(|l| l.0 == "pl_PL.UTF-8" && l.1 == "pl" && l.2 == "Polski"));
        assert!(super::GNOME_LANGS.iter().any(|l| l.0 == "en_GB.UTF-8" && l.2 == "English (UK)"));
        assert!(super::GNOME_LANGS.iter().any(|l| l.1 == "ar" && l.3));
        super::set_lang("pl");
        assert_eq!(super::t("next"), "Dalej");
        assert_eq!(super::gb(3.3), "3,3 GB");
        assert_eq!(super::gb(120.4), "120 GB");
    }
}
