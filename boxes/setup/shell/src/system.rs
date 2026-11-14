//! Dostęp do systemu z kreatora Setup na nośniku: sieć przez NetworkManager (nmcli), układ klawiatury
//! sesji (GSettings), karta NVIDIA. Wywołania blokują, więc okno woła je w osobnym wątku (gio::spawn_blocking).
use std::process::Command;

#[derive(Debug)]
pub struct CmdError(pub String);

fn run(cmd: &str, args: &[&str]) -> Result<String, CmdError> {
    let out = Command::new(cmd)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| CmdError(format!("{cmd}: {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(CmdError(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

// ---------- karta NVIDIA ----------

/// Pierwsza karta graficzna NVIDIA (PCI vendor 0x10de, klasa 0x03xxxx) z nazwą z pci.ids.
pub fn nvidia_gpu() -> Option<String> {
    let dir = std::fs::read_dir("/sys/bus/pci/devices").ok()?;
    for e in dir.flatten() {
        let p = e.path();
        let rd = |f: &str| std::fs::read_to_string(p.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
        if rd("vendor") == "0x10de" && rd("class").starts_with("0x03") {
            let dev = rd("device").trim_start_matches("0x").to_string();
            return Some(pci_name("10de", &dev).unwrap_or_else(|| "NVIDIA".into()));
        }
    }
    None
}

/// Nazwa urządzenia z bazy pci.ids, np. „NVIDIA GeForce RTX 3060”.
pub fn pci_name(vendor: &str, device: &str) -> Option<String> {
    let db = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids", "/usr/share/pci.ids"]
        .iter()
        .find_map(|f| std::fs::read_to_string(f).ok())?;
    pci_name_in(&db, vendor, device)
}

pub fn pci_name_in(db: &str, vendor: &str, device: &str) -> Option<String> {
    let mut in_vendor = false;
    for line in db.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            in_vendor = line.to_lowercase().starts_with(vendor);
            continue;
        }
        if in_vendor && !line.starts_with("\t\t") && line[1..].to_lowercase().starts_with(device) {
            let raw = line[1..][device.len()..].trim();
            // „GA106 [GeForce RTX 3060 Lite Hash Rate]” → „NVIDIA GeForce RTX 3060 Lite Hash Rate”
            let name = match (raw.find('['), raw.rfind(']')) {
                (Some(a), Some(b)) if b > a => &raw[a + 1..b],
                _ => raw,
            };
            return Some(format!("NVIDIA {name}"));
        }
    }
    None
}

// ---------- sieć ----------

#[derive(Clone, Debug, PartialEq)]
pub struct Network {
    pub ssid: String,
    pub secure: bool,
    pub strength: u8,
}

/// Repozytorium pakietów z nośnika jest zamontowane (instalacja offline).
pub fn offline_repo() -> bool {
    std::fs::read_to_string("/usr/share/agama/products.d/shrimp.yaml").is_ok_and(|s| s.contains("url: dir:///run/shrimp-repo"))
}

/// Internet jest, gdy NetworkManager tak mówi. Bez sprawdzania łączności w NetworkManagerze
/// („unknown”) wystarcza stan „connected” (jest trasa domyślna).
pub fn online() -> bool {
    match run("nmcli", &["networking", "connectivity", "check"]).map(|s| s.trim().to_string()) {
        Ok(s) if s == "full" => true,
        Ok(s) if s == "unknown" => matches!(run("nmcli", &["-t", "-f", "STATE", "general"]), Ok(g) if g.trim() == "connected"),
        _ => false,
    }
}

/// Sieci Wi-Fi bez powtórzeń (najsilniejszy punkt dla każdej nazwy), od najsilniejszej.
/// Lista z NetworkManagera to wynik OSTATNIEGO skanowania: tuż po starcie i zaraz po „rescan” bywa pusta,
/// bo skan trwa kilka sekund (karty Intel kilkanaście). Wcześniej Setup pytał raz i pokazywał „Nie widzę sieci
/// Wi-Fi”; kolejne kliknięcie „Wyszukaj ponownie” dawało dopiero wynik poprzedniego skanu. Teraz: włącza radio
/// Wi-Fi, prosi o skan i czeka na wynik (do ok. 12 s), gdy lista jest pusta.
pub fn networks() -> Vec<Network> {
    let list = || {
        parse_networks(&run("nmcli", &["-t", "-f", "SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "no"]).unwrap_or_default())
    };
    let first = list();
    if !first.is_empty() {
        // Skan w tle na następny odczyt (lista ma być świeża, gdy ktoś wróci do kroku).
        let _ = run("nmcli", &["device", "wifi", "rescan"]);
        return first;
    }
    let _ = run("nmcli", &["radio", "wifi", "on"]);
    let _ = run("rfkill", &["unblock", "wifi"]);
    for i in 0..4 {
        let _ = run("nmcli", &["device", "wifi", "rescan"]);
        std::thread::sleep(std::time::Duration::from_secs(3));
        let n = list();
        if !n.is_empty() {
            eprintln!("shrimp-setup: Wi-Fi: sieci widać po {} s", (i + 1) * 3);
            return n;
        }
    }
    eprintln!("shrimp-setup: Wi-Fi: brak sieci po skanowaniu, {}", wifi_diag().summary());
    Vec::new()
}

/// Stan karty Wi-Fi: czy system ją widzi (sterownik i firmware), czy radio jest włączone, czy nie ma blokady.
#[derive(Default, Debug)]
pub struct WifiDiag {
    /// Urządzenia Wi-Fi widziane przez NetworkManager: „nazwa:stan” (np. „wlan0:disconnected”).
    pub devices: Vec<String>,
    /// „enabled” / „disabled” / pusty, gdy nmcli nie odpowiada.
    pub radio: String,
    /// Blokada rfkill: „soft”, „hard”, „soft+hard” albo pusty.
    pub blocked: String,
    /// Karty sieciowe PCI/USB z „lspci -k” (sterownik) skrócone do jednej linii, gdy nie ma urządzenia Wi-Fi.
    pub hardware: String,
}

impl WifiDiag {
    pub fn summary(&self) -> String {
        format!("urządzenia: {:?}, radio: {}, rfkill: {}, sprzęt: {}", self.devices, self.radio, self.blocked, self.hardware)
    }
}

pub fn wifi_diag() -> WifiDiag {
    let devs = run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE", "device"]).unwrap_or_default();
    let devices: Vec<String> = devs.lines().filter_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() >= 3 && f[1] == "wifi").then(|| format!("{}:{}", f[0], f[2]))
    }).collect();
    let radio = run("nmcli", &["radio", "wifi"]).map(|s| s.trim().to_string()).unwrap_or_default();
    let blocked = match run("rfkill", &["-n", "-o", "TYPE,SOFT,HARD", "list", "wifi"]) {
        Ok(o) => {
            let (mut soft, mut hard) = (false, false);
            for l in o.lines() { let f: Vec<&str> = l.split_whitespace().collect(); if f.len() >= 3 { soft |= f[1] == "blocked"; hard |= f[2] == "blocked"; } }
            match (soft, hard) { (true, true) => "soft+hard", (true, false) => "soft", (false, true) => "hard", _ => "" }.to_string()
        }
        Err(_) => String::new(),
    };
    // Bez urządzenia Wi-Fi: co system widzi na PCI (karta bez sterownika albo bez firmware).
    let hardware = if devices.is_empty() {
        run("lspci", &["-nnk"]).map(|o| {
            let mut out = Vec::new();
            let lines: Vec<&str> = o.lines().collect();
            for (i, l) in lines.iter().enumerate() {
                if l.contains("Network controller") || l.contains("Wireless") || l.contains("802.11") {
                    let drv = lines.iter().skip(i + 1).take(3).find(|x| x.contains("Kernel driver in use")).map(|x| x.trim().to_string()).unwrap_or_else(|| "brak sterownika".into());
                    out.push(format!("{} [{}]", l.trim(), drv));
                }
            }
            out.join("; ")
        }).unwrap_or_default()
    } else { String::new() };
    WifiDiag { devices, radio, blocked, hardware }
}

/// Pola nmcli -t są rozdzielone „:”; dwukropek w nazwie jest poprzedzony „\”.
pub fn parse_networks(out: &str) -> Vec<Network> {
    let mut nets: Vec<Network> = Vec::new();
    for line in out.lines() {
        let mut fields = Vec::new();
        let mut cur = String::new();
        let mut esc = false;
        for c in line.chars() {
            match (esc, c) {
                (true, _) => { cur.push(c); esc = false; }
                (false, '\\') => esc = true,
                (false, ':') => fields.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            }
        }
        fields.push(cur);
        if fields.len() < 3 || fields[0].is_empty() {
            continue;
        }
        let n = Network {
            ssid: fields[0].clone(),
            strength: fields[1].parse().unwrap_or(0),
            secure: !(fields[2].is_empty() || fields[2] == "--"),
        };
        match nets.iter_mut().find(|x| x.ssid == n.ssid) {
            Some(x) if x.strength < n.strength => *x = n,
            Some(_) => {}
            None => nets.push(n),
        }
    }
    nets.sort_by(|a, b| b.strength.cmp(&a.strength));
    nets
}

#[derive(Default)]
pub struct Connect {
    pub ssid: String,
    pub password: Option<String>,
    pub hidden: bool,
    /// "none" | "wpa2" | "wpa3" (tylko ukryta sieć).
    pub security: Option<String>,
}

pub fn connect(c: &Connect) -> Result<(), CmdError> {
    if c.hidden {
        let name = format!("{} (Shrimp)", c.ssid);
        let mut args: Vec<&str> = vec!["connection", "add", "type", "wifi", "con-name", &name, "ssid", &c.ssid, "802-11-wireless.hidden", "yes"];
        let pw = c.password.clone().unwrap_or_default();
        match c.security.as_deref() {
            Some("wpa2") => args.extend(["wifi-sec.key-mgmt", "wpa-psk", "wifi-sec.psk", &pw]),
            Some("wpa3") => args.extend(["wifi-sec.key-mgmt", "sae", "wifi-sec.psk", &pw]),
            _ => {}
        }
        run("nmcli", &args)?;
        if let Err(e) = run("nmcli", &["--wait", "30", "connection", "up", &name]) {
            let _ = run("nmcli", &["connection", "delete", &name]);
            return Err(e);
        }
        return Ok(());
    }
    let pw = c.password.as_deref().filter(|p| !p.is_empty());
    // Profil z poprzedniej, nieudanej próby (stare hasło) psuje kolejną: nmcli bierze go zamiast nowego hasła.
    forget(&c.ssid);
    let mut args = vec!["--wait", "30", "device", "wifi", "connect", c.ssid.as_str()];
    if let Some(pw) = pw {
        args.extend(["password", pw]);
    }
    let first = match run("nmcli", &args) {
        Ok(_) => return Ok(()),
        Err(e) => e,
    };
    eprintln!("shrimp-setup: nmcli device wifi connect: {}", first.0);
    // nmcli sam zgaduje zabezpieczenia sieci i przy części routerów (WPA2/WPA3) się myli
    // („key-mgmt: property is missing”, „Secrets were required”). Kolejne podejścia z profilem wprost:
    // WPA2 (wpa-psk), a potem WPA3 (sae) dla routerów w trybie tylko WPA3.
    let Some(pw) = pw else { return Err(first) };
    let name = c.ssid.as_str();
    let mut errs = vec![first.0];
    for mgmt in ["wpa-psk", "sae"] {
        forget(name);
        let r = (|| {
            run("nmcli", &["connection", "add", "type", "wifi", "ifname", "*", "con-name", name, "ssid", name,
                "wifi-sec.key-mgmt", mgmt, "wifi-sec.psk", pw])?;
            run("nmcli", &["--wait", "30", "connection", "up", "id", name])
        })();
        match r {
            Ok(_) => {
                eprintln!("shrimp-setup: Wi-Fi {name}: połączono profilem {mgmt}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("shrimp-setup: nmcli connection up ({mgmt}): {}", e.0);
                errs.push(format!("{mgmt}: {}", e.0));
            }
        }
    }
    forget(name);
    Err(CmdError(errs.join(" / ")))
}

/// Usuwa profile Wi-Fi o tej nazwie (nmcli nazywa profil nazwą sieci).
fn forget(ssid: &str) {
    let list = run("nmcli", &["-t", "-f", "NAME,TYPE", "connection", "show"]).unwrap_or_default();
    for line in list.lines() {
        if let Some((n, t)) = line.rsplit_once(':') {
            if t == "802-11-wireless" && n.replace("\\:", ":") == ssid {
                let _ = run("nmcli", &["connection", "delete", "id", ssid]);
                return;
            }
        }
    }
}

/// Układ klawiatury w bieżącej sesji GNOME. Setup podaje układ jak Agama/XKB: „pl”, „pl(qwertz)”.
pub fn input_source(keymap: &str) -> Result<(), CmdError> {
    let ok = !keymap.is_empty() && keymap.chars().all(|c| c.is_ascii_alphanumeric() || "()_-".contains(c));
    if !ok {
        return Err(CmdError(format!("zły układ klawiatury: {keymap}")));
    }
    let xkb = keymap.replace('(', "+").replace(')', "");
    run("gsettings", &["set", "org.gnome.desktop.input-sources", "sources", &format!("[('xkb', '{xkb}')]")]).map(|_| ())
}

// ---------- przeglądarka, dock, preferencje ----------

/// Typy, które otwiera domyślna przeglądarka (Ustawienia › Aplikacje domyślne › Sieć).
pub const BROWSER_TYPES: [&str; 6] =
    ["x-scheme-handler/http", "x-scheme-handler/https", "text/html", "application/xhtml+xml", "x-scheme-handler/about", "x-scheme-handler/unknown"];

/// Identyfikator Flatpaka jak z Flathub (org.mozilla.firefox, com.google.Chrome).
pub fn valid_app_id(id: &str) -> bool {
    id.split('.').count() >= 3 && id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// `mimeapps.list` z wpisami `[Default Applications]` ustawionymi na `desktop` dla `types`; reszta pliku bez zmian.
pub fn mimeapps_with(text: &str, types: &[&str], desktop: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_default = false;
    let mut seen_default = false;
    let mut pending: Vec<&str> = types.to_vec();
    let flush = |out: &mut Vec<String>, pending: &mut Vec<&str>| {
        for t in pending.drain(..) {
            out.push(format!("{t}={desktop}"));
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_default {
                flush(&mut out, &mut pending);
            }
            in_default = trimmed == "[Default Applications]";
            seen_default |= in_default;
            out.push(line.to_string());
            continue;
        }
        if in_default {
            if let Some((k, _)) = trimmed.split_once('=') {
                if let Some(i) = pending.iter().position(|t| *t == k.trim()) {
                    out.push(format!("{}={desktop}", pending.remove(i)));
                    continue;
                }
                if types.contains(&k.trim()) {
                    continue;
                }
            }
        }
        out.push(line.to_string());
    }
    if in_default {
        flush(&mut out, &mut pending);
    }
    if !seen_default {
        if out.last().is_some_and(|l| !l.is_empty()) {
            out.push(String::new());
        }
        out.push("[Default Applications]".into());
        flush(&mut out, &mut pending);
    }
    out.join("\n") + "\n"
}

/// Dock na start (lista właściciela, 2026-11-13): przeglądarka, Steam, Pliki, sklep, Kalkulator, Kalendarz,
/// LibreOffice, Ustawienia. Pliki = Filer, a bez niego Nautilus.
pub fn dock(browser: &str, filer: bool) -> Vec<String> {
    let files = if filer { "org.shrimp.Filer" } else { "org.gnome.Nautilus" };
    [browser, "com.valvesoftware.Steam", files, "org.shrimp.Apps", "org.gnome.Calculator", "org.gnome.Calendar", "org.libreoffice.LibreOffice", "org.gnome.Settings"]
        .iter()
        .map(|id| format!("{id}.desktop"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_browser_in_mimeapps() {
        let t = ["x-scheme-handler/http", "x-scheme-handler/https"];
        assert_eq!(mimeapps_with("", &t, "a.desktop"), "[Default Applications]\nx-scheme-handler/http=a.desktop\nx-scheme-handler/https=a.desktop\n");
        let old = "[Added Associations]\ntext/plain=x.desktop\n[Default Applications]\nx-scheme-handler/http=old.desktop\nimage/png=v.desktop\n";
        let new = mimeapps_with(old, &t, "b.desktop");
        assert!(new.contains("x-scheme-handler/http=b.desktop") && new.contains("x-scheme-handler/https=b.desktop"));
        assert!(new.contains("image/png=v.desktop") && new.contains("text/plain=x.desktop") && !new.contains("old.desktop"));
        assert_eq!(new.matches("[Default Applications]").count(), 1);
    }

    #[test]
    fn dock_order() {
        let d = dock("com.brave.Browser", true);
        assert_eq!(d[0], "com.brave.Browser.desktop");
        assert!(d.contains(&"org.shrimp.Filer.desktop".to_string()) && !d.iter().any(|x| x.contains("Cherry") || x.contains("Panko")));
        assert!(dock("org.mozilla.firefox", false).contains(&"org.gnome.Nautilus.desktop".to_string()));
        assert!(valid_app_id("org.mozilla.firefox") && !valid_app_id("x; rm") && !valid_app_id("firefox"));
    }

    #[test]
    fn nmcli_parsing_dedupes_and_unescapes() {
        let out = "Dom 5G:80:WPA2\nDom 5G:40:WPA2\nKawiarnia:40:\nA\\:B:60:WPA3\n:20:WPA2\n";
        let n = parse_networks(out);
        assert_eq!(n.iter().map(|x| x.ssid.as_str()).collect::<Vec<_>>(), ["Dom 5G", "A:B", "Kawiarnia"]);
        assert!(n[0].secure && !n[2].secure);
        assert_eq!(n[0].strength, 80);
    }

    #[test]
    fn pci_ids_lookup() {
        let db = "# c\n10de  NVIDIA Corporation\n\t2504  GA106 [GeForce RTX 3060 Lite Hash Rate]\n\t\t1462 397d  sub\n\t1f82  TU117 [GeForce GTX 1650]\n10df  Emulex\n";
        assert_eq!(pci_name_in(db, "10de", "2504").as_deref(), Some("NVIDIA GeForce RTX 3060 Lite Hash Rate"));
        assert_eq!(pci_name_in(db, "10de", "1f82").as_deref(), Some("NVIDIA GeForce GTX 1650"));
        assert_eq!(pci_name_in(db, "10de", "ffff"), None);
    }
}
