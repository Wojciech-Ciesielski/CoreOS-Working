//! Stałe dane Setup. Teksty są w boxes/setup/i18n (klucze); tu zostają identyfikatory, kolejność i dane techniczne.

/// Kraj (kod ISO), strefa czasowa i klucz miasta, gdy kraj ma kilka stref.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub code: &'static str,
    pub tz: &'static str,
    pub city: Option<&'static str>,
}

/// Układ xkb w formacie Agamy („układ(wariant)”); nazwa to język (i kraj), `note` to klucz tłumaczenia wariantu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyboard {
    pub keymap: &'static str,
    pub lang: &'static str,
    pub region: Option<&'static str>,
    pub note: Option<&'static str>,
}

/// Język z listy GNOME: locale dla Agamy, znacznik tłumaczeń, nazwa w tym języku.
#[derive(Clone, Debug, PartialEq)]
pub struct Lang {
    pub locale: String,
    pub tag: String,
    pub name: String,
}

pub fn langs() -> Vec<Lang> {
    crate::i18n::GNOME_LANGS.iter().map(|(l, t, n, _)| Lang { locale: l.to_string(), tag: t.to_string(), name: n.to_string() }).collect()
}

pub static REGIONS: &[Region] = &[
    Region { code: "PL", tz: "Europe/Warsaw", city: None },
    Region { code: "DE", tz: "Europe/Berlin", city: None },
    Region { code: "AT", tz: "Europe/Vienna", city: None },
    Region { code: "BE", tz: "Europe/Brussels", city: None },
    Region { code: "HR", tz: "Europe/Zagreb", city: None },
    Region { code: "CZ", tz: "Europe/Prague", city: None },
    Region { code: "DK", tz: "Europe/Copenhagen", city: None },
    Region { code: "EE", tz: "Europe/Tallinn", city: None },
    Region { code: "FI", tz: "Europe/Helsinki", city: None },
    Region { code: "FR", tz: "Europe/Paris", city: None },
    Region { code: "GR", tz: "Europe/Athens", city: None },
    Region { code: "ES", tz: "Europe/Madrid", city: None },
    Region { code: "ES", tz: "Atlantic/Canary", city: Some("canary") },
    Region { code: "NL", tz: "Europe/Amsterdam", city: None },
    Region { code: "IE", tz: "Europe/Dublin", city: None },
    Region { code: "IS", tz: "Atlantic/Reykjavik", city: None },
    Region { code: "LT", tz: "Europe/Vilnius", city: None },
    Region { code: "LV", tz: "Europe/Riga", city: None },
    Region { code: "NO", tz: "Europe/Oslo", city: None },
    Region { code: "PT", tz: "Europe/Lisbon", city: None },
    Region { code: "RO", tz: "Europe/Bucharest", city: None },
    Region { code: "SK", tz: "Europe/Bratislava", city: None },
    Region { code: "CH", tz: "Europe/Zurich", city: None },
    Region { code: "SE", tz: "Europe/Stockholm", city: None },
    Region { code: "TR", tz: "Europe/Istanbul", city: None },
    Region { code: "UA", tz: "Europe/Kyiv", city: None },
    Region { code: "HU", tz: "Europe/Budapest", city: None },
    Region { code: "GB", tz: "Europe/London", city: None },
    Region { code: "IT", tz: "Europe/Rome", city: None },
    Region { code: "BG", tz: "Europe/Sofia", city: None },
    Region { code: "SI", tz: "Europe/Ljubljana", city: None },
    Region { code: "RS", tz: "Europe/Belgrade", city: None },
    Region { code: "BA", tz: "Europe/Sarajevo", city: None },
    Region { code: "MK", tz: "Europe/Skopje", city: None },
    Region { code: "AL", tz: "Europe/Tirane", city: None },
    Region { code: "ME", tz: "Europe/Podgorica", city: None },
    Region { code: "LU", tz: "Europe/Luxembourg", city: None },
    Region { code: "MT", tz: "Europe/Malta", city: None },
    Region { code: "CY", tz: "Asia/Nicosia", city: None },
    Region { code: "BY", tz: "Europe/Minsk", city: None },
    Region { code: "MD", tz: "Europe/Chisinau", city: None },
    Region { code: "FO", tz: "Atlantic/Faroe", city: None },
    Region { code: "RU", tz: "Europe/Moscow", city: None },
    Region { code: "US", tz: "America/New_York", city: Some("newyork") },
    Region { code: "US", tz: "America/Chicago", city: Some("chicago") },
    Region { code: "US", tz: "America/Denver", city: Some("denver") },
    Region { code: "US", tz: "America/Los_Angeles", city: Some("losangeles") },
    Region { code: "CA", tz: "America/Toronto", city: Some("toronto") },
    Region { code: "CA", tz: "America/Vancouver", city: Some("vancouver") },
    Region { code: "MX", tz: "America/Mexico_City", city: None },
    Region { code: "BR", tz: "America/Sao_Paulo", city: None },
    Region { code: "AR", tz: "America/Argentina/Buenos_Aires", city: None },
    Region { code: "CL", tz: "America/Santiago", city: None },
    Region { code: "CO", tz: "America/Bogota", city: None },
    Region { code: "PE", tz: "America/Lima", city: None },
    Region { code: "VE", tz: "America/Caracas", city: None },
    Region { code: "UY", tz: "America/Montevideo", city: None },
    Region { code: "AE", tz: "Asia/Dubai", city: None },
    Region { code: "SA", tz: "Asia/Riyadh", city: None },
    Region { code: "IQ", tz: "Asia/Baghdad", city: None },
    Region { code: "IR", tz: "Asia/Tehran", city: None },
    Region { code: "IL", tz: "Asia/Jerusalem", city: None },
    Region { code: "JO", tz: "Asia/Amman", city: None },
    Region { code: "LB", tz: "Asia/Beirut", city: None },
    Region { code: "GE", tz: "Asia/Tbilisi", city: None },
    Region { code: "AM", tz: "Asia/Yerevan", city: None },
    Region { code: "AZ", tz: "Asia/Baku", city: None },
    Region { code: "KZ", tz: "Asia/Almaty", city: None },
    Region { code: "UZ", tz: "Asia/Tashkent", city: None },
    Region { code: "KG", tz: "Asia/Bishkek", city: None },
    Region { code: "TJ", tz: "Asia/Dushanbe", city: None },
    Region { code: "MN", tz: "Asia/Ulaanbaatar", city: None },
    Region { code: "IN", tz: "Asia/Kolkata", city: None },
    Region { code: "PK", tz: "Asia/Karachi", city: None },
    Region { code: "BD", tz: "Asia/Dhaka", city: None },
    Region { code: "LK", tz: "Asia/Colombo", city: None },
    Region { code: "NP", tz: "Asia/Kathmandu", city: None },
    Region { code: "BT", tz: "Asia/Thimphu", city: None },
    Region { code: "MM", tz: "Asia/Yangon", city: None },
    Region { code: "TH", tz: "Asia/Bangkok", city: None },
    Region { code: "LA", tz: "Asia/Vientiane", city: None },
    Region { code: "KH", tz: "Asia/Phnom_Penh", city: None },
    Region { code: "VN", tz: "Asia/Ho_Chi_Minh", city: None },
    Region { code: "MY", tz: "Asia/Kuala_Lumpur", city: None },
    Region { code: "SG", tz: "Asia/Singapore", city: None },
    Region { code: "ID", tz: "Asia/Jakarta", city: None },
    Region { code: "PH", tz: "Asia/Manila", city: None },
    Region { code: "CN", tz: "Asia/Shanghai", city: None },
    Region { code: "TW", tz: "Asia/Taipei", city: None },
    Region { code: "HK", tz: "Asia/Hong_Kong", city: None },
    Region { code: "KR", tz: "Asia/Seoul", city: None },
    Region { code: "JP", tz: "Asia/Tokyo", city: None },
    Region { code: "AU", tz: "Australia/Sydney", city: Some("sydney") },
    Region { code: "AU", tz: "Australia/Perth", city: Some("perth") },
    Region { code: "NZ", tz: "Pacific/Auckland", city: None },
    Region { code: "ZA", tz: "Africa/Johannesburg", city: None },
    Region { code: "EG", tz: "Africa/Cairo", city: None },
    Region { code: "ET", tz: "Africa/Addis_Ababa", city: None },
    Region { code: "KE", tz: "Africa/Nairobi", city: None },
    Region { code: "NG", tz: "Africa/Lagos", city: None },
    Region { code: "DZ", tz: "Africa/Algiers", city: None },
    Region { code: "MA", tz: "Africa/Casablanca", city: None },
    Region { code: "RW", tz: "Africa/Kigali", city: None },
    Region { code: "MG", tz: "Indian/Antananarivo", city: None },
];

