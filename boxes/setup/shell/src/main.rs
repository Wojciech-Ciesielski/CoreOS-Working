//! shrimp-setup: Shrimp Setup, jeden kreator instalacji i pierwszej konfiguracji (GTK4 + libadwaita).
//!
//! `shrimp-setup [--window] [--demo]`   na nośniku, w sesji kiosku (gnome-kiosk, bez pulpitu): wszystkie kroki
//!                                      od języka po wygląd, potem instalacja nad Agamą i restart. Po restarcie
//!                                      system jest gotowy (bez drugiej części i bez strażnika).
//!                                      `--demo`: dane z makiety, bez Agamy; `--window`: okno zamiast pełnego ekranu.
//! `shrimp-setup --apply [--root /mnt]` post-install.sh: wybory z kreatora w systemie docelowym (apply.rs).
//! `shrimp-setup --restore --user NAZWA [--root /mnt] [--from KATALOG] [--place folders|folder] [--folder NAZWA]`
//!                                      post-install.sh: pliki z Shrimp Install do nowego konta.
mod agama;
mod apply;
mod data;
mod i18n;
mod logic;
mod names;
mod system;
mod transfer;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let root = PathBuf::from(arg(&args, "--root").map(String::as_str).unwrap_or("/mnt"));
    if args.iter().any(|a| a == "--apply") {
        return match apply::apply(&root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("shrimp-setup: --apply: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.iter().any(|a| a == "--restore") {
        let Some(user) = arg(&args, "--user") else {
            eprintln!("użycie: shrimp-setup --restore --user NAZWA [--root /mnt] [--from KATALOG] [--place folders|folder] [--folder NAZWA]");
            return ExitCode::FAILURE;
        };
        let placement = match arg(&args, "--place").map(String::as_str) {
            Some("folders") => Some(transfer::Placement::Folders),
            Some("folder") => Some(transfer::Placement::Folder(arg(&args, "--folder").cloned().unwrap_or_else(|| "Windows".into()))),
            _ => None,
        };
        return match transfer::restore(arg(&args, "--from").map(PathBuf::from), &root, user, placement) {
            Ok(st) => {
                println!("shrimp-setup: przywrócono {} plików ({} B)", st.files, st.bytes);
                for f in &st.failed {
                    eprintln!("shrimp-setup: nie przywrócono {f}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("shrimp-setup: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let demo = args.iter().any(|a| a == "--demo");
    let windowed = args.iter().any(|a| a == "--window");
    ui::run(demo, windowed)
}
