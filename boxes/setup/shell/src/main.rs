//! shrimp-setup: pełnoekranowe okno Setup Shrimp (GTK4 + WebKitGTK).
//!
//! `shrimp-setup --install`     na nośniku: interfejs serwuje agama-web-server (--web-ui-dir),
//!                              okno loguje się tokenem Agamy i otwiera kroki 1–9.
//! `shrimp-setup --first-boot`  po restarcie, w sesji nowego użytkownika: lokalny serwer
//!                              (server.rs) serwuje ten sam interfejs i API kroków 10–15.
//!                              Kończy od razu, jeśli pierwsza konfiguracja już się odbyła.
//! `shrimp-setup --restore --user NAZWA [--root /mnt] [--from KATALOG]`
//!                              po instalacji (post-install.sh): pliki z Shrimp Install do nowego konta.
mod server;
mod system;
mod transfer;

use gtk::prelude::*;
use gtk::glib;
use std::path::PathBuf;
use webkit6::prelude::*;

/// Gdzie leży zbudowany frontend (setup/web/dist).
const UI_DIR: &str = "/usr/share/shrimp-setup/ui";
/// Kopia tokenu Agamy czytelna dla użytkownika nośnika (shrimp-setup-token.service).
const TOKEN_FILES: [&str; 2] = ["/run/shrimp-setup/token", "/run/agama/token"];
const AGAMA_URL: &str = "http://127.0.0.1";
/// Po tylu sekundach bez tokenu Agamy `--install` zapisuje to w dzienniku (i czeka dalej).
const TOKEN_WAIT_SECS: u32 = 180;

enum Mode {
    Install,
    FirstBoot,
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1))
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--restore") {
        let Some(user) = arg(&args, "--user") else {
            eprintln!("użycie: shrimp-setup --restore --user NAZWA [--root /mnt] [--from KATALOG]");
            return glib::ExitCode::FAILURE;
        };
        let root = PathBuf::from(arg(&args, "--root").map(String::as_str).unwrap_or("/mnt"));
        return match transfer::restore(arg(&args, "--from").map(PathBuf::from), &root, user) {
            Ok(st) => {
                println!("shrimp-setup: przywrócono {} plików ({} B)", st.files, st.bytes);
                for f in &st.failed {
                    eprintln!("shrimp-setup: nie przywrócono {f}");
                }
                glib::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("shrimp-setup: {e}");
                glib::ExitCode::FAILURE
            }
        };
    }
    let mode = if args.iter().any(|a| a == "--install") {
        Mode::Install
    } else if args.iter().any(|a| a == "--first-boot" || a == "--serve") {
        Mode::FirstBoot
    } else {
        eprintln!("użycie: shrimp-setup --install | --first-boot | --serve [--ui-dir KATALOG] [--window]");
        return glib::ExitCode::FAILURE;
    };
    let ui_dir = arg(&args, "--ui-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(UI_DIR));
    let windowed = args.iter().any(|a| a == "--window");

    // Do testów: sam serwer pierwszej konfiguracji, bez okna i bez sprawdzania znacznika.
    if args.iter().any(|a| a == "--serve") {
        let (port, key) = server::spawn(ui_dir).expect("serwer");
        println!("http://127.0.0.1:{port}/?mode=firstboot&k={key}");
        loop { std::thread::park(); }
    }

    // Instalacja: okno pokazuje się od razu (czarne, jak ekran startu), a token Agamy czytamy w tle.
    // Wcześniej okno czekało na token do 3 minut i przez ten czas widać było pulpit sesji live.
    // Na nośniku lokalny serwer daje tylko sieć (krok „Połącz z internetem”): Agama pobiera system
    // z internetu, a laptop bez kabla nie ma go, dopóki nie wybierze się sieci Wi-Fi.
    let local = if matches!(mode, Mode::Install) {
        match server::spawn(ui_dir.clone()) {
            Ok((port, key)) => Some(format!("window.SHRIMP_LOCAL = {{ url: \"http://127.0.0.1:{port}/shrimp/api\", key: \"{key}\" }};")),
            Err(e) => {
                eprintln!("shrimp-setup: serwer sieci: {e}");
                None
            }
        }
    } else {
        None
    };
    let url = match mode {
        Mode::Install => None,
        Mode::FirstBoot => {
            if !system::first_boot_pending() {
                return glib::ExitCode::SUCCESS;
            }
            match server::spawn(ui_dir) {
                Ok((port, key)) => Some(format!("http://127.0.0.1:{port}/?mode=firstboot&k={key}")),
                Err(e) => {
                    eprintln!("shrimp-setup: serwer: {e}");
                    return glib::ExitCode::FAILURE;
                }
            }
        }
    };
    let install = matches!(mode, Mode::Install);
    // Podpowiedzi z Shrimp Install dla kroków 1–9 (imię, język, strefa, klawiatura, BitLocker).
    // Bez shrimp.json zostaje język menu GRUB, który Shrimp Install też bierze z Windows.
    let hint = if install {
        let dir = transfer::find_dir();
        let data = dir.as_deref().and_then(transfer::load);
        let grub = transfer::grub_lang();
        eprintln!(
            "shrimp-setup: shrimp-transfer {:?}, shrimp.json {}, język GRUB {:?}",
            dir,
            if data.is_some() { "wczytany" } else { "brak" },
            grub
        );
        Some(transfer::page_hint(data.as_ref(), grub) + local.as_deref().unwrap_or(""))
    } else {
        None
    };

    let app = gtk::Application::builder()
        .application_id("org.shrimp.Setup")
        .build();
    // Jedno okno: ponowne uruchomienie (skrót „Zainstaluj Shrimp” na nośniku) pokazuje istniejące.
    app.connect_activate(move |app| match app.active_window() {
        Some(w) => w.present(),
        None => build(app, url.as_deref(), install, windowed, hint.as_deref()),
    });
    // GTK nie zna naszych argumentów.
    app.run_with_args::<&str>(&[])
}

