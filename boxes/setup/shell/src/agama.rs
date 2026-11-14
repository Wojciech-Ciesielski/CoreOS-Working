//! Instalacja nad HTTP API Agamy (API v2, Leap 16.1 / Agama 24) na 127.0.0.1, z tokenem z nośnika.
//! Czyste funkcje (dyski, konfiguracja, postęp) mają testy; [`Client`] blokuje, więc okno woła go w wątku.
use crate::data::WINDOWS_SPARE_GB;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const GB: f64 = 1e9;
pub const AGAMA_URL: &str = "http://127.0.0.1";
/// Kopia tokenu Agamy czytelna dla użytkownika nośnika (shrimp-setup-token.service).
const TOKEN_FILES: [&str; 2] = ["/run/shrimp-setup/token", "/run/agama/token"];
/// Etykieta nośnika z Shrimp Install (exFAT z LiveOS) i obrazu ISO (volid w image-base/config.kiwi).
pub const LIVE_LABEL: &str = "SHRIMP_LIVE";
/// Ile nieudanych odpytań /api/status z rzędu (co sekundę) oznacza, że instalator nie odpowiada.
pub const POLL_MISSES: u32 = 30;
/// Po ilu minutach bez żadnej zmiany postępu Setup pyta, czy czekać dalej.
pub const STALL_MIN: u64 = 20;

