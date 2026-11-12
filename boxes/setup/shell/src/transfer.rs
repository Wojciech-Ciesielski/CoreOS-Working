//! Dane z Shrimp Install (shrimp-install/README.md › shrimp.json) na nośniku:
//! `shrimp-transfer/shrimp.json` i `shrimp-transfer/files/<klucz>/…`.
//!
//! - `--install`: podpowiedzi dla kroków 1–9 (imię, język, strefa, klawiatura, BitLocker, rozmiar plików)
//!   trafiają do strony jako `window.SHRIMP_TRANSFER` ([`hint`]).
//! - `--restore` (post-install.sh, root): pliki wracają do folderów nowego konta ([`restore`]),
//!   a shrimp.json do /var/lib/shrimp-setup (lista programów dla kroku Programy).
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Gdzie żywy system ma nośnik (kiwi/dracut) i gdzie GNOME montuje go, jeśli to inny dysk.
pub fn find_dir() -> Option<PathBuf> {
    // KIWI montuje nośnik live w /run/overlay/live (sprawdzone w teście instalacji w QEMU);
    // /run/initramfs/live to ścieżka dracut dmsquash, zostaje na wszelki wypadek.
    let mut c = vec![PathBuf::from("/run/overlay/live/shrimp-transfer"), PathBuf::from("/run/initramfs/live/shrimp-transfer")];
    if let Ok(users) = fs::read_dir("/run/media") {
        for u in users.flatten() {
            if let Ok(vols) = fs::read_dir(u.path()) {
                c.extend(vols.flatten().map(|v| v.path().join("shrimp-transfer")));
            }
        }
    }
    c.into_iter().find(|d| d.join("shrimp.json").is_file())
}

pub fn load(dir: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(dir.join("shrimp.json")).ok()?).ok()
}

/// Wersja formatu shrimp.json, którą rozumie Setup (boxes/install-schema/shrimp.schema.json).
pub const FORMAT_VERSION: u64 = 1;

/// Brak pola `version` = pliki z pierwszych wersji Shrimp Install, czyli 1.
pub fn known_format(v: &Value) -> bool {
    v["version"].as_u64().unwrap_or(FORMAT_VERSION) == FORMAT_VERSION
}

/// Tylko to, czego potrzebują kroki instalacji. Lista programów i ścieżki zostają poza stroną.
/// Nieznana wersja formatu: bez podpowiedzi (imię, język mogłyby znaczyć co innego), tylko rozmiar plików.
pub fn hint(v: &Value) -> Value {
    let bytes: u64 = v["files"].as_array().map(|a| a.iter().filter_map(|f| f["bytes"].as_u64()).sum()).unwrap_or(0);
    if !known_format(v) {
        eprintln!("shrimp-setup: shrimp.json w wersji {} (znana: {FORMAT_VERSION}), bez podpowiedzi", v["version"]);
        return json!({ "bytes": bytes });
    }
    json!({
        "user": v["user"],
        "locale": v["locale"],
        "bitlocker": v["windows"]["bitlocker"].as_bool().unwrap_or(false),
        "bytes": bytes,
    })
}

/// Skrypt strony z podpowiedziami (JSON jest poprawnym wyrażeniem JS; `</` nie może zamknąć skryptu,
/// ale skrypt użytkownika i tak nie jest w HTML).
#[cfg(test)]
pub fn hint_script(v: &Value) -> String {
    page_hint(Some(v), None)
}

/// Podpowiedzi dla strony: shrimp.json (jeśli jest) i język menu GRUB z nośnika (`grubLang`).
/// Bez shrimp.json strona i tak dostaje język z Windows, gdy Shrimp Install zapisał shrimp-lang.cfg.
pub fn page_hint(v: Option<&Value>, grub: Option<String>) -> String {
    let mut h = v.map(hint).unwrap_or_else(|| json!({}));
    if let Some(g) = grub {
        h["grubLang"] = json!(g);
    }
    format!("window.SHRIMP_TRANSFER = {h};")
}

/// `set shrimp_lang=de` → `de` (boot/grub2/shrimp-lang.cfg, zapisuje go Shrimp Install z języka interfejsu Windows).
pub fn parse_grub_lang(s: &str) -> Option<String> {
    s.lines()
        .find_map(|l| l.trim().strip_prefix("set shrimp_lang="))
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty() && v.len() <= 16 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
}