pub static KEYBOARDS: &[Keyboard] = &[
    Keyboard { keymap: "pl", lang: "pl", region: None, note: Some("kb.programmer") },
    Keyboard { keymap: "pl(qwertz)", lang: "pl", region: None, note: Some("kb.typist") },
    Keyboard { keymap: "us", lang: "en", region: Some("US"), note: None },
    Keyboard { keymap: "gb", lang: "en", region: Some("GB"), note: None },
    Keyboard { keymap: "de", lang: "de", region: None, note: None },
    Keyboard { keymap: "fr", lang: "fr", region: None, note: None },
    Keyboard { keymap: "es", lang: "es", region: None, note: None },
    Keyboard { keymap: "cz", lang: "cs", region: None, note: None },
    Keyboard { keymap: "ua", lang: "uk", region: None, note: None },
    Keyboard { keymap: "ru", lang: "ru", region: None, note: None },
    Keyboard { keymap: "it", lang: "it", region: None, note: None },
    Keyboard { keymap: "pt", lang: "pt", region: Some("PT"), note: None },
    Keyboard { keymap: "br", lang: "pt", region: Some("BR"), note: None },
    Keyboard { keymap: "latam", lang: "es", region: Some("419"), note: None },
    Keyboard { keymap: "nl", lang: "nl", region: None, note: None },
    Keyboard { keymap: "be", lang: "nl", region: Some("BE"), note: None },
    Keyboard { keymap: "ch", lang: "de", region: Some("CH"), note: None },
    Keyboard { keymap: "ca", lang: "fr", region: Some("CA"), note: None },
    Keyboard { keymap: "sk", lang: "sk", region: None, note: None },
    Keyboard { keymap: "se", lang: "sv", region: None, note: None },
    Keyboard { keymap: "no", lang: "nb", region: None, note: None },
    Keyboard { keymap: "dk", lang: "da", region: None, note: None },
    Keyboard { keymap: "fi", lang: "fi", region: None, note: None },
    Keyboard { keymap: "hu", lang: "hu", region: None, note: None },
    Keyboard { keymap: "lt", lang: "lt", region: None, note: None },
    Keyboard { keymap: "lv", lang: "lv", region: None, note: None },
    Keyboard { keymap: "ee", lang: "et", region: None, note: None },
    Keyboard { keymap: "ro", lang: "ro", region: None, note: None },
    Keyboard { keymap: "bg", lang: "bg", region: None, note: None },
    Keyboard { keymap: "hr", lang: "hr", region: None, note: None },
    Keyboard { keymap: "si", lang: "sl", region: None, note: None },
    Keyboard { keymap: "rs", lang: "sr", region: None, note: None },
    Keyboard { keymap: "ba", lang: "bs", region: None, note: None },
    Keyboard { keymap: "mk", lang: "mk", region: None, note: None },
    Keyboard { keymap: "al", lang: "sq", region: None, note: None },
    Keyboard { keymap: "gr", lang: "el", region: None, note: None },
    Keyboard { keymap: "tr", lang: "tr", region: None, note: None },
    Keyboard { keymap: "is", lang: "is", region: None, note: None },
    Keyboard { keymap: "ie", lang: "ga", region: None, note: None },
    Keyboard { keymap: "mt", lang: "mt", region: None, note: None },
    Keyboard { keymap: "by", lang: "be", region: None, note: None },
    Keyboard { keymap: "ge", lang: "ka", region: None, note: None },
    Keyboard { keymap: "am", lang: "hy", region: None, note: None },
    Keyboard { keymap: "az", lang: "az", region: None, note: None },
    Keyboard { keymap: "kz", lang: "kk", region: None, note: None },
    Keyboard { keymap: "kg", lang: "ky", region: None, note: None },
    Keyboard { keymap: "uz", lang: "uz", region: None, note: None },
    Keyboard { keymap: "tj", lang: "tg", region: None, note: None },
    Keyboard { keymap: "mn", lang: "mn", region: None, note: None },
    Keyboard { keymap: "il", lang: "he", region: None, note: None },
    Keyboard { keymap: "ara", lang: "ar", region: None, note: None },
    Keyboard { keymap: "ir", lang: "fa", region: None, note: None },
    Keyboard { keymap: "in", lang: "hi", region: None, note: None },
    Keyboard { keymap: "lk", lang: "si", region: None, note: None },
    Keyboard { keymap: "np", lang: "ne", region: None, note: None },
    Keyboard { keymap: "mm", lang: "my", region: None, note: None },
    Keyboard { keymap: "th", lang: "th", region: None, note: None },
    Keyboard { keymap: "la", lang: "lo", region: None, note: None },
    Keyboard { keymap: "kh", lang: "km", region: None, note: None },
    Keyboard { keymap: "vn", lang: "vi", region: None, note: None },
    Keyboard { keymap: "ph", lang: "fil", region: None, note: None },
    Keyboard { keymap: "jp", lang: "ja", region: None, note: None },
    Keyboard { keymap: "kr", lang: "ko", region: None, note: None },
    Keyboard { keymap: "cn", lang: "zh", region: None, note: None },
    Keyboard { keymap: "et", lang: "am", region: None, note: None },
];

