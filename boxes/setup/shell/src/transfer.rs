//! Dane z Shrimp Install (shrimp-install/README.md › shrimp.json) na nośniku:
//! `shrimp-transfer/shrimp.json` i `shrimp-transfer/files/<klucz>/…`.
//!
//! - Kreator Setup: podpowiedzi (imię, język, strefa, klawiatura, BitLocker, pliki, programy) z [`setup_transfer`].
//! - `--restore` (post-install.sh, root): pliki trafiają do katalogu domowego nowego konta według wyboru
//!   z kroku „Pliki z Windows” ([`Placement`]): do folderów użytkownika albo do jednego folderu.
//!   Bez wyboru czekają w poczekalni [`STAGE`]. shrimp.json trafia do /var/lib/shrimp-setup.
use serde::Deserialize;
use serde_json::Value;
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

/// Podpowiedzi dla Setup: konto, język, strefa, klawiatura, BitLocker, rozmiar i foldery plików, programy z Windows.
/// Nieznana wersja formatu: tylko rozmiar plików (imię, język mogłyby znaczyć co innego).
pub fn setup_transfer(v: &Value) -> crate::logic::Transfer {
    let files: Vec<&Value> = v["files"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let bytes: u64 = files.iter().filter_map(|f| f["bytes"].as_u64()).sum();
    let mut folders: Vec<(String, u64, u64)> = Vec::new();
    for f in &files {
        let key = f["key"].as_str().unwrap_or("other").to_string();
        let b = f["bytes"].as_u64().unwrap_or(0);
        match folders.iter_mut().find(|x| x.0 == key) {
            Some(x) => x.2 += b,
            None => folders.push((key, f["files"].as_u64().unwrap_or(0), b)),
        }
    }
    let mut t = crate::logic::Transfer { bytes, folders, ..Default::default() };
    if !known_format(v) {
        eprintln!("shrimp-setup: shrimp.json w wersji {} (znana: {FORMAT_VERSION}), bez podpowiedzi", v["version"]);
        return t;
    }
    let s = |x: &Value| x.as_str().filter(|s| !s.is_empty()).map(str::to_string);
    t.user.name = s(&v["user"]["name"]).unwrap_or_default();
    t.user.full_name = s(&v["user"]["fullName"]).unwrap_or_default();
    t.locale.language = s(&v["locale"]["language"]);
    t.locale.format = s(&v["locale"]["format"]);
    t.locale.timezone = s(&v["locale"]["timezone"]);
    t.locale.keyboards = v["locale"]["keyboards"].as_array().map(|a| a.iter().filter_map(s).collect()).unwrap_or_default();
    t.bitlocker = v["windows"]["bitlocker"].as_bool().unwrap_or(false);
    t.apps = v["apps"].as_array().map(|a| a.iter().filter_map(|x| s(&x["name"])).collect()).unwrap_or_default();
    t.store_apps = v["storeApps"].as_array().map(|a| a.iter().filter_map(s).collect()).unwrap_or_default();
    t.default_browser = s(&v["defaultBrowser"]);
    t
}

/// Kopia zawartości pendrive'a zrobiona przez Shrimp Install (shrimp-backup/manifest.tsv): (pliki, bajty).
pub fn usb_backup() -> Option<(u64, u64)> {
    let roots = [PathBuf::from("/run/overlay/live"), PathBuf::from("/run/initramfs/live")];
    let extra = find_dir().and_then(|d| d.parent().map(Path::to_path_buf));
    let manifest = extra.iter().chain(roots.iter()).map(|r| r.join("shrimp-backup/manifest.tsv")).find(|p| p.is_file())?;
    Some(parse_manifest(&fs::read_to_string(manifest).ok()?))
}

/// Wiersze `f⇥sha⇥bajty⇥…` opisu kopii (format: shrimp-install/rufus/src/shrimp/usbbackup.c).
pub fn parse_manifest(s: &str) -> (u64, u64) {
    s.lines().filter_map(|l| l.strip_prefix("f\t")).fold((0, 0), |(n, b), l| (n + 1, b + l.split('\t').nth(1).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0)))
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

/// Poczekalnia plików z Windows w katalogu domowym (względem niego), do kroku „Pliki z Windows”.
pub const STAGE: &str = ".local/share/shrimp-setup/windows";

/// Tworzy kolejne katalogi `rel` pod `base` i oddaje je właścicielowi konta.
fn mkdirs_owned(base: &Path, rel: &str, a: &Account) -> io::Result<PathBuf> {
    let mut p = base.to_path_buf();
    for c in rel.split('/') {
        p.push(c);
        if !p.is_dir() {
            fs::create_dir(&p)?;
            chown(&p, a);
        }
    }
    Ok(p)
}

/// Ile plików i bajtów czeka w poczekalni (`home`/[`STAGE`]). Brak poczekalni = (0, 0).
#[cfg(test)]
fn staged(home: &Path) -> (u64, u64) {
    fn walk(p: &Path, n: &mut (u64, u64)) {
        let Ok(rd) = fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                walk(&e.path(), n);
            } else {
                n.0 += 1;
                n.1 += md.len();
            }
        }
    }
    let mut n = (0, 0);
    walk(&home.join(STAGE), &mut n);
    n
}