/// Język menu GRUB z nośnika, z którego działa sesja live.
pub fn grub_lang() -> Option<String> {
    let mut roots = vec![PathBuf::from("/run/overlay/live"), PathBuf::from("/run/initramfs/live")];
    if let Some(d) = find_dir().and_then(|d| d.parent().map(Path::to_path_buf)) {
        roots.insert(0, d);
    }
    roots.iter().find_map(|r| fs::read_to_string(r.join("boot/grub2/shrimp-lang.cfg")).ok()).and_then(|s| parse_grub_lang(&s))
}

// ---------- przywracanie plików ----------

#[derive(Deserialize)]
struct Entry {
    key: String,
    path: String,
}

/// Klucz folderu z Windows → zmienna XDG i nazwa angielska, gdy xdg-user-dirs nie zadziała.
fn xdg_var(key: &str) -> Option<(&'static str, &'static str)> {
    Some(match key {
        "documents" => ("XDG_DOCUMENTS_DIR", "Documents"),
        "pictures" => ("XDG_PICTURES_DIR", "Pictures"),
        "desktop" => ("XDG_DESKTOP_DIR", "Desktop"),
        "music" => ("XDG_MUSIC_DIR", "Music"),
        "videos" => ("XDG_VIDEOS_DIR", "Videos"),
        "downloads" => ("XDG_DOWNLOAD_DIR", "Downloads"),
        _ => return None,
    })
}

/// `XDG_DOCUMENTS_DIR="$HOME/Dokumenty"` → `Dokumenty` (względem katalogu domowego).
pub fn parse_user_dirs(s: &str, var: &str) -> Option<String> {
    s.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .find_map(|l| l.strip_prefix(var)?.strip_prefix('='))
        .map(|v| v.trim().trim_matches('"').to_string())
        .and_then(|v| v.strip_prefix("$HOME/").map(str::to_string))
        .filter(|v| !v.is_empty() && !v.contains(".."))
}

struct Account {
    home: PathBuf,
    uid: u32,
    gid: u32,
}

fn account(root: &Path, user: &str) -> Option<Account> {
    let pw = fs::read_to_string(root.join("etc/passwd")).ok()?;
    let f: Vec<&str> = pw.lines().find(|l| l.starts_with(&format!("{user}:")))?.split(':').collect();
    Some(Account { uid: f.get(2)?.parse().ok()?, gid: f.get(3)?.parse().ok()?, home: PathBuf::from(f.get(5)?) })
}

/// Język nowego systemu (Agama zapisuje go w /etc/locale.conf).
fn target_lang(root: &Path) -> String {
    fs::read_to_string(root.join("etc/locale.conf"))
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("LANG=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_else(|| "en_US.UTF-8".into())
}

/// Plik docelowy, który jeszcze nie istnieje: „x.pdf”, „x (Windows).pdf”, „x (Windows 2).pdf”…
pub fn free_name(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_path_buf();
    }
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (1..)
        .map(|n| p.with_file_name(if n == 1 { format!("{stem} (Windows){ext}") } else { format!("{stem} (Windows {n}){ext}") }))
        .find(|c| !c.exists())
        .unwrap()
}

const PART: &str = ".shrimp-part";

/// „film.mkv.shrimp-part000” → („film.mkv”, 0).
pub fn split_part(name: &str) -> Option<(&str, u32)> {
    let i = name.rfind(PART)?;
    let n = &name[i + PART.len()..];
    (n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit())).then(|| (&name[..i], n.parse().unwrap()))
}

#[derive(Default)]
pub struct Stats {
    pub files: u64,
    pub bytes: u64,
    pub failed: Vec<String>,
}

fn chown(p: &Path, a: &Account) {
    let _ = std::os::unix::fs::chown(p, Some(a.uid), Some(a.gid));
}

fn copy_into(f: &mut fs::File, src: &Path, buf: &mut [u8]) -> io::Result<u64> {
    let mut r = fs::File::open(src)?;
    let mut n = 0;
    loop {
        let k = r.read(buf)?;
        if k == 0 {
            return Ok(n);
        }
        f.write_all(&buf[..k])?;
        n += k as u64;
    }
}