/// Domyślny układ dla języka (locale z krajem ma pierwszeństwo przed samym językiem).
static KB_FOR: &[(&str, &str)] = &[("pl", "pl"), ("en", "us"), ("en_GB", "gb"), ("de", "de"), ("fr", "fr"), ("es", "es"), ("it", "it"), ("pt_PT", "pt"), ("pt_BR", "br"), ("nl", "nl"), ("cs", "cz"), ("sk", "sk"), ("uk", "ua"), ("ru", "ru"), ("sv", "se"), ("nb", "no"), ("nn", "no"), ("da", "dk"), ("fi", "fi"), ("hu", "hu"), ("lt", "lt"), ("lv", "lv"), ("et", "ee"), ("ro", "ro"), ("bg", "bg"), ("hr", "hr"), ("sl", "si"), ("sr", "rs"), ("bs", "ba"), ("mk", "mk"), ("sq", "al"), ("el", "gr"), ("tr", "tr"), ("is", "is"), ("ga", "ie"), ("gd", "gb"), ("cy", "gb"), ("ja", "jp"), ("ko", "kr"), ("zh", "cn"), ("ar", "ara"), ("he", "il"), ("fa", "ir"), ("hi", "in"), ("bn", "in"), ("ta", "in"), ("te", "in"), ("mr", "in"), ("gu", "in"), ("kn", "in"), ("ml", "in"), ("pa", "in"), ("as", "in"), ("or", "in"), ("si", "lk"), ("ne", "np"), ("my", "mm"), ("th", "th"), ("lo", "la"), ("km", "kh"), ("vi", "vn"), ("ka", "ge"), ("hy", "am"), ("az", "az"), ("kk", "kz"), ("ky", "kg"), ("uz", "uz"), ("tg", "tj"), ("mn", "mn"), ("be", "by"), ("mt", "mt"), ("am", "et"), ("ca", "es"), ("eu", "es"), ("gl", "es")];