/// Nazwa folderu na pliki z Windows bez ukośników i kropki na początku („Windows”, gdy nic nie zostanie).
pub fn safe_folder(f: &str) -> String {
    let f = f.replace(['/', '\\'], "").trim().to_string();
    if f.is_empty() || f.starts_with('.') { "Windows".into() } else { f }
}

/// Przenosi pliki z poczekalni: `into_folders` = do folderów użytkownika (Dokumenty, Obrazy… i katalog
/// domowy, scalając; nazwy zajęte dostają „(Windows)”), inaczej całość do folderu `folder` w katalogu domowym.
/// Ten sam system plików, więc to tylko zmiana nazw. Poczekalnia znika.
pub fn place(home: &Path, into_folders: bool, folder: &str, owner: Option<(u32, u32)>) -> Result<Stats, String> {
    let merge_owner = owner;
    fn merge(src: &Path, dst: &Path, st: &mut Stats, owner: Option<(u32, u32)>) {
        let Ok(rd) = fs::read_dir(src) else { return };
        if !dst.is_dir() {
            if let Err(e) = fs::create_dir_all(dst) {
                st.failed.push(format!("{}: {e}", dst.display()));
                return;
            }
            if let Some((u, g)) = owner {
                let _ = std::os::unix::fs::chown(dst, Some(u), Some(g));
            }
        }
        for e in rd.flatten() {
            let (from, to) = (e.path(), dst.join(e.file_name()));
            if from.is_dir() && to.is_dir() {
                merge(&from, &to, st, owner);
                continue;
            }
            let to = free_name(&to);
            match fs::rename(&from, &to) {
                Ok(()) => st.files += 1,
                Err(e) => st.failed.push(format!("{}: {e}", from.display())),
            }
        }
    }
    let stage = home.join(STAGE);
    if !stage.is_dir() {
        return Ok(Stats::default());
    }
    let mut st = Stats::default();
    let dst = if into_folders { home.to_path_buf() } else { free_name(&home.join(folder)) };
    merge(&stage, &dst, &mut st, merge_owner);
    if st.failed.is_empty() {
        fs::remove_dir_all(&stage).map_err(|e| e.to_string())?;
        Ok(st)
    } else {
        Err(st.failed.join("; "))
    }
}

/// `shrimp-setup --restore [--from DIR] [--root /mnt] --user NAZWA` (post-install.sh, jako root).
/// Gdzie trafiają pliki z Windows (krok „Pliki z Windows” w Setup): do folderów użytkownika albo do jednego folderu.
pub enum Placement {
    Folders,
    Folder(String),
}

