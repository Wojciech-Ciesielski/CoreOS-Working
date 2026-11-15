//! Nazwy krajów i języków w języku interfejsu z pakietu iso-codes (JSON z nazwami angielskimi i tłumaczenia
//! gettext .mo z iso-codes-lang), godzina w strefie z GLib. Bez tłumaczenia zostaje nazwa angielska, bez
//! iso-codes kod kraju albo języka.
use crate::data::{Keyboard, Region};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const ISO_JSON: &str = "/usr/share/iso-codes/json";
const LOCALE_DIR: &str = "/usr/share/locale";

/// Tłumaczenia z pliku .mo (format GNU gettext, little albo big endian).
pub fn parse_mo(b: &[u8]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if b.len() < 28 {
        return out;
    }
    let le = match b[0..4] {
        [0xde, 0x12, 0x04, 0x95] => true,
        [0x95, 0x04, 0x12, 0xde] => false,
        _ => return out,
    };
    let u = |o: usize| -> Option<usize> {
        let s: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?;
        Some(if le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) } as usize)
    };
    let (Some(n), Some(orig), Some(trans)) = (u(8), u(12), u(16)) else { return out };
    let s = |tab: usize, i: usize| -> Option<String> {
        let len = u(tab + i * 8)?;
        let off = u(tab + i * 8 + 4)?;
        String::from_utf8(b.get(off..off + len)?.to_vec()).ok()
    };
    for i in 0..n {
        if let (Some(k), Some(v)) = (s(orig, i), s(trans, i)) {
            if !k.is_empty() && !v.is_empty() {
                // Liczba mnoga i kontekst: tylko pierwsza forma.
                out.insert(k.split('\0').next().unwrap_or("").to_string(), v.split('\0').next().unwrap_or("").to_string());
            }
        }
    }
    out
}

/// Katalogi języka dla locale: „pt_BR.UTF-8” → pt_BR, pt; „sr_RS@latin” → sr@latin, sr_RS, sr.
fn locale_dirs(locale: &str) -> Vec<String> {
    let base = locale.split('.').next().unwrap_or("");
    let (main, modi) = match base.split_once('@') {
        Some((m, x)) => (m, Some(x)),
        None => (base, None),
    };
    let lang = main.split('_').next().unwrap_or("");
    let mut v = Vec::new();
    if let Some(m) = modi {
        v.push(format!("{lang}@{m}"));
    }
    v.push(main.to_string());
    v.push(lang.to_string());
    v.dedup();
    v
}

type Catalog = HashMap<String, String>;

fn catalog(domain: &str, locale: &str) -> Catalog {
    static CACHE: OnceLock<Mutex<HashMap<String, Catalog>>> = OnceLock::new();
    let key = format!("{domain}|{locale}");
    let cache = CACHE.get_or_init(Default::default);
    if let Some(c) = cache.lock().unwrap().get(&key) {
        return c.clone();
    }
    let c = locale_dirs(locale)
        .iter()
        .find_map(|d| std::fs::read(format!("{LOCALE_DIR}/{d}/LC_MESSAGES/{domain}.mo")).ok())
        .map(|b| parse_mo(&b))
        .unwrap_or_default();
    cache.lock().unwrap().insert(key, c.clone());
    c
}

/// Lista z iso-codes: kod → nazwa angielska.
fn english(file: &str, list: &str, code_field: &str) -> &'static HashMap<String, String> {
    static ALL: OnceLock<Mutex<HashMap<String, &'static HashMap<String, String>>>> = OnceLock::new();
    let key = format!("{file}|{code_field}");
    let all = ALL.get_or_init(Default::default);
    if let Some(m) = all.lock().unwrap().get(&key) {
        return m;
    }
    let mut m = HashMap::new();
    if let Ok(v) = std::fs::read_to_string(format!("{ISO_JSON}/{file}.json")).map(|s| serde_json::from_str::<serde_json::Value>(&s).unwrap_or_default()) {
        for e in v[list].as_array().into_iter().flatten() {
            if let (Some(c), Some(n)) = (e[code_field].as_str(), e["name"].as_str()) {
                m.insert(c.to_lowercase(), n.to_string());
            }
        }
    }
    let m: &'static HashMap<String, String> = Box::leak(Box::new(m));
    all.lock().unwrap().insert(key, m);
    m
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Nazwa kraju (kod ISO 3166-1, np. „PL”) w języku `locale`.
pub fn country(code: &str, locale: &str) -> String {
    let Some(en) = english("iso_3166-1", "3166-1", "alpha_2").get(&code.to_lowercase()) else { return code.to_string() };
    catalog("iso_3166-1", locale).get(en).cloned().unwrap_or_else(|| en.clone())
}