fn locale_parts(locale: &str) -> (&str, Option<&str>) {
    let base = locale.split(['.', '@']).next().unwrap_or("");
    match base.split_once('_') {
        Some((l, c)) => (l, Some(c)),
        None => (base, None),
    }
}

pub fn default_keymap(locale: &str) -> &'static str {
    let (lang, cc) = locale_parts(locale);
    let full = cc.map(|c| format!("{lang}_{c}")).unwrap_or_default();
    KB_FOR.iter().find(|(k, _)| *k == full).or_else(|| KB_FOR.iter().find(|(k, _)| *k == lang)).map(|(_, v)| *v).unwrap_or("us")
}

/// Domyślna strefa dla języka: pierwszy region z kodem kraju z locale (pl_PL → PL).
pub fn default_tz(locale: &str) -> Option<&'static str> {
    let cc = locale_parts(locale).1?;
    REGIONS.iter().find(|r| r.code == cc).map(|r| r.tz)
}

pub static HELLO: &[&str] = &["Witaj", "Hello", "Hallo", "Bonjour", "Hola", "Ciao"];

/// Kolory dodatkowe GNOME: klucz nazwy, wartość `accent-color`, kolor.
pub static ACCENTS: &[(&str, &str, &str)] = &[
    ("acc.blue", "blue", "#3584e4"), ("acc.teal", "teal", "#2190a4"), ("acc.green", "green", "#3a944a"),
    ("acc.yellow", "yellow", "#c88800"), ("acc.orange", "orange", "#ed5b00"), ("acc.red", "red", "#e62d42"),
    ("acc.pink", "pink", "#d56199"), ("acc.purple", "purple", "#9141ac"), ("acc.slate", "slate", "#6f8396"),
];