fn hide_overview() {
    use gtk::gio;
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return;
    };
    let args = ("org.gnome.Shell", "OverviewActive", false.to_variant()).to_variant();
    bus.call(
        Some("org.gnome.Shell"),
        "/org/gnome/Shell",
        "org.freedesktop.DBus.Properties",
        "Set",
        Some(&args),
        None,
        gio::DBusCallFlags::NONE,
        1000,
        gio::Cancellable::NONE,
        |_| {},
    );
}

/// Token Agamy (sesja live startuje szybciej niż Agama, więc może go jeszcze nie być).
fn read_token() -> Option<String> {
    TOKEN_FILES
        .iter()
        .find_map(|f| std::fs::read_to_string(f).ok())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Czarna strona z logo na czas czekania na Agamę: tak samo wygląda tło sesji live, osłona strażnika
/// Setup i pierwszy widok strony Setup, więc od startu do ekranu „Witaj” nic nie miga.
fn splash() -> String {
    let logo = include_str!("../../web/src/assets/logo.svg").replace("fill=\"#000\"", "fill=\"rgba(255,255,255,0.92)\"");
    format!(
        "<!doctype html><html><body style=\"margin:0;height:100vh;background:#000;cursor:default;display:flex;align-items:center;justify-content:center\">\
         <div style=\"width:96px;height:96px\">{logo}</div></body></html>"
    )
}

fn build(app: &gtk::Application, url: Option<&str>, install: bool, windowed: bool, hint: Option<&str>) {
    let view = webkit6::WebView::new();
    // Bez białego błysku przed pierwszą klatką strony.
    view.set_background_color(&gtk::gdk::RGBA::BLACK);
    if let (Some(js), Some(ucm)) = (hint, view.user_content_manager()) {
        ucm.add_script(&webkit6::UserScript::new(
            js,
            webkit6::UserContentInjectedFrames::TopFrame,
            webkit6::UserScriptInjectionTime::Start,
            &[],
            &[],
        ));
    }
    if let Some(s) = WebViewExt::settings(&view) {
        s.set_enable_developer_extras(std::env::var_os("SHRIMP_SETUP_DEBUG").is_some());
        s.set_hardware_acceleration_policy(webkit6::HardwareAccelerationPolicy::Never);
        // console.log strony (dyski Agamy, problemy instalacji) trafia do dziennika: journalctl --user -b | grep shrimp-setup
        s.set_enable_write_console_messages_to_stdout(true);
    }
    // Bez menu kontekstowego i bez nawigacji poza Setup.
    view.connect_context_menu(|_, _, _| true);
    // Serwer (Agama albo lokalny) może jeszcze nie słuchać: zamiast strony błędu
    // ponawiamy ładowanie co 2 s.
    view.connect_load_failed(|v, _ev, uri, err| {
        // Przerwane ładowanie (nowa nawigacja, przekierowanie) to nie błąd serwera.
        if err.matches(webkit6::NetworkError::Cancelled)
            || err.domain() == <webkit6::PolicyError as glib::error::ErrorDomain>::domain()
        {
            return false;
        }
        eprintln!("shrimp-setup: {uri}: {err}; ponawiam");
        let (v, uri) = (v.clone(), uri.to_string());
        glib::timeout_add_local_once(std::time::Duration::from_secs(2), move || v.load_uri(&uri));
        true
    });
    if install {
        // /login ustawia ciasteczko i przekierowuje na „/”; dopiero wtedy otwieramy tryb instalacji.
        view.connect_load_changed(|v, ev| {
            if ev == webkit6::LoadEvent::Finished {
                if let Some(uri) = v.uri() {
                    if uri.trim_end_matches('/') == AGAMA_URL || uri.starts_with(&format!("{AGAMA_URL}/?lang=")) {
                        v.load_uri(&format!("{AGAMA_URL}/?mode=install"));
                    }
                }
            }
        });
    }

    let win = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Shrimp")
        .default_width(1366)
        .default_height(768)
        .child(&view)
        .build();
    win.set_decorated(windowed);
    if !windowed {
        win.fullscreen();
        // Setup prowadzi do końca: zamknąć można tylko przyciskiem w ostatnim kroku.
        win.connect_close_request(|_| glib::Propagation::Stop);
    }
    match url {
        Some(u) => view.load_uri(u),
        None => {
            view.load_html(&splash(), None);
            let v = view.clone();
            let mut waited = 0u32;
            glib::timeout_add_local(std::time::Duration::from_millis(500), move || match read_token() {
                Some(t) => {
                    v.load_uri(&format!("{AGAMA_URL}/login?token={t}"));
                    glib::ControlFlow::Break
                }
                None => {
                    waited += 1;
                    if waited == TOKEN_WAIT_SECS * 2 {
                        eprintln!("shrimp-setup: brak tokenu Agamy po {TOKEN_WAIT_SECS} s, czekam dalej");
                    }
                    glib::ControlFlow::Continue
                }
            });
        }
    }
    win.present();
    if !windowed {
        // GNOME po zalogowaniu otwiera widok przeglądu i okno Setup jest wtedy tylko miniaturą.
        // Zamykamy przegląd przez D-Bus (właściwość OverviewActive), dopiero po animacji startowej:
        // zamknięty w jej trakcie zostawiał niewidzialną warstwę GNOME, która połykała kliknięcia.
        for delay in [6, 12] {
            glib::timeout_add_seconds_local_once(delay, hide_overview);
        }
    }

    // Serwer pierwszej konfiguracji prosi o zamknięcie okna po „Zaczynajmy”.
    let app = app.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(300), move || {
        if server::QUIT.load(std::sync::atomic::Ordering::SeqCst) {
            app.quit();
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}