pub fn restore(from: Option<PathBuf>, root: &Path, user: &str, placement: Option<Placement>) -> Result<Stats, String> {
    let dir = from.or_else(find_dir).ok_or("brak shrimp-transfer na nośniku")?;
    let v = load(&dir).ok_or("nie można odczytać shrimp.json")?;
    let a = account(root, user).ok_or_else(|| format!("brak konta {user} w {}", root.display()))?;
    let real_home = root.join(a.home.strip_prefix("/").unwrap_or(&a.home));

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
    let dirs = fs::read_to_string(real_home.join(".config/user-dirs.dirs")).unwrap_or_default();
    // Pliki czekają na zgodę w poczekalni; układ w niej jest taki jak w katalogu domowym.
    let home = mkdirs_owned(&real_home, STAGE, &a).map_err(|e| e.to_string())?;

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
    // Wybór z Setup: od razu na miejsce (bez poczekalni i bez pytania po pierwszym uruchomieniu).
    if let Some(p) = placement {
        let (into, folder) = match p {
            Placement::Folders => (true, String::new()),
            Placement::Folder(f) => (false, safe_folder(&f)),
        };
        if let Err(e) = place(&real_home, into, &folder, Some((a.uid, a.gid))) {
            st.failed.push(e);
        }
    }
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        let st = restore(Some(tr.clone()), &root, "jan", None).unwrap();
        let h = root.join("home/jan");
        let w = h.join(STAGE);
        assert_eq!(fs::read_to_string(w.join("Dokumenty/Umowy/najem.pdf")).unwrap(), "pdf");
        assert_eq!(fs::read_to_string(w.join("Projekty/a.txt")).unwrap(), "a");
        assert_eq!(fs::read_to_string(w.join("wakacje.mkv")).unwrap(), "123");
        assert!(root.join("var/lib/shrimp-setup/shrimp.json").is_file());
        assert_eq!(st.files, 3);
        assert_eq!(staged(&h), (3, 7));
        assert!(restore(Some(tr), &root, "ola", None).is_err());
        // Zgoda: do folderów użytkownika, z konfliktem nazw.
        fs::create_dir_all(h.join("Dokumenty/Umowy")).unwrap();
        fs::write(h.join("Dokumenty/Umowy/najem.pdf"), "stary").unwrap();
        let st = place(&h, true, "Windows", None).unwrap();
        assert_eq!(st.files, 3);
        assert_eq!(fs::read_to_string(h.join("Dokumenty/Umowy/najem.pdf")).unwrap(), "stary");
        assert_eq!(fs::read_to_string(h.join("Dokumenty/Umowy/najem (Windows).pdf")).unwrap(), "pdf");
        assert_eq!(fs::read_to_string(h.join("wakacje.mkv")).unwrap(), "123");
        assert!(!w.exists());
        assert_eq!(staged(&h), (0, 0));
        fs::remove_dir_all(&t).unwrap();
    }

    #[test]
    fn place_into_one_folder() {
        let h = std::env::temp_dir().join(format!("shrimp-place-{}", std::process::id()));
        fs::create_dir_all(h.join(STAGE).join("Obrazy")).unwrap();
        fs::write(h.join(STAGE).join("Obrazy/a.jpg"), "a").unwrap();
        fs::create_dir_all(h.join("Windows")).unwrap();
        place(&h, false, "Windows", None).unwrap();
        assert_eq!(fs::read_to_string(h.join("Windows (Windows)/Obrazy/a.jpg")).unwrap(), "a");
        assert!(place(&h, true, "Windows", None).unwrap().files == 0);
        assert_eq!(safe_folder("../x/"), "Windows");
        assert_eq!(safe_folder("Pliki/Win"), "PlikiWin");
        assert_eq!(safe_folder(" /"), "Windows");
        fs::remove_dir_all(&h).unwrap();
    }

    /// Przykład z kontraktu Shrimp Install ↔ Setup (boxes/install-schema): to samo, co zapisuje program dla Windows.
    fn example() -> Value {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install-schema/shrimp.example.json");
        serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn transfer_from_contract_example() {
        let t = setup_transfer(&example());
        assert_eq!(t.user.name, "Jan.Kowalski");
        assert_eq!(t.locale.timezone.as_deref(), Some("Europe/Warsaw"));
        assert_eq!(t.locale.keyboards[0], "00000415");
        assert!(!t.bitlocker);
        assert_eq!(t.bytes, 3435973836u64 + 1024);
        assert_eq!(t.apps[1], "Steam");
        assert_eq!(t.store_apps, ["SpotifyAB.SpotifyMusic"]);
        assert_eq!(t.default_browser.as_deref(), Some("FirefoxURL-308046B0AF4A39CB"));
        assert_eq!(t.folders.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["documents", "other"]);
        let p = crate::logic::prefill(&t, &crate::data::langs(), crate::data::REGIONS, crate::data::KEYBOARDS);
        assert_eq!(crate::data::KEYBOARDS[p.kb.unwrap()].keymap, "pl");
        assert_eq!(p.user, "jankowalski");
        assert!(!crate::logic::map_windows_apps(&t.apps, &t.store_apps).is_empty());
    }

    #[test]
    fn unknown_format_version_gives_no_hints() {
        let mut v = example();
        v["version"] = json!(2);
        let t = setup_transfer(&v);
        assert!(t.user.name.is_empty() && t.locale.language.is_none() && t.apps.is_empty());
        assert_eq!(t.bytes, 3435973836u64 + 1024);
        assert!(known_format(&json!({})));
    }

    #[test]
    fn usb_manifest() {
        let m = "#shrimp-backup 1\nlabel\tUSB\nd\tdocs\nf\t".to_string() + &"a".repeat(64) + "\t10\t0\t0\tdocs/a\nf\t" + &"b".repeat(64) + "\t5\t0\t0\tb\n";
        assert_eq!(parse_manifest(&m), (2, 15));
    }
}