/// Części tekstu licencji: nagłówek i treść.
pub static LIC_TEXT: &[(&str, &str)] = &[("lic.codecs.h", "lic.codecs.p"), ("lic.nvidia.h", "lic.nvidia.p"), ("lic.firmware.h", "lic.firmware.p")];

/// Minimum dla Shrimp i zapas ponad zajęte miejsce Windows (GB).
pub const SHRIMP_MIN_GB: u64 = 40;
pub const WINDOWS_SPARE_GB: u64 = 20;

/// Przeglądarki do wyboru (Flathub). `prog`: ProgId domyślnej przeglądarki w Windows (HKCU …\UrlAssociations\https\UserChoice,
/// zapisuje Shrimp Install), `re`: nazwa z listy programów Windows. `extra`: tylko, gdy była w Windows (Opera, Vivaldi).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Browser {
    pub id: &'static str,
    pub name: &'static str,
    pub prog: &'static str,
    pub re: &'static str,
    pub extra: bool,
}

pub static BROWSERS: &[Browser] = &[
    Browser { id: "org.mozilla.firefox", name: "Firefox", prog: "^FirefoxURL", re: "^Mozilla Firefox", extra: false },
    Browser { id: "com.google.Chrome", name: "Google Chrome", prog: "^ChromeHTML", re: "^Google Chrome$", extra: false },
    Browser { id: "org.chromium.Chromium", name: "Chromium", prog: "^ChromiumHTM", re: r"^Chromium\b", extra: false },
    Browser { id: "com.brave.Browser", name: "Brave", prog: "^BraveHTML", re: r"^Brave\b", extra: false },
    Browser { id: "com.microsoft.Edge", name: "Microsoft Edge", prog: "^MSEdgeHTM", re: "^Microsoft Edge(?!.*WebView)", extra: false },
    Browser { id: "com.opera.Opera", name: "Opera", prog: "^Opera", re: r"^Opera\b", extra: true },
    Browser { id: "com.vivaldi.Vivaldi", name: "Vivaldi", prog: "^VivaldiHTM", re: "^Vivaldi(?![A-Za-z0-9])", extra: true },
];

/// Układ klawiatury Windows (KLID) → układ xkb. Poza tą listą liczy się język układu (4 ostatnie cyfry).
pub static KLID_XKB: &[(&str, &str)] = &[
    ("00000415", "pl"), ("00010415", "pl(qwertz)"), ("00000409", "us"), ("00020409", "us"), ("00000809", "gb"),
    ("00000407", "de"), ("0000040c", "fr"), ("0000040a", "es"), ("00000c0a", "es"), ("00000405", "cz"),
    ("00000422", "ua"), ("00020422", "ua"), ("00000419", "ru"),
];
pub static LANGID_XKB: &[(&str, &str)] = &[
    ("0415", "pl"), ("0409", "us"), ("0809", "gb"), ("0407", "de"), ("040c", "fr"), ("040a", "es"), ("0c0a", "es"), ("0405", "cz"), ("0422", "ua"), ("0419", "ru"),
];
/// Stare nazwy stref, które Windows (ICU) może jeszcze zwracać.
pub static TZ_ALIAS: &[(&str, &str)] = &[("Europe/Kiev", "Europe/Kyiv"), ("Asia/Calcutta", "Asia/Kolkata"), ("America/Buenos_Aires", "America/Argentina/Buenos_Aires")];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrKey {
    Pass,
    Dpass,
    Space,
    NoDisk,
    Install,
    Stall,
    Agama,
    User,
    Wifi,
    Offline,
    Repo,
}

