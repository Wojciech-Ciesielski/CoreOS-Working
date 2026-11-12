//! Dostęp do systemu w pierwszej konfiguracji. Działa jako zalogowany użytkownik:
//! sieć przez NetworkManager (nmcli), wygląd przez GSettings, aplikacje przez Flatpak (Flathub),
//! zmiany systemu (kodeki, sterowniki) przez pomocnika z uprawnieniami (pkexec → transactional-update).
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

pub const STATE_DIR: &str = "/var/lib/shrimp-setup";
/// Znacznik zostawiany przez post-install.sh; usuwa go shrimp-setup-apply po zakończeniu.
pub const FIRST_BOOT_MARKER: &str = "/var/lib/shrimp-setup/first-boot";
pub const APPLY_HELPER: &str = "/usr/libexec/shrimp-setup-apply";
pub const USB_HELPER: &str = "/usr/libexec/shrimp-usb-backup";

pub fn first_boot_pending() -> bool {
    Path::new(FIRST_BOOT_MARKER).exists()
}

#[derive(Debug)]
pub struct CmdError(pub String);

async fn run(cmd: &str, args: &[&str]) -> Result<String, CmdError> {
    let out = Command::new(cmd)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .await
        .map_err(|e| CmdError(format!("{cmd}: {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(CmdError(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

// ---------- informacje z instalacji ----------

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Handoff {
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    beside: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub full_name: String,
    pub beside: bool,
    pub wired: bool,
    pub nvidia: Option<String>,
    pub windows_apps: Option<Vec<String>>,
    pub store_apps: Option<Vec<String>>,
}

pub async fn info() -> Info {
    let h: Handoff = std::fs::read_to_string(format!("{STATE_DIR}/install.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // Lista programów z Shrimp Install (post-install.sh kopiuje shrimp.json z nośnika).
    let (windows_apps, store_apps) = std::fs::read_to_string(format!("{STATE_DIR}/shrimp.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .map(|v| windows_apps_of(&v))
        .unwrap_or((None, None));
    let full_name = if h.full_name.is_empty() { real_name() } else { h.full_name };
    Info { full_name, beside: h.beside, wired: wired_online().await, nvidia: nvidia_gpu(), windows_apps, store_apps }
}

/// `apps[].name` i `storeApps` z shrimp.json. Odpowiedniki dobiera interfejs (windows-apps.json).
pub fn windows_apps_of(v: &serde_json::Value) -> (Option<Vec<String>>, Option<Vec<String>>) {
    let apps = v["apps"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str().map(str::to_string)).collect());
    let store = v["storeApps"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect());
    (apps, store)
}

fn real_name() -> String {
    let user = std::env::var("USER").unwrap_or_default();
    std::fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with(&format!("{user}:")))
        .and_then(|l| l.split(':').nth(4))
        .map(|g| g.split(',').next().unwrap_or("").to_string())
        .unwrap_or(user)
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

#[derive(Serialize, Clone)]
pub struct Network {
    pub ssid: String,
    pub secure: bool,
    pub strength: u8,
}

/// Internet jest, gdy NetworkManager tak mówi. Bez sprawdzania łączności w NetworkManagerze
/// („unknown”) wystarcza stan „connected” (jest trasa domyślna).
pub async fn online() -> bool {
    match run("nmcli", &["networking", "connectivity", "check"]).await.map(|s| s.trim().to_string()) {
        Ok(s) if s == "full" => true,
        Ok(s) if s == "unknown" => matches!(run("nmcli", &["-t", "-f", "STATE", "general"]).await, Ok(g) if g.trim() == "connected"),
        _ => false,
    }
}

async fn wired_online() -> bool {
    let devs = run("nmcli", &["-t", "-f", "TYPE,STATE", "device"]).await.unwrap_or_default();
    devs.lines().any(|l| l.starts_with("ethernet:connected")) && online().await
}

/// Sieci Wi-Fi bez powtórzeń (najsilniejszy punkt dla każdej nazwy), od najsilniejszej.
/// Lista z NetworkManagera to wynik OSTATNIEGO skanowania: tuż po starcie i zaraz po „rescan” bywa pusta,
/// bo skan trwa kilka sekund (karty Intel kilkanaście). Wcześniej Setup pytał raz i pokazywał „Nie widzę sieci
/// Wi-Fi”; kolejne kliknięcie „Wyszukaj ponownie” dawało dopiero wynik poprzedniego skanu. Teraz: włącza radio
/// Wi-Fi, prosi o skan i czeka na wynik (do ok. 12 s), gdy lista jest pusta.
pub async fn networks() -> Vec<Network> {
    let list = || async {
        parse_networks(&run("nmcli", &["-t", "-f", "SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "no"]).await.unwrap_or_default())
    };
    let first = list().await;
    if !first.is_empty() {
        // Skan w tle na następny odczyt (lista ma być świeża, gdy ktoś wróci do kroku).
        let _ = run("nmcli", &["device", "wifi", "rescan"]).await;
        return first;
    }
    let _ = run("nmcli", &["radio", "wifi", "on"]).await;
    let _ = run("rfkill", &["unblock", "wifi"]).await;
    for i in 0..4 {
        let _ = run("nmcli", &["device", "wifi", "rescan"]).await;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let n = list().await;
        if !n.is_empty() {
            eprintln!("shrimp-setup: Wi-Fi: sieci widać po {} s", (i + 1) * 3);
            return n;
        }
    }
    eprintln!("shrimp-setup: Wi-Fi: brak sieci po skanowaniu, {}", wifi_diag().await.summary());
    Vec::new()
}

/// Stan karty Wi-Fi: czy system ją widzi (sterownik i firmware), czy radio jest włączone, czy nie ma blokady.
#[derive(Serialize, Default, Debug)]
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

pub async fn wifi_diag() -> WifiDiag {
    let devs = run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE", "device"]).await.unwrap_or_default();
    let devices: Vec<String> = devs.lines().filter_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() >= 3 && f[1] == "wifi").then(|| format!("{}:{}", f[0], f[2]))
    }).collect();
    let radio = run("nmcli", &["radio", "wifi"]).await.map(|s| s.trim().to_string()).unwrap_or_default();
    let blocked = match run("rfkill", &["-n", "-o", "TYPE,SOFT,HARD", "list", "wifi"]).await {
        Ok(o) => {
            let (mut soft, mut hard) = (false, false);
            for l in o.lines() { let f: Vec<&str> = l.split_whitespace().collect(); if f.len() >= 3 { soft |= f[1] == "blocked"; hard |= f[2] == "blocked"; } }
            match (soft, hard) { (true, true) => "soft+hard", (true, false) => "soft", (false, true) => "hard", _ => "" }.to_string()
        }
        Err(_) => String::new(),
    };
    // Bez urządzenia Wi-Fi: co system widzi na PCI (karta bez sterownika albo bez firmware).
    let hardware = if devices.is_empty() {
        run("lspci", &["-nnk"]).await.map(|o| {
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

#[derive(Deserialize)]
pub struct Connect {
    pub ssid: String,
    pub password: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    /// "none" | "wpa2" | "wpa3" (tylko ukryta sieć).
    pub security: Option<String>,
}

pub async fn connect(c: &Connect) -> Result<(), CmdError> {
    if c.hidden {
        let name = format!("{} (Shrimp)", c.ssid);
        let mut args: Vec<&str> = vec!["connection", "add", "type", "wifi", "con-name", &name, "ssid", &c.ssid, "802-11-wireless.hidden", "yes"];
        let pw = c.password.clone().unwrap_or_default();
        match c.security.as_deref() {
            Some("wpa2") => args.extend(["wifi-sec.key-mgmt", "wpa-psk", "wifi-sec.psk", &pw]),
            Some("wpa3") => args.extend(["wifi-sec.key-mgmt", "sae", "wifi-sec.psk", &pw]),
            _ => {}
        }
        run("nmcli", &args).await?;
        if let Err(e) = run("nmcli", &["--wait", "30", "connection", "up", &name]).await {
            let _ = run("nmcli", &["connection", "delete", &name]).await;
            return Err(e);
        }
        return Ok(());
    }
    let pw = c.password.as_deref().filter(|p| !p.is_empty());
    // Profil z poprzedniej, nieudanej próby (stare hasło) psuje kolejną: nmcli bierze go zamiast nowego hasła.
    forget(&c.ssid).await;
    let mut args = vec!["--wait", "30", "device", "wifi", "connect", c.ssid.as_str()];
    if let Some(pw) = pw {
        args.extend(["password", pw]);
    }
    let first = match run("nmcli", &args).await {
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
        forget(name).await;
        let r = async {
            run("nmcli", &["connection", "add", "type", "wifi", "ifname", "*", "con-name", name, "ssid", name,
                "wifi-sec.key-mgmt", mgmt, "wifi-sec.psk", pw]).await?;
            run("nmcli", &["--wait", "30", "connection", "up", "id", name]).await
        }.await;
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
    forget(name).await;
    Err(CmdError(errs.join(" / ")))
}

/// Usuwa profile Wi-Fi o tej nazwie (nmcli nazywa profil nazwą sieci).
async fn forget(ssid: &str) {
    let list = run("nmcli", &["-t", "-f", "NAME,TYPE", "connection", "show"]).await.unwrap_or_default();
    for line in list.lines() {
        if let Some((n, t)) = line.rsplit_once(':') {
            if t == "802-11-wireless" && n.replace("\\:", ":") == ssid {
                let _ = run("nmcli", &["connection", "delete", "id", ssid]).await;
                return;
            }
        }
    }
}

/// Układ klawiatury w bieżącej sesji GNOME. Setup podaje układ jak Agama/XKB: „pl”, „pl(qwertz)”.
pub async fn input_source(keymap: &str) -> Result<(), CmdError> {
    let ok = !keymap.is_empty() && keymap.chars().all(|c| c.is_ascii_alphanumeric() || "()_-".contains(c));
    if !ok {
        return Err(CmdError(format!("zły układ klawiatury: {keymap}")));
    }
    let xkb = keymap.replace('(', "+").replace(')', "");
    run("gsettings", &["set", "org.gnome.desktop.input-sources", "sources", &format!("[('xkb', '{xkb}')]")]).await.map(|_| ())
}

#[derive(Deserialize)]
pub struct Advanced {
    pub ip: String,
    pub dns: String,
    pub proxy: String,
}

/// DNS na aktywnym połączeniu, proxy w GSettings. „Ręczny” adres IP ustawia się w Ustawieniach
/// (makieta nie ma pól adresu), więc tutaj go nie zmieniamy.
pub async fn advanced(a: &Advanced) -> Result<(), CmdError> {
    let dns = match a.dns.as_str() {
        "cloudflare" => Some("1.1.1.1 1.0.0.1"),
        "google" => Some("8.8.8.8 8.8.4.4"),
        _ => None,
    };
    let active = run("nmcli", &["-t", "-f", "NAME,TYPE", "connection", "show", "--active"]).await.unwrap_or_default();
    for name in active.lines().filter(|l| !l.contains(":loopback")).filter_map(|l| l.rsplit_once(':').map(|x| x.0)) {
        let name = name.replace("\\:", ":");
        let (servers, ignore) = match dns { Some(d) => (d, "yes"), None => ("", "no") };
        run("nmcli", &["connection", "modify", &name, "ipv4.dns", servers, "ipv4.ignore-auto-dns", ignore]).await?;
        let _ = run("nmcli", &["connection", "up", &name]).await;
    }
    let mode = if a.proxy == "auto" { "auto" } else { "none" };
    run("gsettings", &["set", "org.gnome.system.proxy", "mode", mode]).await?;
    let _ = &a.ip;
    Ok(())
}

// ---------- wygląd ----------

const ACCENTS: [&str; 9] = ["blue", "teal", "green", "yellow", "orange", "red", "pink", "purple", "slate"];

pub async fn appearance(scheme: &str, accent: Option<&str>) -> Result<(), CmdError> {
    let scheme = if scheme == "prefer-dark" { "prefer-dark" } else { "default" };
    run("gsettings", &["set", "org.gnome.desktop.interface", "color-scheme", scheme]).await?;
    if let Some(a) = accent.filter(|a| ACCENTS.contains(a)) {
        run("gsettings", &["set", "org.gnome.desktop.interface", "accent-color", a]).await?;
    }
    Ok(())
}

// ---------- aplikacje ----------

/// Dodatki, bez których aplikacja z Flathub nie działa albo kapryśnie się instaluje. Steam potrzebuje
/// 32-bitowych bibliotek GL (gry i sam klient) oraz kodeków do wideo w sklepie. Instalowane przed Steamem,
/// błąd któregokolwiek nie blokuje reszty (Flatpak dociągnie, co jeszcze brakuje).
pub fn app_dependencies(id: &str) -> &'static [&'static str] {
    match id {
        "com.valvesoftware.Steam" => &["org.freedesktop.Platform.GL32.default", "org.freedesktop.Platform.ffmpeg-full"],
        _ => &[],
    }
}

const SHRIMP_APPS: &str = "/usr/bin/shrimp-apps";

/// Aplikacje instaluje sklep Shrimp (shrimp-apps --install), a nie bezpośrednio flatpak:
/// to jego repozytorium i backend pobierają wszystko. Bez sklepu (np. test na obrazie bez niego) działa flatpak.
pub async fn install_app(id: &str) -> Result<(), CmdError> {
    if std::path::Path::new(SHRIMP_APPS).exists() {
        return run(SHRIMP_APPS, &["--install", id]).await.map(|_| ());
    }
    let _ = run("flatpak", &["remote-add", "--system", "--if-not-exists", "flathub", "https://dl.flathub.org/repo/flathub.flatpakrepo"]).await;
    for dep in app_dependencies(id) {
        if let Err(e) = run("flatpak", &["install", "--system", "--noninteractive", "-y", "flathub", dep]).await {
            eprintln!("shrimp-setup: {id}: dodatek {dep}: {}", e.0);
        }
    }
    run("flatpak", &["install", "--system", "--noninteractive", "-y", "flathub", id]).await.map(|_| ())
}

// ---------- pendrive z Shrimp Install ----------

#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usb {
    #[serde(default)]
    pub pending: bool,
    /// Pendrive z kopii jest teraz podłączony.
    #[serde(default)]
    pub present: bool,
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub files: u64,
}

/// Czy po instalacji czeka zawartość pendrive'a i czy pendrive jest podłączony (bez uprawnień).
pub async fn usb_status() -> Usb {
    run(USB_HELPER, &["status"]).await.ok().and_then(|o| parse_usb(&o)).unwrap_or_default()
}

pub fn parse_usb(out: &str) -> Option<Usb> {
    serde_json::from_str(out.lines().last()?).ok()
}

/// Formatuje podłączony pendrive i wgrywa na niego pliki (pkexec; pomocnik sam sprawdza kopię i tożsamość dysku).
pub async fn usb_restore(device: &str) -> Result<(), CmdError> {
    run("pkexec", &[USB_HELPER, "restore", device]).await.map(|_| ())
}

/// Pliki do ~/Pendrive: gdy pendrive nie wróci na miejsce albo użytkownik zrezygnuje.
pub async fn usb_to_home() -> Result<(), CmdError> {
    run("pkexec", &[USB_HELPER, "to-home"]).await.map(|_| ())
}

/// Zestaw podstawowy (Firefox i aplikacje GNOME z /usr/share/shrimp/base-apps.json) przez sklep Shrimp.
/// Brak sklepu albo manifestu nie jest błędem (np. test), pojedyncze nieudane aplikacje też nie.
pub async fn install_base_apps() -> Result<(), CmdError> {
    if !std::path::Path::new(SHRIMP_APPS).exists() || !std::path::Path::new("/usr/share/shrimp/base-apps.json").exists() {
        return Ok(());
    }
    run(SHRIMP_APPS, &["--install", "--base"]).await.map(|_| ())
}

// ---------- zakończenie ----------

/// Kodeki i sterowniki (jedna migawka transactional-update) oraz zdjęcie znacznika pierwszego
/// uruchomienia. Zwraca true, jeśli trzeba uruchomić komputer ponownie.
pub async fn apply(codecs: bool, nvidia: &str) -> Result<bool, CmdError> {
    let out = run(
        "pkexec",
        &[APPLY_HELPER, "--codecs", if codecs { "yes" } else { "no" }, "--nvidia", nvidia],
    )
    .await?;
    Ok(out.lines().any(|l| l.trim() == "reboot"))
}

pub async fn reboot() {
    let _ = run("systemctl", &["reboot"]).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_gets_32bit_gl() {
        assert!(app_dependencies("com.valvesoftware.Steam").iter().any(|d| d.contains("GL32")));
        assert!(app_dependencies("org.mozilla.firefox").is_empty());
    }

    #[test]
    fn usb_status_json() {
        let u = parse_usb("{\"pending\":true,\"present\":true,\"device\":\"/dev/sdb\",\"bytes\":5,\"files\":2}\n").unwrap();
        assert!(u.pending && u.present && u.device == "/dev/sdb" && u.files == 2);
        assert_eq!(parse_usb("{\"pending\":false}").unwrap(), Usb::default());
        assert!(parse_usb("śmieci").is_none());
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
    fn windows_apps_from_contract_example() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install-schema/shrimp.example.json");
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        let (apps, store) = windows_apps_of(&v);
        assert_eq!(apps.unwrap()[1], "Steam");
        assert_eq!(store.unwrap(), vec!["SpotifyAB.SpotifyMusic".to_string()]);
    }

    #[test]
    fn windows_apps_from_shrimp_json() {
        let v = serde_json::json!({ "apps": [{ "name": "Steam", "version": "2.10" }, { "publisher": "x" }], "storeApps": ["SpotifyAB.SpotifyMusic"] });
        let (a, s) = windows_apps_of(&v);
        assert_eq!(a.unwrap(), ["Steam"]);
        assert_eq!(s.unwrap(), ["SpotifyAB.SpotifyMusic"]);
        assert_eq!(windows_apps_of(&serde_json::json!({})), (None, None));
    }

    #[test]
    fn pci_ids_lookup() {
        let db = "# c\n10de  NVIDIA Corporation\n\t2504  GA106 [GeForce RTX 3060 Lite Hash Rate]\n\t\t1462 397d  sub\n\t1f82  TU117 [GeForce GTX 1650]\n10df  Emulex\n";
        assert_eq!(pci_name_in(db, "10de", "2504").as_deref(), Some("NVIDIA GeForce RTX 3060 Lite Hash Rate"));
        assert_eq!(pci_name_in(db, "10de", "1f82").as_deref(), Some("NVIDIA GeForce GTX 1650"));
        assert_eq!(pci_name_in(db, "10de", "ffff"), None);
    }
}