/// Kopiuje katalog `src` do `dst` (scalając z tym, co już jest) i skleja pliki podzielone przez Shrimp Install.
#[cfg(test)]
pub fn copy_tree(src: &Path, dst: &Path, owner: Option<(u32, u32)>, st: &mut Stats) {
    let acc = owner.map(|(uid, gid)| Account { home: PathBuf::new(), uid, gid });
    let mut buf = vec![0u8; 1 << 20];
    copy_dir(src, dst, acc.as_ref(), st, &mut buf);
}

fn copy_dir(src: &Path, dst: &Path, a: Option<&Account>, st: &mut Stats, buf: &mut [u8]) {
    if !dst.is_dir() {
        if let Err(e) = fs::create_dir_all(dst) {
            st.failed.push(format!("{}: {e}", dst.display()));
            return;
        }
        if let Some(a) = a {
            chown(dst, a);
        }
    }
    let Ok(rd) = fs::read_dir(src) else {
        st.failed.push(format!("{}: nie można odczytać", src.display()));
        return;
    };
    let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for name in names {
        if !copy_entry(src, &name, dst, a, st, buf) {
            return;
        }
    }
}

/// Jeden element katalogu `src` (folder albo plik, także część 000 pliku podzielonego) do katalogu `dst`.
/// Zwraca false, gdy zabrakło miejsca i dalsze kopiowanie nie ma sensu.
fn copy_entry(src: &Path, name: &str, dst: &Path, a: Option<&Account>, st: &mut Stats, buf: &mut [u8]) -> bool {
    let path = src.join(name);
    let Ok(md) = fs::symlink_metadata(&path) else { return true };
    if md.is_dir() {
        copy_dir(&path, &dst.join(name), a, st, buf);
        return true;
    }
    if !md.is_file() {
        return true;
    }
    let (target, parts) = match split_part(name) {
        Some((_, n)) if n > 0 => return true, // dalsze części dokleja część 000
        Some((base, _)) => {
            let mut parts = vec![path.clone()];
            for n in 1.. {
                let p = src.join(format!("{base}{PART}{n:03}"));
                if !p.is_file() {
                    break;
                }
                parts.push(p);
            }
            (base.to_string(), parts)
        }
        None => (name.to_string(), vec![path.clone()]),
    };
    if !dst.is_dir() {
        if let Err(e) = fs::create_dir_all(dst) {
            st.failed.push(format!("{}: {e}", dst.display()));
            return true;
        }
    }
    let out = free_name(&dst.join(&target));
    let res = (|| -> io::Result<u64> {
        let mut f = fs::File::create(&out)?;
        let mut n = 0;
        for p in &parts {
            n += copy_into(&mut f, p, buf)?;
        }
        if let Ok(m) = md.modified() {
            let _ = f.set_modified(m);
        }
        Ok(n)
    })();
    match res {
        Ok(n) => {
            st.files += 1;
            st.bytes += n;
            if let Some(a) = a {
                chown(&out, a);
            }
            true
        }
        Err(e) => {
            let _ = fs::remove_file(&out);
            st.failed.push(format!("{}: {e}", out.display()));
            e.raw_os_error() != Some(28) // ENOSPC
        }
    }
}