/// Klucze tłumaczeń okna błędu: tytuł, opis, przycisk główny, przycisk dodatkowy.
pub fn err_texts(k: ErrKey) -> (&'static str, &'static str, &'static str, Option<&'static str>) {
    use ErrKey::*;
    match k {
        Dpass => ("err.dpass.t", "err.dpass.d", "err.retry", None),
        Pass => ("err.pass.t", "err.pass.d", "err.retry", None),
        Space => ("err.space.t", "err.space.d", "err.space.p", Some("err.space.s")),
        NoDisk => ("err.nodisk.t", "err.nodisk.d", "err.retry", None),
        Install => ("err.install.t", "err.install.d", "err.retry", Some("err.install.s")),
        Stall => ("err.stall.t", "err.stall.d", "err.stall.p", Some("err.install.s")),
        Agama => ("err.agama.t", "err.agama.d", "err.agama.p", Some("err.install.s")),
        User => ("err.user.t", "err.user.d", "err.retry", None),
        Wifi => ("err.wifi.t", "err.wifi.d", "err.retry", None),
        Offline => ("err.net.t", "err.offline.d", "err.retry", None),
        Repo => ("err.repo.t", "err.offline.repo", "err.retry", None),
    }
}

/// Slajdy w czasie instalacji: klucze tytułu i opisu; `win` tylko obok Windows, `extra` dopisek obok Windows.
pub struct Slide {
    pub t: &'static str,
    pub d: &'static str,
    pub win: bool,
    pub extra: Option<&'static str>,
}

pub static SLIDES: &[Slide] = &[
    Slide { t: "slide.linux.t", d: "slide.linux.d", win: false, extra: None },
    Slide { t: "slide.win.t", d: "slide.win.d", win: true, extra: None },
    Slide { t: "slide.apps.t", d: "slide.apps.d", win: false, extra: None },
    Slide { t: "slide.pass.t", d: "slide.pass.d", win: false, extra: None },
    Slide { t: "slide.files.t", d: "slide.files.d", win: false, extra: Some("slide.files.win") },
    Slide { t: "slide.panko.t", d: "slide.panko.d", win: false, extra: None },
    Slide { t: "slide.cocktail.t", d: "slide.cocktail.d", win: false, extra: None },
    Slide { t: "slide.search.t", d: "slide.search.d", win: false, extra: None },
    Slide { t: "slide.help.t", d: "slide.help.d", win: false, extra: None },
];

pub fn slides(beside: bool) -> Vec<&'static Slide> {
    SLIDES.iter().filter(|s| !s.win || beside).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_for_locale() {
        assert_eq!(default_keymap("pl_PL.UTF-8"), "pl");
        assert_eq!(default_keymap("en_GB.UTF-8"), "gb");
        assert_eq!(default_keymap("en_US.UTF-8"), "us");
        assert_eq!(default_keymap("pt_BR.UTF-8"), "br");
        assert_eq!(default_keymap("xx_YY.UTF-8"), "us");
        assert_eq!(default_tz("pl_PL.UTF-8"), Some("Europe/Warsaw"));
        assert_eq!(default_tz("en_US.UTF-8"), Some("America/New_York"));
        assert_eq!(default_tz("en"), None);
    }

    #[test]
    fn regions_unique() {
        for (i, r) in REGIONS.iter().enumerate() {
            assert!(!REGIONS[..i].iter().any(|x| x.tz == r.tz), "{}", r.tz);
        }
        for (i, k) in KEYBOARDS.iter().enumerate() {
            assert!(!KEYBOARDS[..i].iter().any(|x| x.keymap == k.keymap), "{}", k.keymap);
        }
    }
}