// ---------- odpowiedzi Agamy (tylko pola, których używamy) ----------

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ADevice {
    pub sid: u64,
    pub name: String,
    pub description: Option<String>,
    pub block: Option<ABlock>,
    pub drive: Option<ADrive>,
    pub filesystem: Option<AFs>,
    pub encryption: Option<Value>,
    pub partition: Option<APartition>,
    pub partition_table: Option<APartitionTable>,
    pub partitions: Vec<ADevice>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ABlock {
    pub size: f64,
    pub encrypted: bool,
    pub systems: Vec<String>,
    pub shrinking: Option<AShrink>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct AShrink {
    pub supported: bool,
    pub min_size: Option<f64>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ADrive {
    pub vendor: Option<String>,
    pub model: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct AFs {
    #[serde(rename = "type")]
    pub kind: String,
    pub label: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct APartition {
    pub efi: bool,
    pub id: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct APartitionTable {
    pub unused_slots: Vec<ASlot>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ASlot {
    pub start: f64,
    pub size: f64,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct ASystem {
    pub l10n: AL10n,
    pub storage: AStorage,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct AL10n {
    pub locales: Vec<AId>,
    pub keymaps: Vec<AId>,
    pub timezones: Vec<AId>,
}

#[derive(Deserialize, Default, Debug)]
pub struct AId {
    pub id: String,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct AStorage {
    pub devices: Vec<ADevice>,
    pub candidate_drives: Option<Vec<u64>>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct AStatus {
    pub stage: String,
    pub tasks: Vec<ATask>,
    pub progresses: Vec<AProgress>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct ATask {
    pub id: u64,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct AProgress {
    pub scope: String,
    pub index: u64,
    pub size: u64,
    pub step: String,
}

#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct AIssue {
    pub scope: String,
    pub class: String,
    pub description: String,
    pub details: Option<String>,
}

// ---------- dyski ----------

/// Dysk, na którym można zainstalować Shrimp. Rozmiary w GB (10^9, jak na etykietach dysków).
#[derive(Clone, Debug, PartialEq)]
pub struct Disk {
    pub device: String,
    pub name: String,
    pub size_gb: u64,
    pub windows: Option<WinPart>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WinPart {
    /// Partycja systemowa Windows (NTFS), którą zmniejszamy przy instalacji „obok”.
    pub partition: String,
    /// Najmniejszy rozmiar Windows: zajęte miejsce + zapas.
    pub min_gb: u64,
    /// Miejsce dzielone suwakiem: partycja Windows + wolne miejsce na dysku.
    pub pool_gb: u64,
    /// BitLocker: partycji nie da się zmniejszyć, więc nie ma „Obok Windows”.
    pub bitlocker: bool,
}

/// Partycja zaszyfrowana BitLockerem: system plików „bitlocker” albo szyfrowanie bez systemu plików
/// z typem partycji danych Windows (libstorage-ng w Leap 16.1, Vivobook).
pub fn is_bitlocker(p: &ADevice) -> bool {
    let probe = format!(
        "{:?}{:?}{:?}{:?}",
        p.filesystem.as_ref().map(|f| &f.kind),
        p.encryption,
        p.description,
        p.block.as_ref().map(|b| &b.systems)
    )
    .to_lowercase();
    if probe.contains("bitlocker") {
        return true;
    }
    let id = p.partition.as_ref().and_then(|x| x.id.clone()).unwrap_or_default().to_lowercase();
    p.block.as_ref().is_some_and(|b| b.encrypted) && p.filesystem.is_none() && ["windows", "microsoft", "basic_data", "ntfs"].iter().any(|w| id.contains(w))
}

/// Nośnik, z którego działa Setup: nie wolno na nim instalować.
pub fn is_live_medium(d: &ADevice) -> bool {
    std::iter::once(d).chain(d.partitions.iter()).any(|p| p.filesystem.as_ref().and_then(|f| f.label.as_deref()) == Some(LIVE_LABEL))
}

/// Dyski Agamy → dyski Setup: najpierw z Windows, potem od największego, bez nośnika.
pub fn disks_from_system(sys: &ASystem) -> Vec<Disk> {
    let devs = &sys.storage.devices;
    let cand: Vec<u64> = sys.storage.candidate_drives.clone().unwrap_or_else(|| devs.iter().map(|d| d.sid).collect());
    let mut disks: Vec<Disk> = devs
        .iter()
        .filter(|d| cand.contains(&d.sid) && d.block.is_some() && !is_live_medium(d))
        .map(|d| {
            let size_gb = (d.block.as_ref().unwrap().size / GB).floor() as u64;
            let named = [d.drive.as_ref().and_then(|x| x.vendor.clone()), d.drive.as_ref().and_then(|x| x.model.clone())]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            let name = if !named.is_empty() {
                named
            } else {
                d.description.clone().filter(|x| !x.to_lowercase().contains("multipath")).unwrap_or_else(|| d.name.clone())
            };
            let size = |p: &ADevice| p.block.as_ref().map(|b| b.size).unwrap_or(0.0);
            fn biggest<'a>(v: Vec<&'a ADevice>, size: impl Fn(&ADevice) -> f64) -> Option<&'a ADevice> {
                v.into_iter().max_by(|a, b| size(a).total_cmp(&size(b)))
            }
            let locked = biggest(d.partitions.iter().filter(|p| is_bitlocker(p)).collect(), size);
            let ntfs = biggest(
                d.partitions
                    .iter()
                    .filter(|p| p.filesystem.as_ref().is_some_and(|f| f.kind == "ntfs") && p.block.as_ref().is_some_and(|b| b.systems.iter().any(|s| s.to_lowercase().contains("windows"))))
                    .collect(),
                size,
            );
            let win = ntfs.or(locked);
            let windows = win.and_then(|w| {
                let b = w.block.as_ref()?;
                let free: f64 = d.partition_table.as_ref().map(|t| t.unused_slots.iter().map(|u| u.size).sum()).unwrap_or(0.0);
                let used = match &b.shrinking {
                    Some(AShrink { supported: true, min_size: Some(m) }) if *m > 0.0 => *m,
                    _ => b.size,
                };
                Some(WinPart {
                    partition: w.name.clone(),
                    min_gb: (used / GB).ceil() as u64 + WINDOWS_SPARE_GB,
                    pool_gb: ((b.size + free) / GB).floor() as u64,
                    bitlocker: ntfs.is_none() || locked.is_some_and(|l| l.name == w.name),
                })
            });
            Disk { device: d.name.clone(), name, size_gb, windows }
        })
        .collect();
    disks.sort_by(|a, b| b.windows.is_some().cmp(&a.windows.is_some()).then(b.size_gb.cmp(&a.size_gb)));
    disks
}

// ---------- plan i konfiguracja ----------

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    Beside,
    Replace,
    Whole,
}

/// Wszystko, co zebrały kroki kreatora.
#[derive(Clone, Debug)]
pub struct InstallPlan {
    pub locale: String,
    pub timezone: String,
    pub keymap: String,
    pub disk: Disk,
    pub place: Place,
    /// Rozmiar Windows po zmniejszeniu (tylko „beside”).
    pub windows_gb: Option<u64>,
    /// Hasło szyfrowania dysku (LUKS2).
    pub disk_password: Option<String>,
    pub full_name: String,
    pub user_name: String,
    pub password: String,
    pub choices: Choices,
}

/// Wybory z dawnej pierwszej konfiguracji, teraz w tym samym kreatorze. Stosuje je skrypt po instalacji
/// (post-install.sh › shrimp-setup --apply) w systemie docelowym, więc po restarcie nie ma już żadnych pytań.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Choices {
    /// Zgoda na kodeki i sterowniki własnościowe (krok „Licencje”).
    pub proprietary: bool,
    /// Pliki z Windows: "folders" (do folderów użytkownika) albo "folder" (jeden folder `files_folder`).
    pub files: String,
    pub files_folder: String,
    /// Pendrive z Shrimp Install: "restore" (wraca po pierwszym uruchomieniu) albo "home" (folder Pendrive).
    pub usb: String,
    /// Przeglądarka (Flathub) i programy z Windows do doinstalowania.
    pub browser: String,
    pub apps: Vec<String>,
    /// Wygląd: "default" | "prefer-dark", kolor dodatkowy GNOME albo pusty, rozmiar tekstu, światło nocne.
    pub color_scheme: String,
    pub accent: String,
    pub text_scale: f64,
    pub night_light: bool,
    pub auto_updates: bool,
}

pub const PRODUCT: (&str, &str) = ("Shrimp", "immutable");
/// Akcja Agamy: ponowne czytanie repozytoriów (bez sondowania dysków). `{"probe":null}` dawało HTTP 422.
pub fn probe_software() -> Value {
    json!({ "probe": { "only": ["software"] } })
}

/// Plan → konfiguracja Agamy (PUT /api/config).
pub fn config_from_plan(plan: &InstallPlan) -> Value {
    let mut generate = json!({ "generate": { "partitions": "default" } });
    if let Some(pw) = &plan.disk_password {
        generate["generate"]["encryption"] = json!({ "luks2": { "password": pw } });
    }
    let partitions = match (&plan.place, &plan.disk.windows) {
        (Place::Beside, Some(w)) => json!([{ "search": w.partition, "size": format!("{} GB", plan.windows_gb.unwrap_or(w.min_gb)) }, generate]),
        _ => json!([{ "search": "*", "delete": true }, generate]),
    };
    json!({
        "product": { "id": PRODUCT.0, "mode": PRODUCT.1 },
        "l10n": { "locale": plan.locale, "keymap": plan.keymap, "timezone": plan.timezone },
        "user": { "fullName": plan.full_name, "userName": plan.user_name, "password": plan.password, "hashedPassword": false },
        "storage": { "drives": [{ "search": plan.disk.device, "partitions": partitions }] },
    })
}

/// Plik dla skryptu po instalacji (/var/lib/shrimp-setup/install.json): konto i wszystkie wybory.
pub fn handoff(plan: &InstallPlan) -> Value {
    let mut v = serde_json::to_value(&plan.choices).unwrap_or_else(|_| json!({}));
    v["fullName"] = json!(plan.full_name);
    v["userName"] = json!(plan.user_name);
    v["beside"] = json!(plan.place == Place::Beside);
    v
}

/// Pełna konfiguracja instalacji: plan + install.json + skrypt po instalacji. Ten sam wynik dla testu
/// instalacji w QEMU leży w tests/agama-test-config.json (sprawdza go test `test_config_is_current`).
pub fn install_config(plan: &InstallPlan) -> Value {
    let mut c = config_from_plan(plan);
    c["files"] = json!([{ "destination": "/var/lib/shrimp-setup/install.json", "permissions": "0644", "content": handoff(plan).to_string() }]);
    c["scripts"] = json!({ "post": [{ "name": "shrimp-post-install", "chroot": false, "url": "file:///usr/share/shrimp-setup/post-install.sh" }] });
    c
}

/// Problemy, które blokują instalację. Repozytorium aktualizacji (…/update/…) bez metadanych nie blokuje.
pub fn blocking_issues(issues: Vec<AIssue>) -> Vec<AIssue> {
    issues
        .into_iter()
        .filter(|i| !(i.scope == "software" && i.class == "loadSource" && i.details.as_deref().unwrap_or(&i.description).contains("/update/")))
        .collect()
}

/// Odcisk postępu: zmienia się przy każdym kroku, pakiecie albo nowym zadaniu Agamy.
pub fn progress_key(st: &AStatus) -> String {
    let p: Vec<String> = st.progresses.iter().map(|p| format!("{}:{}:{}:{}", p.scope, p.index, p.size, p.step)).collect();
    let t: Vec<String> = st.tasks.iter().map(|t| t.id.to_string()).collect();
    format!("{}|{}|{}", st.stage, p.join(","), t.join(","))
}

/// Procent i faza instalacji (klucz tłumaczenia) z postępu Agamy.
pub fn phase_of(st: &AStatus) -> (f64, &'static str) {
    let p = st.progresses.iter().find(|x| x.scope == "manager").or(st.progresses.first());
    let sub = st.progresses.iter().find(|x| x.scope != "manager");
    let scope = sub.or(p).map(|x| x.scope.as_str()).unwrap_or("storage");
    let phase = match scope {
        "storage" => "inst.disk",
        "software" => "inst.system",
        _ => "inst.files",
    };
    let mut percent = 0.0;
    if let Some(p) = p.filter(|p| p.size > 0) {
        let inner = match sub {
            Some(s) if s.scope != p.scope && s.size > 0 => s.index as f64 / s.size as f64,
            _ => 0.0,
        };
        percent = (((p.index as f64 - 1.0 + inner) / p.size as f64) * 100.0).min(99.0);
    }
    (percent.max(0.0), phase)
}

// ---------- klient ----------

#[derive(Debug)]
pub enum InstallError {
    /// Za mało miejsca (problem dysku z Agamy).
    Space(String),
    Install(String),
    Stall,
    Agama(String),
}

pub fn read_token() -> Option<String> {
    TOKEN_FILES.iter().find_map(|f| std::fs::read_to_string(f).ok()).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

#[derive(Clone)]
pub struct Client {
    token: String,
}

impl Client {
    /// Czeka na token Agamy (sesja live startuje szybciej niż Agama).
    pub fn connect(timeout: Duration) -> Option<Client> {
        let t0 = Instant::now();
        loop {
            if let Some(token) = read_token() {
                let c = Client { token };
                if c.get("/api/status").is_ok() {
                    return Some(c);
                }
            }
            if t0.elapsed() > timeout {
                return None;
            }
            std::thread::sleep(Duration::from_millis(700));
        }
    }

    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let req = ureq::request(method, &format!("{AGAMA_URL}{path}")).set("Authorization", &format!("Bearer {}", self.token)).timeout(Duration::from_secs(60));
        let res = match body {
            Some(b) => req.send_json(b.clone()),
            None => req.call(),
        };
        match res {
            Ok(r) => {
                let t = r.into_string().map_err(|e| e.to_string())?;
                Ok(if t.trim().is_empty() { Value::Null } else { serde_json::from_str(&t).unwrap_or(Value::Null) })
            }
            Err(ureq::Error::Status(code, r)) => {
                let t: String = r.into_string().unwrap_or_default().chars().take(300).collect();
                Err(format!("{method} {path}: {code} {t}"))
            }
            Err(e) => Err(format!("{method} {path}: {e}")),
        }
    }

    pub fn get(&self, path: &str) -> Result<Value, String> {
        self.call("GET", path, None)
    }
    pub fn put(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.call("PUT", path, Some(body))
    }
    pub fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.call("POST", path, Some(body))
    }

    pub fn system(&self) -> Result<ASystem, String> {
        serde_json::from_value(self.get("/api/system")?).map_err(|e| e.to_string())
    }

    fn status(&self) -> Result<AStatus, String> {
        serde_json::from_value(self.get("/api/status")?).map_err(|e| e.to_string())
    }

    fn issues(&self) -> Result<Vec<AIssue>, String> {
        serde_json::from_value(self.get("/api/issues")?).map_err(|e| e.to_string())
    }

    /// Czeka, aż Agama przeliczy propozycję (puste „progresses” i „tasks”). Do 15 minut domyślnie:
    /// przy instalacji offline samo „Refreshing metadata” trwało w QEMU ponad 2 minuty.
    fn settle(&self, tries: u32) {
        for _ in 0..tries {
            match self.status() {
                Ok(st) if st.progresses.is_empty() && st.tasks.is_empty() => return,
                _ => std::thread::sleep(Duration::from_millis(500)),
            }
        }
    }

    /// Dyski, gdy Agama skończy je sondować: pierwszy odczyt bywa pusty albo niepełny (NVMe/RST), więc
    /// czytamy, aż lista się ustabilizuje (najwyżej ok. 40 s).
    pub fn disks(&self) -> Vec<Disk> {
        let mut disks = self.system().map(|s| disks_from_system(&s)).unwrap_or_default();
        for _ in 0..20 {
            self.settle(20);
            let next = self.system().map(|s| disks_from_system(&s)).unwrap_or_default();
            let same = next == disks;
            disks = next;
            if !disks.is_empty() && same {
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        eprintln!("shrimp-setup: dyski Agamy → Setup {disks:?}");
        disks
    }

    pub fn set_language(&self, locale: &str) {
        let _ = self.post("/api/action", &json!({ "configureL10n": { "locale": locale } }));
    }

    pub fn set_keymap(&self, keymap: &str) {
        let _ = self.post("/api/action", &json!({ "configureL10n": { "keymap": keymap } }));
    }

    /// Po podłączeniu do internetu: repozytoria od nowa, potem produkt jeszcze raz. "" albo opis problemu.
    pub fn reprobe(&self) -> String {
        if let Err(e) = self.post("/api/action", &probe_software()) {
            eprintln!("shrimp-setup: probe: {e}");
        }
        self.settle(1800);
        if let Err(e) = self.put("/api/config", &json!({ "product": { "id": PRODUCT.0, "mode": PRODUCT.1 } })) {
            return e;
        }
        self.settle(1800);
        let soft: Vec<AIssue> = blocking_issues(self.issues().unwrap_or_default()).into_iter().filter(|i| i.scope == "software").collect();
        eprintln!("shrimp-setup: po ponownym czytaniu repozytoriów problemy: {soft:?}");
        soft.iter().map(|i| i.description.as_str()).collect::<Vec<_>>().join("; ")
    }

    /// Sprawdza plan bez instalowania (np. czy starczy miejsca).
    pub fn check(&self, plan: &InstallPlan) -> Result<(), InstallError> {
        self.put("/api/config", &config_from_plan(plan)).map_err(InstallError::Install)?;
        self.settle(1800);
        let issues = blocking_issues(self.issues().unwrap_or_default());
        if !issues.is_empty() {
            eprintln!("shrimp-setup: problemy Agamy {issues:?}");
        }
        let join = |v: Vec<&AIssue>| v.iter().map(|i| i.description.as_str()).collect::<Vec<_>>().join("; ");
        let storage: Vec<&AIssue> = issues.iter().filter(|i| i.scope == "storage").collect();
        if !storage.is_empty() {
            return Err(InstallError::Space(join(storage)));
        }
        if !issues.is_empty() {
            return Err(InstallError::Install(join(issues.iter().collect())));
        }
        Ok(())
    }

    /// Zapisuje pełną konfigurację i startuje instalację (Agama zajęta: czeka i ponawia).
    pub fn start_install(&self, plan: &InstallPlan) -> Result<(), InstallError> {
        self.put("/api/config", &install_config(plan)).map_err(InstallError::Install)?;
        self.settle(1800);
        for i in 0.. {
            match self.post("/api/action", &json!({ "install": null })) {
                Ok(_) => return Ok(()),
                Err(e) if i < 20 && e.to_lowercase().contains("busy") => {
                    eprintln!("shrimp-setup: Agama zajęta, ponawiam start instalacji: {e}");
                    std::thread::sleep(Duration::from_secs(3));
                    self.settle(1800);
                }
                Err(e) => return Err(InstallError::Install(e)),
            }
        }
        unreachable!()
    }

    /// Śledzi /api/status co sekundę i podaje (procent, faza, minuty do końca). Pojedynczy brak odpowiedzi
    /// nie przerywa; dopiero POLL_MISSES z rzędu. Postęp stojący STALL_MIN minut to [`InstallError::Stall`].
    pub fn watch(&self, mut on: impl FnMut(f64, &'static str, Option<u64>)) -> Result<(), InstallError> {
        let t0 = Instant::now();
        let (mut misses, mut last, mut changed) = (0, String::new(), Instant::now());
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let st = match self.status() {
                Ok(s) => {
                    misses = 0;
                    s
                }
                Err(e) => {
                    misses += 1;
                    if misses >= POLL_MISSES {
                        return Err(InstallError::Agama(e));
                    }
                    continue;
                }
            };
            match st.stage.as_str() {
                "failed" => return Err(InstallError::Install(String::new())),
                "finished" => {
                    on(100.0, "done", None);
                    return Ok(());
                }
                _ => {}
            }
            let key = progress_key(&st);
            if key != last {
                last = key;
                changed = Instant::now();
            } else if changed.elapsed() > Duration::from_secs(STALL_MIN * 60) {
                return Err(InstallError::Stall);
            }
            let (percent, phase) = phase_of(&st);
            let el = t0.elapsed().as_secs_f64() / 60.0;
            let eta = if percent > 3.0 { Some(((el / percent) * (100.0 - percent)).ceil().max(1.0) as u64) } else { None };
            on(percent, phase, eta);
        }
    }

    pub fn reboot(&self) -> Result<(), String> {
        self.post("/api/action", &json!({ "finish": "reboot" })).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(v: Value) -> ASystem {
        serde_json::from_value(v).unwrap()
    }

    pub fn test_plan() -> InstallPlan {
        InstallPlan {
            locale: "pl_PL.UTF-8".into(),
            timezone: "Europe/Warsaw".into(),
            keymap: "pl".into(),
            disk: Disk { device: "/dev/vda".into(), name: "QEMU".into(), size_gb: 40, windows: None },
            place: Place::Whole,
            windows_gb: None,
            disk_password: None,
            full_name: "Jan Testowy".into(),
            user_name: "jan".into(),
            password: "shrimp".into(),
            choices: Choices {
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
            },
        }
    }

    #[test]
    fn beside_windows_shrinks_partition() {
        let mut p = test_plan();
        p.place = Place::Beside;
        p.windows_gb = Some(200);
        p.disk = Disk { device: "/dev/nvme0n1".into(), name: "SSD".into(), size_gb: 476, windows: Some(WinPart { partition: "/dev/nvme0n1p3".into(), min_gb: 80, pool_gb: 470, bitlocker: false }) };
        let c = config_from_plan(&p);
        assert_eq!(c["product"], json!({ "id": "Shrimp", "mode": "immutable" }));
        assert_eq!(c["storage"]["drives"][0]["partitions"], json!([{ "search": "/dev/nvme0n1p3", "size": "200 GB" }, { "generate": { "partitions": "default" } }]));
        p.place = Place::Replace;
        p.disk_password = Some("tajne".into());
        let c = config_from_plan(&p);
        assert_eq!(c["storage"]["drives"][0]["partitions"][0], json!({ "search": "*", "delete": true }));
        assert_eq!(c["storage"]["drives"][0]["partitions"][1]["generate"]["encryption"], json!({ "luks2": { "password": "tajne" } }));
    }

    #[test]
    fn windows_disk_and_min_from_shrinking() {
        let d = disks_from_system(&dev(json!({ "storage": { "candidateDrives": [1], "devices": [{
            "sid": 1, "name": "/dev/sda", "block": { "size": 500e9 }, "drive": { "vendor": "WD", "model": "Blue" },
            "partitionTable": { "unusedSlots": [{ "start": 0, "size": 10e9 }] },
            "partitions": [
                { "sid": 2, "name": "/dev/sda1", "block": { "size": 1e8 }, "filesystem": { "type": "vfat" } },
                { "sid": 3, "name": "/dev/sda3", "block": { "size": 480e9, "systems": ["Windows 11"], "shrinking": { "supported": true, "minSize": 120.4e9 } }, "filesystem": { "type": "ntfs" } }
            ] }] } })));
        assert_eq!(d, vec![Disk { device: "/dev/sda".into(), name: "WD Blue".into(), size_gb: 500, windows: Some(WinPart { partition: "/dev/sda3".into(), min_gb: 141, pool_gb: 490, bitlocker: false }) }]);
    }

    #[test]
    fn bitlocker_variants() {
        let d = disks_from_system(&dev(json!({ "storage": { "devices": [{ "sid": 1, "name": "/dev/nvme0n1", "block": { "size": 256e9 },
            "partitions": [{ "sid": 3, "name": "/dev/nvme0n1p3", "block": { "size": 250e9 }, "filesystem": { "type": "bitlocker" } }] }] } })));
        assert_eq!(d[0].windows, Some(WinPart { partition: "/dev/nvme0n1p3".into(), min_gb: 270, pool_gb: 250, bitlocker: true }));
        // Vivobook: pendrive z Shrimp pierwszy na liście, BitLocker jako szyfrowanie z typem danych Windows.
        let d = disks_from_system(&dev(json!({ "storage": { "candidateDrives": [10, 1], "devices": [
            { "sid": 10, "name": "/dev/sda", "block": { "size": 32e9 }, "drive": { "vendor": "SanDisk", "model": "Ultra" },
              "partitions": [{ "sid": 11, "name": "/dev/sda1", "block": { "size": 32e9 }, "filesystem": { "type": "vfat", "label": "SHRIMP_LIVE" } }] },
            { "sid": 1, "name": "/dev/nvme0n1", "block": { "size": 512e9 }, "drive": { "model": "INTEL SSDPEKNW512G8" }, "partitionTable": { "unusedSlots": [] },
              "partitions": [
                { "sid": 2, "name": "/dev/nvme0n1p1", "block": { "size": 1e8 }, "filesystem": { "type": "vfat" }, "partition": { "efi": true, "id": "esp" } },
                { "sid": 3, "name": "/dev/nvme0n1p3", "description": "Encrypted Partition", "block": { "size": 510e9, "encrypted": true, "shrinking": { "supported": false } }, "partition": { "efi": false, "id": "windows_basic_data" } }
              ] }
        ] } })));
        assert_eq!(d, vec![Disk { device: "/dev/nvme0n1".into(), name: "INTEL SSDPEKNW512G8".into(), size_gb: 512, windows: Some(WinPart { partition: "/dev/nvme0n1p3".into(), min_gb: 530, pool_gb: 510, bitlocker: true }) }]);
        let d = disks_from_system(&dev(json!({ "storage": { "devices": [
            { "sid": 1, "name": "/dev/sda", "block": { "size": 128e9 }, "partitions": [{ "sid": 2, "name": "/dev/sda2", "block": { "size": 120e9, "encrypted": true }, "partition": { "efi": false, "id": "linux" } }] },
            { "sid": 5, "name": "/dev/nvme0n1", "block": { "size": 1000e9 } }
        ] } })));
        assert_eq!(d.iter().map(|x| (x.device.as_str(), x.windows.is_some())).collect::<Vec<_>>(), [("/dev/nvme0n1", false), ("/dev/sda", false)]);
    }

    #[test]
    fn progress() {
        let st: AStatus = serde_json::from_value(json!({ "stage": "installing", "progresses": [
            { "scope": "manager", "index": 2, "size": 4, "step": "" }, { "scope": "software", "index": 5, "size": 10, "step": "" }] })).unwrap();
        assert_eq!(phase_of(&st), (37.5, "inst.system"));
        let mut st2 = st.clone();
        assert_eq!(progress_key(&st), progress_key(&st2));
        st2.progresses[1].index = 6;
        assert_ne!(progress_key(&st), progress_key(&st2));
        assert_eq!(probe_software(), json!({ "probe": { "only": ["software"] } }));
    }

    #[test]
    fn update_repo_does_not_block() {
        let upd = AIssue { scope: "software".into(), class: "loadSource".into(), description: "x".into(), details: Some("https://download.opensuse.org/update/leap/16.1/oss".into()) };
        let dist = AIssue { scope: "software".into(), class: "loadSource".into(), description: "https://download.opensuse.org/distribution/leap/16.1/repo/oss".into(), details: None };
        let disk = AIssue { scope: "storage".into(), class: "x".into(), description: "no space".into(), details: None };
        assert!(blocking_issues(vec![upd.clone()]).is_empty());
        assert_eq!(blocking_issues(vec![upd, dist.clone(), disk.clone()]), vec![dist, disk]);
    }

    /// Konfiguracja testu instalacji w QEMU (test-install.yml) to wynik tej samej funkcji, której używa okno.
    /// Po zmianie formatu: SHRIMP_WRITE_TEST_CONFIG=1 cargo test test_config_is_current.
    #[test]
    fn test_config_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/agama-test-config.json");
        let want = serde_json::to_string_pretty(&install_config(&test_plan())).unwrap() + "\n";
        if std::env::var_os("SHRIMP_WRITE_TEST_CONFIG").is_some() {
            std::fs::write(&path, &want).unwrap();
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap_or_default(), want, "uruchom: SHRIMP_WRITE_TEST_CONFIG=1 cargo test test_config_is_current");
    }
}