/// `shrimp-setup --restore [--from DIR] [--root /mnt] --user NAZWA` (post-install.sh, jako root).
pub fn restore(from: Option<PathBuf>, root: &Path, user: &str) -> Result<Stats, String> {
    let dir = from.or_else(find_dir).ok_or("brak shrimp-transfer na nośniku")?;
    let v = load(&dir).ok_or("nie można odczytać shrimp.json")?;
    let a = account(root, user).ok_or_else(|| format!("brak konta {user} w {}", root.display()))?;
    let home = root.join(a.home.strip_prefix("/").unwrap_or(&a.home));

    let state = root.join("var/lib/shrimp-setup");
    fs::create_dir_all(&state).map_err(|e| e.to_string())?;
    fs::copy(dir.join("shrimp.json"), state.join("shrimp.json")).map_err(|e| e.to_string())?;

    // Foldery w języku nowego systemu („Dokumenty”, „Obrazy”…): tworzy je xdg-user-dirs-update jako użytkownik.
    let lang = target_lang(root);
    let _ = Command::new("chroot")
        .arg(root)
        .args(["runuser", "-u", user, "--", "env"])
        .arg(format!("HOME={}", a.home.display()))
        .arg(format!("LANG={lang}"))
        .arg("xdg-user-dirs-update")
        .status();
    let dirs = fs::read_to_string(home.join(".config/user-dirs.dirs")).unwrap_or_default();

    let files: Vec<Entry> = serde_json::from_value(v["files"].clone()).unwrap_or_default();
    let mut st = Stats::default();
    let mut buf = vec![0u8; 1 << 20];
    for f in files {
        if f.path.split('/').any(|c| c == ".." || c.is_empty()) {
            continue;
        }
        let src = dir.join(&f.path);
        match xdg_var(&f.key) {
            Some((var, en)) => {
                let dst = home.join(parse_user_dirs(&dirs, var).unwrap_or_else(|| en.into()));
                if src.is_dir() {
                    copy_dir(&src, &dst, Some(&a), &mut st, &mut buf);
                }
            }
            // „Wybiorę sam”: files/other/<nazwa> (folder albo plik) trafia wprost do katalogu domowego.
            None => {
                let (Some(parent), Some(name)) = (src.parent(), src.file_name().map(|n| n.to_string_lossy().into_owned())) else { continue };
                let name = if src.exists() { name } else { format!("{name}{PART}000") };
                copy_entry(parent, &name, &home, Some(&a), &mut st, &mut buf);
            }
        }
    }
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_dirs() {
        let s = "# c\nXDG_DESKTOP_DIR=\"$HOME/Pulpit\"\nXDG_DOCUMENTS_DIR=\"$HOME/Dokumenty\"\nXDG_MUSIC_DIR=\"$HOME/\"\nXDG_VIDEOS_DIR=\"/srv/x\"\n";
        assert_eq!(parse_user_dirs(s, "XDG_DOCUMENTS_DIR").as_deref(), Some("Dokumenty"));
        assert_eq!(parse_user_dirs(s, "XDG_DESKTOP_DIR").as_deref(), Some("Pulpit"));
        assert_eq!(parse_user_dirs(s, "XDG_MUSIC_DIR"), None);
        assert_eq!(parse_user_dirs(s, "XDG_VIDEOS_DIR"), None);
        assert_eq!(parse_user_dirs(s, "XDG_PICTURES_DIR"), None);
    }

    #[test]
    fn parts() {
        assert_eq!(split_part("film.mkv.shrimp-part000"), Some(("film.mkv", 0)));
        assert_eq!(split_part("film.mkv.shrimp-part012"), Some(("film.mkv", 12)));
        assert_eq!(split_part("film.mkv"), None);
        assert_eq!(split_part("x.shrimp-part1"), None);
    }

    #[test]
    fn restores_tree_with_split_files_and_conflicts() {
        let t = std::env::temp_dir().join(format!("shrimp-restore-{}", std::process::id()));
        let (src, dst) = (t.join("src"), t.join("dst"));
        fs::create_dir_all(src.join("Rodzina")).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("a.txt"), "nowy").unwrap();
        fs::write(dst.join("a.txt"), "stary").unwrap();
        fs::write(src.join("Rodzina/film.mkv.shrimp-part000"), "abc").unwrap();
        fs::write(src.join("Rodzina/film.mkv.shrimp-part001"), "def").unwrap();
        fs::write(src.join("Rodzina/film.mkv.shrimp-part002"), "g").unwrap();
        let mut st = Stats::default();
        copy_tree(&src, &dst, None, &mut st);
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "stary");
        assert_eq!(fs::read_to_string(dst.join("a (Windows).txt")).unwrap(), "nowy");
        assert_eq!(fs::read_to_string(dst.join("Rodzina/film.mkv")).unwrap(), "abcdefg");
        assert!(!dst.join("Rodzina/film.mkv.shrimp-part001").exists());
        assert_eq!((st.files, st.bytes, st.failed.len()), (2, 11, 0));
        fs::remove_dir_all(&t).unwrap();
    }

    #[test]
    fn restore_puts_folders_into_xdg_dirs_and_other_into_home() {
        let t = std::env::temp_dir().join(format!("shrimp-restore-e2e-{}", std::process::id()));
        let (tr, root) = (t.join("shrimp-transfer"), t.join("mnt"));
        fs::create_dir_all(tr.join("files/documents/Umowy")).unwrap();
        fs::create_dir_all(tr.join("files/other/Projekty")).unwrap();
        fs::write(tr.join("files/documents/Umowy/najem.pdf"), "pdf").unwrap();
        fs::write(tr.join("files/other/Projekty/a.txt"), "a").unwrap();
        fs::write(tr.join("files/other/wakacje.mkv.shrimp-part000"), "12").unwrap();
        fs::write(tr.join("files/other/wakacje.mkv.shrimp-part001"), "3").unwrap();
        fs::write(tr.join("shrimp.json"), json!({ "version": 1, "files": [
            { "key": "documents", "path": "files/documents" },
            { "key": "other", "path": "files/other/Projekty" },
            { "key": "other", "path": "files/other/wakacje.mkv" },
            { "key": "other", "path": "files/other/../../x" },
        ] }).to_string()).unwrap();
        fs::create_dir_all(root.join("etc")).unwrap();
        fs::create_dir_all(root.join("home/jan/.config")).unwrap();
        fs::write(root.join("etc/passwd"), "root:x:0:0::/root:/bin/bash\njan:x:1000:1000:Jan:/home/jan:/bin/bash\n").unwrap();
        fs::write(root.join("home/jan/.config/user-dirs.dirs"), "XDG_DOCUMENTS_DIR=\"$HOME/Dokumenty\"\n").unwrap();
        let st = restore(Some(tr.clone()), &root, "jan").unwrap();
        let h = root.join("home/jan");
        assert_eq!(fs::read_to_string(h.join("Dokumenty/Umowy/najem.pdf")).unwrap(), "pdf");
        assert_eq!(fs::read_to_string(h.join("Projekty/a.txt")).unwrap(), "a");
        assert_eq!(fs::read_to_string(h.join("wakacje.mkv")).unwrap(), "123");
        assert!(root.join("var/lib/shrimp-setup/shrimp.json").is_file());
        assert_eq!(st.files, 3);
        assert!(restore(Some(tr), &root, "ola").is_err());
        fs::remove_dir_all(&t).unwrap();
    }

    /// Przykład z kontraktu Shrimp Install ↔ Setup (boxes/install-schema): to samo, co zapisuje program dla Windows.
    fn example() -> Value {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install-schema/shrimp.example.json");
        serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn hint_from_contract_example() {
        let v = example();
        let h = hint(&v);
        assert_eq!(h["user"]["name"], "Jan.Kowalski");
        assert_eq!(h["locale"]["timezone"], "Europe/Warsaw");
        assert_eq!(h["locale"]["keyboards"][0], "00000415");
        assert_eq!(h["bitlocker"], false);
        assert_eq!(h["bytes"], 3435973836u64 + 1024);
        assert!(hint_script(&v).starts_with("window.SHRIMP_TRANSFER = {"));
    }

    #[test]
    fn unknown_format_version_gives_no_hints() {
        let mut v = example();
        v["version"] = json!(2);
        let h = hint(&v);
        assert!(h.get("user").is_none() && h.get("locale").is_none());
        assert_eq!(h["bytes"], 3435973836u64 + 1024);
        assert!(known_format(&json!({})));
    }

    #[test]
    fn grub_lang_from_shrimp_install() {
        assert_eq!(parse_grub_lang("set shrimp_lang=de\n").as_deref(), Some("de"));
        assert_eq!(parse_grub_lang("set shrimp_lang=ca-valencia").as_deref(), Some("ca-valencia"));
        assert_eq!(parse_grub_lang("set shrimp_lang=\"x; rm\""), None);
        assert_eq!(parse_grub_lang(""), None);
        assert_eq!(page_hint(None, Some("en".into())), "window.SHRIMP_TRANSFER = {\"grubLang\":\"en\"};");
        let v = example();
        assert!(page_hint(Some(&v), Some("pl".into())).contains("\"grubLang\":\"pl\""));
    }

    #[test]
    fn hint_sums_bytes_and_hides_apps() {
        let v = json!({ "user": { "name": "jan" }, "windows": { "bitlocker": true }, "apps": [{ "name": "Steam" }],
            "files": [{ "key": "documents", "bytes": 5 }, { "key": "other", "bytes": 7 }] });
        let h = hint(&v);
        assert_eq!(h["bytes"], 12);
        assert_eq!(h["bitlocker"], true);
        assert!(h.get("apps").is_none());
    }
}