/// Nazwa języka (ISO 639, dwie albo trzy litery) w języku `locale`, z wielkiej litery.
pub fn language(code: &str, locale: &str) -> String {
    let c = code.to_lowercase();
    let field = if c.len() == 2 { "alpha_2" } else { "alpha_3" };
    let found = english("iso_639-2", "639-2", field).get(&c).map(|n| ("iso_639-2", n)).or_else(|| english("iso_639-3", "639-3", field).get(&c).map(|n| ("iso_639-3", n)));
    let Some((domain, en)) = found else { return code.to_string() };
    // „Greek, Modern (1453-)” → tłumaczenie albo pierwsza część nazwy.
    let name = catalog(domain, locale).get(en).cloned().unwrap_or_else(|| en.split([',', ';', '(']).next().unwrap_or(en).trim().to_string());
    capitalize(&name)
}

/// „Polska”, „Stany Zjednoczone · Nowy Jork”: kraj z iso-codes, miasto z tłumaczeń Setup.
pub fn region_label(r: &Region, locale: &str) -> String {
    let c = country(r.code, locale);
    match r.city {
        Some(city) => format!("{c} · {}", crate::i18n::t(city_key(city))),
        None => c,
    }
}

fn city_key(city: &str) -> &'static str {
    match city {
        "newyork" => "city.newyork",
        "chicago" => "city.chicago",
        "denver" => "city.denver",
        "losangeles" => "city.losangeles",
        "toronto" => "city.toronto",
        "vancouver" => "city.vancouver",
        "canary" => "city.canary",
        "sydney" => "city.sydney",
        _ => "city.perth",
    }
}

/// „Polski (programisty)”, „Angielski (Stany Zjednoczone)”.
pub fn keyboard_label(k: &Keyboard, locale: &str) -> String {
    let lang = language(k.lang, locale);
    let mut extra: Vec<String> = Vec::new();
    if let Some(r) = k.region {
        // 419 = Ameryka Łacińska (kod UN M.49, nie ma go w ISO 3166-1).
        extra.push(if r == "419" { "Latinoamérica".into() } else { country(r, locale) });
    }
    if let Some(n) = k.note {
        extra.push(crate::i18n::t(if n == "kb.programmer" { "kb.programmer" } else { "kb.typist" }).to_string());
    }
    if extra.is_empty() { lang } else { format!("{lang} ({})", extra.join(", ")) }
}

/// Godzina w strefie, np. „14:05”.
pub fn time_in(tz: &str) -> String {
    #[allow(deprecated)]
    let zone = gtk::glib::TimeZone::new(Some(tz));
    gtk::glib::DateTime::now(&zone).ok().and_then(|d| d.format("%H:%M").ok()).map(|s| s.to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mo_roundtrip() {
        // Minimalny plik .mo z jednym tłumaczeniem „Poland” → „Polska”.
        let (k, v) = (b"Poland", "Polska".as_bytes());
        let mut b = vec![0xde, 0x12, 0x04, 0x95];
        let push = |b: &mut Vec<u8>, x: u32| b.extend_from_slice(&x.to_le_bytes());
        push(&mut b, 0);
        push(&mut b, 1);
        push(&mut b, 28);
        push(&mut b, 36);
        push(&mut b, 0);
        push(&mut b, 0);
        let off = 44u32;
        push(&mut b, k.len() as u32);
        push(&mut b, off);
        push(&mut b, v.len() as u32);
        push(&mut b, off + k.len() as u32 + 1);
        b.extend_from_slice(k);
        b.push(0);
        b.extend_from_slice(v);
        b.push(0);
        assert_eq!(parse_mo(&b).get("Poland").map(String::as_str), Some("Polska"));
        assert!(parse_mo(b"zly plik").is_empty());
    }

    #[test]
    fn dirs_for_locale() {
        assert_eq!(locale_dirs("pt_BR.UTF-8"), ["pt_BR", "pt"]);
        assert_eq!(locale_dirs("sr_RS@latin"), ["sr@latin", "sr_RS", "sr"]);
    }

    #[test]
    fn english_fallbacks() {
        // Bez tłumaczeń (środowisko CI) zostaje nazwa angielska albo kod.
        let pl = country("PL", "xx_XX.UTF-8");
        assert!(pl == "Poland" || pl == "PL");
        assert_eq!(country("ZZ", "pl_PL.UTF-8"), "ZZ");
        let el = language("el", "xx_XX.UTF-8");
        assert!(el == "Greek" || el == "el" || el.starts_with("Greek"));
    }
}
