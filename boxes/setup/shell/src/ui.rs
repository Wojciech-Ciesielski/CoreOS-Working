//! Okno kreatora: jedna ciągła prezentacja od „Witaj” po restart. Każdy krok buduje swój widok od nowa
//! (render), stan jest w [`S`]. Wszystko, co blokuje (Agama, nmcli), idzie w osobnym wątku ([`bg`]).
use crate::agama::{self, Choices, Client, Disk, InstallError, InstallPlan, Place};
use crate::data::{self, ErrKey, Keyboard, Lang, Region, ACCENTS, SHRIMP_MIN_GB};
use crate::i18n::{gb, recommended, t, tf};
use crate::logic::{self, Prefill, Transfer, WinApp};
use crate::names;
use crate::system::{self, Network};
use crate::transfer;
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const APP_ID: &str = "org.shrimp.Setup";
const LOGO_ICON: &str = "shrimp-setup-logo-symbolic";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Witaj,
    Jezyk,
    Licencja,
    Region,
    Klawiatura,
    Internet,
    Dysk,
    Konto,
    Pliki,
    Pendrive,
    Przegladarka,
    Programy,
    Wyglad,
    Podsumowanie,
    Instalacja,
    Restart,
}

use Step::*;
const STEPS: [Step; 16] = [Witaj, Jezyk, Licencja, Region, Klawiatura, Internet, Dysk, Konto, Pliki, Pendrive, Przegladarka, Programy, Wyglad, Podsumowanie, Instalacja, Restart];

fn idx(s: Step) -> usize {
    STEPS.iter().position(|x| *x == s).unwrap()
}

/// Stan kreatora.
struct S {
    langs: Vec<Lang>,
    regs: Vec<Region>,
    kbs: Vec<Keyboard>,
    disks: Vec<Disk>,
    nets: Vec<Network>,
    tr: Option<Transfer>,
    pre: Option<Prefill>,
    usb: Option<(u64, u64)>,
    win_apps: Vec<WinApp>,
    no_alt: Vec<String>,
    nvidia: Option<String>,
    online: bool,
    offline_repo: bool,
    lang: usize,
    region: usize,
    kb: usize,
    disk: usize,
    /// 0 = obok Windows, 1 = zamiast Windows.
    place: usize,
    win_gb: u64,
    enc: bool,
    dp: String,
    dp2: String,
    full: String,
    user: String,
    user_edited: bool,
    pass: String,
    pass2: String,
    lic_yes: bool,
    files_into: bool,
    usb_restore: bool,
    browser: usize,
    sel: HashMap<String, bool>,
    dark: bool,
    accent: Option<usize>,
    text_scale: f64,
    night: bool,
    auto: bool,
    net: usize,
    net_pass: String,
    hsec: usize,
    ssid: String,
    hpass: String,
    step: usize,
    sub: u8,
    disk_loading: bool,
    disk_seq: u32,
    busy: bool,
    /// Instalacja: postęp z wątku (procent, faza, minuty do końca).
    progress: Arc<Mutex<(f64, &'static str, Option<u64>)>>,
}

impl S {
    fn cur(&self) -> Step {
        STEPS[self.step]
    }
    fn disk(&self) -> Option<&Disk> {
        self.disks.get(self.disk)
    }
    fn has_win(&self) -> bool {
        self.disk().is_some_and(|d| d.windows.is_some())
    }
    /// Windows z BitLockerem: tylko „Zamiast Windows”.
    fn locked(&self) -> bool {
        self.has_win() && (self.disk().unwrap().windows.as_ref().unwrap().bitlocker || self.tr.as_ref().is_some_and(|t| t.bitlocker))
    }
    fn shrimp_min(&self) -> u64 {
        SHRIMP_MIN_GB + self.tr.as_ref().map(|t| t.bytes.div_ceil(1_000_000_000)).unwrap_or(0)
    }
    fn beside(&self) -> bool {
        self.has_win() && self.place == 0
    }
    fn pool(&self) -> u64 {
        self.disk().map(|d| d.windows.as_ref().map(|w| w.pool_gb).unwrap_or(d.size_gb)).unwrap_or(0)
    }
    fn win_min(&self) -> u64 {
        self.disk().and_then(|d| d.windows.as_ref()).map(|w| w.min_gb).unwrap_or(0)
    }
    fn place_kind(&self) -> Place {
        if !self.has_win() {
            Place::Whole
        } else if self.place == 0 {
            Place::Beside
        } else {
            Place::Replace
        }
    }
    fn locale(&self) -> String {
        self.langs.get(self.lang).map(|l| l.locale.clone()).unwrap_or_else(|| "en_US.UTF-8".into())
    }
    fn browsers(&self) -> Vec<data::Browser> {
        let (names, prog) = self.tr.as_ref().map(|t| (t.apps.clone(), t.default_browser.clone())).unwrap_or_default();
        logic::browser_choices(&names, prog.as_deref())
    }
    fn win_browser(&self) -> Option<usize> {
        let (names, prog) = self.tr.as_ref().map(|t| (t.apps.clone(), t.default_browser.clone())).unwrap_or_default();
        logic::windows_browser(&self.browsers(), &names, prog.as_deref())
    }
    fn win_list(&self) -> Vec<WinApp> {
        logic::windows_list(&self.win_apps)
    }
    fn files(&self) -> u64 {
        self.tr.as_ref().map(|t| t.bytes).unwrap_or(0)
    }
    fn default_split(&mut self) {
        let min = self.shrimp_min();
        let locked = self.locked();
        if let Some(w) = self.disk().and_then(|d| d.windows.clone()) {
            let half = w.min_gb as f64 + (w.pool_gb.saturating_sub(w.min_gb)) as f64 / 2.0;
            self.win_gb = (half.min(w.pool_gb.saturating_sub(min) as f64).max(w.min_gb as f64)).round() as u64;
            if locked {
                self.place = 1;
            }
        }
    }
    fn plan(&self) -> Option<InstallPlan> {
        let disk = self.disk()?.clone();
        let browser = self.browsers().get(self.browser).map(|b| b.id.to_string()).unwrap_or_else(|| "org.mozilla.firefox".into());
        let mut apps: Vec<String> = Vec::new();
        for a in self.win_list() {
            if self.sel.get(&a.windows) != Some(&false) && !apps.contains(&a.id) {
                apps.push(a.id);
            }
        }
        Some(InstallPlan {
            locale: self.locale(),
            timezone: self.regs.get(self.region)?.tz.to_string(),
            keymap: self.kbs.get(self.kb)?.keymap.to_string(),
            place: self.place_kind(),
            windows_gb: self.beside().then_some(self.win_gb),
            disk,
            disk_password: self.enc.then(|| self.dp.clone()),
            full_name: self.full.trim().to_string(),
            user_name: self.user.clone(),
            password: self.pass.clone(),
            choices: Choices {
                proprietary: self.lic_yes,
                files: if self.files_into { "folders".into() } else { "folder".into() },
                files_folder: t("files.folder").to_string(),
                usb: if self.usb_restore { "restore".into() } else { "home".into() },
                browser,
                apps,
                color_scheme: if self.dark { "prefer-dark".into() } else { "default".into() },
                accent: self.accent.map(|i| ACCENTS[i].1.to_string()).unwrap_or_default(),
                text_scale: self.text_scale,
                night_light: self.night,
                auto_updates: self.auto,
            },
        })
    }
}

struct Ui {
    s: RefCell<S>,
    client: RefCell<Option<Client>>,
    demo: bool,
    win: adw::ApplicationWindow,
    stack: gtk::Stack,
    back: gtk::Button,
    accent_css: gtk::CssProvider,
    seq: Cell<u64>,
    timers: RefCell<Vec<(glib::SourceId, Rc<Cell<bool>>)>>,
}

/// Robota w wątku, wynik z powrotem w wątku okna.
fn bg<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
    glib::spawn_future_local(async move {
        if let Ok(r) = gio::spawn_blocking(f).await {
            done(r);
        }
    });
}

pub fn run(demo: bool, windowed: bool) -> ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).flags(gio::ApplicationFlags::NON_UNIQUE).build();
    app.connect_activate(move |app| {
        if let Some(w) = app.active_window() {
            w.present();
            return;
        }
        setup_style();
        let ui = Ui::new(app, demo, windowed);
        ui.start();
    });
    let code = app.run_with_args::<&str>(&[]);
    if code == glib::ExitCode::SUCCESS { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// Logo jako ikona symboliczna (barwi się kolorem tekstu, jasny i ciemny wygląd) i style kreatora.
fn setup_style() {
    let dir = std::env::temp_dir().join(format!("shrimp-setup-{}", std::process::id())).join("icons");
    let apps = dir.join("hicolor/scalable/apps");
    if std::fs::create_dir_all(&apps).is_ok() {
        let svg = include_str!("../data/logo.svg").replace(r#"viewBox="0 0 190 190""#, r#"viewBox="-1.1 3.6 184.6 178.5""#);
        let _ = std::fs::write(apps.join(format!("{LOGO_ICON}.svg")), svg);
        if let Some(d) = gdk::Display::default() {
            gtk::IconTheme::for_display(&d).add_search_path(&dir);
        }
    }
    let css = gtk::CssProvider::new();
    let mut s = String::from(
        ".setup-title { font-size: 26pt; font-weight: 800; }
         .setup-sub { font-size: 13pt; opacity: 0.75; }
         .hello { font-size: 52pt; font-weight: 800; }
         .splash { background: black; color: rgba(255,255,255,0.92); }
         .setup-desc { opacity: 0.75; }
         .setup-status { opacity: 0.8; font-style: italic; }
         .slide-title { font-size: 17pt; font-weight: 700; }
         .split-label { font-size: 12pt; }
         .split-label .value { font-weight: 700; font-size: 15pt; }
         .look-card { padding: 18px 28px; border-radius: 14px; }
         .look-card.light { background: #fafafa; color: #222; }
         .look-card.dark { background: #242424; color: #eee; }
         .swatch { min-width: 34px; min-height: 34px; border-radius: 999px; padding: 0; border: 3px solid transparent; }
         .swatch:checked { border-color: @window_fg_color; }
         .back-btn { margin: 18px; }
         ",
    );
    for (_, name, color) in ACCENTS {
        s += &format!(".swatch.{name} {{ background: {color}; }}\n");
    }
    css.load_from_string(&s);
    if let Some(d) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&d, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

fn label(text: &str, classes: &[&str]) -> gtk::Label {
    let l = gtk::Label::builder().label(text).wrap(true).justify(gtk::Justification::Center).build();
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    for c in classes {
        l.add_css_class(c);
    }
    l
}

fn logo(size: i32) -> gtk::Image {
    let i = gtk::Image::from_icon_name(LOGO_ICON);
    i.set_pixel_size(size);
    i
}

fn pill(text: &str, suggested: bool) -> gtk::Button {
    let b = gtk::Button::with_label(text);
    b.add_css_class("pill");
    if suggested {
        b.add_css_class("suggested-action");
    }
    b
}

fn boxed_list() -> gtk::ListBox {
    let l = gtk::ListBox::new();
    l.add_css_class("boxed-list");
    l.set_selection_mode(gtk::SelectionMode::None);
    l
}

fn scrolled(child: &impl IsA<gtk::Widget>, min: i32, max: i32) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .child(child)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(min)
        .max_content_height(max)
        .propagate_natural_height(true)
        .build()
}

/// Lista jednego wyboru: wiersze z zaznaczeniem po prawej (strzałki, Enter, kliknięcie).
fn choice_list(items: &[(String, String, bool)], selected: usize, on: impl Fn(usize) + 'static) -> gtk::ListBox {
    let list = boxed_list();
    list.set_selection_mode(gtk::SelectionMode::Single);
    let marks: Rc<RefCell<Vec<gtk::Image>>> = Rc::default();
    for (i, (title, sub, enabled)) in items.iter().enumerate() {
        let row = adw::ActionRow::builder().title(glib::markup_escape_text(title).as_str()).activatable(true).build();
        if !sub.is_empty() {
            row.set_subtitle(glib::markup_escape_text(sub).as_str());
        }
        let mark = gtk::Image::from_icon_name("object-select-symbolic");
        mark.set_visible(i == selected);
        row.add_suffix(&mark);
        row.set_sensitive(*enabled);
        marks.borrow_mut().push(mark);
        list.append(&row);
    }
    if let Some(r) = list.row_at_index(selected as i32) {
        list.select_row(Some(&r));
    }
    let on = Rc::new(on);
    let m = marks.clone();
    list.connect_row_selected(move |_, row| {
        let Some(row) = row else { return };
        let i = row.index() as usize;
        for (k, mk) in m.borrow().iter().enumerate() {
            mk.set_visible(k == i);
        }
        on(i);
    });
    list
}

/// Klucz sortowania nazw: bez wielkości liter i znaków diakrytycznych (Å, Ö, Ś obok A, O, S).
fn sort_key(s: &str) -> String {
    glib::casefold(glib::normalize(s, glib::NormalizeMode::Default).as_str())
        .chars()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect()
}

/// Zaznaczony wiersz widoczny po zbudowaniu widoku (długie listy: język, kraj, klawiatura).
fn focus_selected(list: &gtk::ListBox) {
    let l = list.clone();
    glib::idle_add_local_once(move || {
        if let Some(r) = l.selected_row() {
            r.grab_focus();
        }
    });
}

impl Ui {
    fn new(app: &adw::Application, demo: bool, windowed: bool) -> Rc<Ui> {
        let stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::Crossfade).transition_duration(250).hexpand(true).vexpand(true).build();
        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        back.add_css_class("circular");
        back.add_css_class("back-btn");
        back.set_halign(gtk::Align::Start);
        back.set_valign(gtk::Align::Start);
        back.set_visible(false);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&stack));
        overlay.add_overlay(&back);
        let win = adw::ApplicationWindow::builder().application(app).title("Shrimp").default_width(1280).default_height(800).content(&overlay).build();
        if !windowed {
            win.fullscreen();
            // Setup prowadzi do końca: zamknąć okno można tylko restartem w ostatnim kroku.
            win.connect_close_request(|_| glib::Propagation::Stop);
        }
        let accent_css = gtk::CssProvider::new();
        if let Some(d) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&d, &accent_css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        }
        let s = S {
            langs: data::langs(),
            regs: data::REGIONS.to_vec(),
            kbs: data::KEYBOARDS.to_vec(),
            disks: vec![],
            nets: vec![],
            tr: None,
            pre: None,
            usb: None,
            win_apps: vec![],
            no_alt: vec![],
            nvidia: None,
            online: true,
            offline_repo: false,
            lang: 0,
            region: 0,
            kb: 0,
            disk: 0,
            place: 0,
            win_gb: 0,
            enc: false,
            dp: String::new(),
            dp2: String::new(),
            full: String::new(),
            user: String::new(),
            user_edited: false,
            pass: String::new(),
            pass2: String::new(),
            lic_yes: true,
            files_into: true,
            usb_restore: true,
            browser: 0,
            sel: HashMap::new(),
            dark: false,
            accent: None,
            text_scale: 1.0,
            night: false,
            auto: true,
            net: 0,
            net_pass: String::new(),
            hsec: 1,
            ssid: String::new(),
            hpass: String::new(),
            step: 0,
            sub: 0,
            disk_loading: false,
            disk_seq: 0,
            busy: false,
            progress: Arc::new(Mutex::new((0.0, "inst.disk", None))),
        };
        let ui = Rc::new(Ui { s: RefCell::new(s), client: RefCell::new(None), demo, win, stack, back, accent_css, seq: Cell::new(0), timers: RefCell::new(vec![]) });
        let u = ui.clone();
        ui.back.connect_clicked(move |_| u.go_back());
        // Esc wraca, jak przycisk Wstecz (gdy jest widoczny).
        let keys = gtk::EventControllerKey::new();
        let u = ui.clone();
        keys.connect_key_pressed(move |_, k, _, _| {
            if k == gdk::Key::Escape && u.back.is_visible() {
                u.go_back();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        ui.win.add_controller(keys);
        // Test QEMU (faza 0): czy kliknięcie dochodzi do okna w sesji kiosku. Jeden wpis w dzienniku.
        let first = gtk::GestureClick::new();
        first.set_propagation_phase(gtk::PropagationPhase::Capture);
        let logged = Cell::new(false);
        first.connect_pressed(move |_, _, x, y| {
            if !logged.replace(true) {
                eprintln!("shrimp-setup: pierwsze kliknięcie {x:.0},{y:.0}");
            }
        });
        ui.win.add_controller(first);
        ui
    }

    fn client(&self) -> Option<Client> {
        self.client.borrow().clone()
    }

    // ---------- start ----------

    fn start(self: &Rc<Self>) {
        // Czarny ekran z logo, jak ekran startu i tło sesji: od włączenia komputera do „Witaj” nic nie miga.
        let b = gtk::Box::new(gtk::Orientation::Vertical, 24);
        b.add_css_class("splash");
        b.set_valign(gtk::Align::Fill);
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 24);
        inner.set_valign(gtk::Align::Center);
        inner.set_vexpand(true);
        inner.append(&logo(96));
        let sp = gtk::Spinner::new();
        sp.start();
        inner.append(&sp);
        b.append(&inner);
        self.stack.add_named(&b, Some("splash"));
        self.win.present();
        eprintln!("shrimp-setup: start (kreator GTK{})", if self.demo { ", makieta" } else { "" });
        let demo = self.demo;
        let u = self.clone();
        bg(
            move || {
                let client = if demo { None } else { Client::connect(Duration::from_secs(900)) };
                if !demo && client.is_none() {
                    eprintln!("shrimp-setup: Agama nie odpowiada po 15 min");
                }
                let sys = client.as_ref().and_then(|c| c.system().ok());
                let dir = transfer::find_dir();
                let tr = dir.as_deref().and_then(transfer::load).map(|v| transfer::setup_transfer(&v));
                eprintln!("shrimp-setup: shrimp-transfer {dir:?}, shrimp.json {}", if tr.is_some() { "wczytany" } else { "brak" });
                let offline_repo = system::offline_repo();
                let online = demo || offline_repo && system::online() || system::online();
                let nets = if online { vec![] } else { system::networks() };
                let usb = if demo { Some((120, 3_400_000_000)) } else { transfer::usb_backup() };
                (client, sys, tr, offline_repo, online, nets, usb, system::nvidia_gpu())
            },
            move |(client, sys, tr, offline_repo, online, nets, usb, nvidia)| {
                *u.client.borrow_mut() = client;
                u.init(sys, tr, offline_repo, online, nets, usb, nvidia);
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn init(self: &Rc<Self>, sys: Option<agama::ASystem>, tr: Option<Transfer>, offline_repo: bool, online: bool, nets: Vec<Network>, usb: Option<(u64, u64)>, nvidia: Option<String>) {
        {
            let mut s = self.s.borrow_mut();
            // Tylko to, co zna system nośnika (Agama): języki, strefy, układy.
            if let Some(sys) = &sys {
                let ids = |v: &Vec<agama::AId>| v.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
                let (l, z, k) = (ids(&sys.l10n.locales), ids(&sys.l10n.timezones), ids(&sys.l10n.keymaps));
                if !l.is_empty() {
                    s.langs.retain(|x| l.contains(&x.locale));
                }
                if !z.is_empty() {
                    s.regs.retain(|x| z.iter().any(|y| y == x.tz));
                }
                if !k.is_empty() {
                    s.kbs.retain(|x| k.iter().any(|y| y == x.keymap));
                }
            }
            s.offline_repo = offline_repo;
            s.online = online;
            s.nets = nets;
            s.usb = usb.filter(|u| u.0 > 0);
            s.nvidia = nvidia;
            if let Some(t) = &tr {
                s.win_apps = logic::map_windows_apps(&t.apps, &t.store_apps);
                s.no_alt = logic::no_alternative(&t.apps, &t.store_apps);
            }
            if self.demo && tr.is_none() {
                let names: Vec<String> = ["Google Chrome", "Spotify", "Discord", "Microsoft 365 Apps for business - pl-pl", "TeamViewer 15"].iter().map(|x| x.to_string()).collect();
                s.win_apps = logic::map_windows_apps(&names, &[]);
                s.no_alt = logic::no_alternative(&names, &[]);
                s.tr = Some(Transfer { apps: names, bytes: 5_200_000_000, folders: vec![("documents".into(), 0, 3_100_000_000), ("pictures".into(), 0, 2_100_000_000)], ..Default::default() });
                s.full = "Anna Kowalska".into();
                s.user = "anna".into();
            }
            if tr.is_some() {
                s.tr = tr;
            }
            let own = logic::install_lang(s.tr.as_ref(), &s.langs);
            s.lang = own;
            if let Some(t) = s.tr.clone() {
                let p = logic::prefill(&t, &s.langs, &s.regs, &s.kbs);
                if let Some(l) = p.lang {
                    s.lang = l;
                }
                if !p.full.is_empty() {
                    s.full = p.full.clone();
                }
                if !p.user.is_empty() {
                    s.user = p.user.clone();
                    s.user_edited = true;
                }
                s.pre = Some(p);
            }
            Self::lang_defaults(&mut s);
            let wb = s.win_browser();
            s.browser = wb.unwrap_or(0);
        }
        self.apply_lang();
        let kb = self.s.borrow().kbs.get(self.s.borrow().kb).map(|k| k.keymap);
        if let Some(k) = kb {
            self.set_keymap(k);
        }
        self.load_disks(false);
        // Makieta: SHRIMP_SETUP_STEP=Dysk otwiera od razu ten krok (podgląd widoków bez klikania).
        let first = std::env::var("SHRIMP_SETUP_STEP").ok().filter(|_| self.demo).and_then(|n| STEPS.iter().position(|s| format!("{s:?}").eq_ignore_ascii_case(&n)));
        self.go(first.unwrap_or(0), 0);
    }

    /// Strefa i klawiatura dla wybranego języka (z Windows, gdy język też jest z Windows).
    fn lang_defaults(s: &mut S) {
        let locale = s.locale();
        let same = s.pre.as_ref().is_some_and(|p| p.lang == Some(s.lang));
        let pre_region = s.pre.as_ref().and_then(|p| p.region).filter(|_| same);
        let pre_kb = s.pre.as_ref().and_then(|p| p.kb).filter(|_| same);
        if let Some(r) = pre_region.or_else(|| data::default_tz(&locale).and_then(|z| s.regs.iter().position(|r| r.tz == z))) {
            s.region = r;
        }
        let km = data::default_keymap(&locale);
        if let Some(k) = pre_kb.or_else(|| s.kbs.iter().position(|k| k.keymap == km)) {
            s.kb = k;
        }
    }

    fn apply_lang(&self) {
        let tag = self.s.borrow().langs.get(self.s.borrow().lang).map(|l| l.tag.clone()).unwrap_or_else(|| "en".into());
        let rtl = crate::i18n::set_lang(&tag);
        gtk::Widget::set_default_direction(if rtl { gtk::TextDirection::Rtl } else { gtk::TextDirection::Ltr });
    }

    fn set_keymap(&self, keymap: &'static str) {
        let c = self.client();
        let demo = self.demo;
        bg(
            move || {
                if let Some(c) = c {
                    c.set_keymap(keymap);
                }
                if !demo {
                    let _ = system::input_source(keymap);
                }
            },
            |_| {},
        );
    }

    /// Dyski od nowa: pierwszy odczyt bywa niepełny, bo Agama jeszcze sonduje (dysk bez partycji = bez „Obok Windows”).
    fn load_disks(self: &Rc<Self>, show: bool) {
        let seq = {
            let mut s = self.s.borrow_mut();
            s.disk_seq += 1;
            s.disk_loading = show;
            s.disk_seq
        };
        let c = self.client();
        let demo = self.demo;
        let u = self.clone();
        bg(
            move || match c {
                Some(c) => c.disks(),
                None if demo => vec![
                    Disk { device: "/dev/nvme0n1".into(), name: "Samsung SSD 980".into(), size_gb: 512, windows: Some(agama::WinPart { partition: "/dev/nvme0n1p3".into(), min_gb: 180, pool_gb: 500, bitlocker: false }) },
                    Disk { device: "/dev/sda".into(), name: "WD Blue".into(), size_gb: 1000, windows: None },
                ],
                None => vec![],
            },
            move |d| {
                let rerender = {
                    let mut s = u.s.borrow_mut();
                    if seq != s.disk_seq {
                        return;
                    }
                    s.disk_loading = false;
                    if !d.is_empty() && d != s.disks {
                        let had = s.disk().is_some_and(|x| x.windows.is_some());
                        s.disks = d;
                        s.disk = s.disk.min(s.disks.len() - 1);
                        if !had && s.has_win() {
                            s.place = 0;
                        }
                        s.default_split();
                    }
                    matches!(s.cur(), Dysk | Podsumowanie)
                };
                if rerender {
                    u.render();
                }
            },
        );
    }

    // ---------- nawigacja ----------

    fn skip(s: &S, step: Step, sub: u8) -> bool {
        match step {
            Internet => sub == 0 && s.online,
            Pliki => s.files() == 0,
            Pendrive => s.usb.is_none(),
            _ => false,
        }
    }

    fn go(self: &Rc<Self>, n: usize, sub: u8) {
        self.clear_timers();
        let enter_disk = {
            let mut s = self.s.borrow_mut();
            let prev = s.step;
            let mut n = n.min(STEPS.len() - 1);
            let fwd = n >= prev;
            while Self::skip(&s, STEPS[n], sub) {
                n = if fwd { (n + 1).min(STEPS.len() - 1) } else { n.saturating_sub(1) };
            }
            s.step = n;
            s.sub = sub;
            s.cur() == Dysk && prev != n
        };
        if enter_disk {
            self.load_disks(true);
        }
        self.render();
    }

    fn next(self: &Rc<Self>) {
        let n = self.s.borrow().step + 1;
        self.go(n, 0);
    }

    fn go_back(self: &Rc<Self>) {
        let (step, sub) = {
            let s = self.s.borrow();
            (s.step, s.sub)
        };
        if sub > 0 {
            self.go(step, sub - 1);
        } else if step > 0 {
            self.go(step - 1, 0);
        }
    }

    fn clear_timers(&self) {
        for (id, alive) in self.timers.borrow_mut().drain(..) {
            // Zatrzymany sam (Break) już nie istnieje; drugie usunięcie to panika w glib.
            if alive.replace(false) {
                id.remove();
            }
        }
    }

    fn timer(self: &Rc<Self>, every: Duration, f: impl Fn(&Rc<Ui>) -> bool + 'static) {
        let u = self.clone();
        let alive: Rc<Cell<bool>> = Rc::new(Cell::new(true));
        let a = alive.clone();
        let id = glib::timeout_add_local(every, move || {
            if !a.get() || !f(&u) {
                a.set(false);
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        self.timers.borrow_mut().push((id, alive));
    }

    /// Nowy widok kroku z płynnym przejściem; poprzedni znika po animacji.
    fn render(self: &Rc<Self>) {
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let (view, back) = self.view();
        let name = format!("v{seq}");
        let old = self.stack.visible_child();
        self.stack.add_named(&view, Some(&name));
        self.stack.set_visible_child_name(&name);
        self.back.set_visible(back);
        if let Some(old) = old {
            let st = self.stack.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || {
                if old.parent().is_some() {
                    st.remove(&old);
                }
            });
        }
    }

    /// Szkielet widoku: logo, tytuł, podtytuł, treść, przyciski.
    fn page(&self, logo_size: i32, title: &str, sub: &str, body: Option<gtk::Widget>, acts: &[gtk::Widget]) -> gtk::Widget {
        let b = gtk::Box::new(gtk::Orientation::Vertical, 14);
        b.set_valign(gtk::Align::Center);
        b.set_margin_top(48);
        b.set_margin_bottom(48);
        b.set_margin_start(24);
        b.set_margin_end(24);
        if logo_size > 0 {
            let l = logo(logo_size);
            l.set_margin_bottom(6);
            b.append(&l);
        }
        if !title.is_empty() {
            b.append(&label(title, &["setup-title"]));
        }
        if !sub.is_empty() {
            b.append(&label(sub, &["setup-sub"]));
        }
        if let Some(w) = body {
            w.set_margin_top(14);
            b.append(&w);
        }
        if !acts.is_empty() {
            let a = gtk::Box::new(gtk::Orientation::Vertical, 10);
            a.set_halign(gtk::Align::Center);
            a.set_margin_top(18);
            for w in acts {
                a.append(w);
            }
            b.append(&a);
        }
        let clamp = adw::Clamp::builder().maximum_size(680).child(&b).build();
        gtk::ScrolledWindow::builder().child(&clamp).hscrollbar_policy(gtk::PolicyType::Never).build().upcast()
    }

    fn next_btn(self: &Rc<Self>, text: &str) -> gtk::Button {
        let b = pill(text, true);
        let u = self.clone();
        b.connect_clicked(move |_| u.act_next());
        self.win.set_default_widget(Some(&b));
        b
    }

    fn link_btn(self: &Rc<Self>, text: &str, f: impl Fn(&Rc<Ui>) + 'static) -> gtk::Button {
        let b = gtk::Button::with_label(text);
        b.add_css_class("flat");
        let u = self.clone();
        b.connect_clicked(move |_| f(&u));
        b
    }

    fn view(self: &Rc<Self>) -> (gtk::Widget, bool) {
        let (cur, sub, step) = {
            let s = self.s.borrow();
            (s.cur(), s.sub, s.step)
        };
        let back = step > 0 && !matches!(cur, Witaj | Instalacja | Restart);
        let w = match cur {
            Witaj => self.v_witaj(),
            Jezyk => self.v_jezyk(),
            Licencja => self.v_licencja(),
            Region => self.v_region(),
            Klawiatura => self.v_klawiatura(),
            Internet if sub == 1 => self.v_hidden(),
            Internet => self.v_internet(),
            Dysk if sub == 1 => self.v_dysk_adv(),
            Dysk => self.v_dysk(),
            Konto => self.v_konto(),
            Pliki => self.v_pliki(),
            Pendrive => self.v_pendrive(),
            Przegladarka => self.v_przegladarka(),
            Programy => self.v_programy(),
            Wyglad => self.v_wyglad(),
            Podsumowanie => self.v_podsumowanie(),
            Instalacja => self.v_instalacja(),
            Restart => self.v_restart(),
        };
        (w, back)
    }

    // ---------- widoki ----------

    fn v_witaj(self: &Rc<Self>) -> gtk::Widget {
        let hello = label(data::HELLO[0], &["hello"]);
        let k = Rc::new(Cell::new(0usize));
        let h = hello.clone();
        self.timer(Duration::from_millis(2300), move |_| {
            k.set((k.get() + 1) % data::HELLO.len());
            h.set_label(data::HELLO[k.get()]);
            true
        });
        let b = gtk::Box::new(gtk::Orientation::Vertical, 10);
        b.append(&hello);
        b.append(&label(t("welcome.sub"), &["setup-sub"]));
        self.page(128, "", "", Some(b.upcast()), &[self.next_btn(t("start")).upcast()])
    }

    fn v_jezyk(self: &Rc<Self>) -> gtk::Widget {
        let (items, sel) = {
            let s = self.s.borrow();
            (s.langs.iter().map(|l| (l.name.clone(), String::new(), true)).collect::<Vec<_>>(), s.lang)
        };
        let u = self.clone();
        let pending: Rc<Cell<Option<glib::SourceId>>> = Rc::default();
        let list = choice_list(&items, sel, move |i| {
            if u.s.borrow().lang == i {
                return;
            }
            u.s.borrow_mut().lang = i;
            // Interfejs przechodzi na wybrany język, gdy wybór się zatrzyma (przewijanie strzałkami nie przebudowuje widoku co krok).
            if let Some(id) = pending.take() {
                id.remove();
            }
            let (u2, p2) = (u.clone(), pending.clone());
            pending.set(Some(glib::timeout_add_local_once(Duration::from_millis(450), move || {
                p2.set(None);
                u2.apply_lang();
                u2.render();
            })));
        });
        focus_selected(&list);
        self.page(56, t("lang.title"), "", Some(scrolled(&list, 300, 420).upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn v_licencja(self: &Rc<Self>) -> gtk::Widget {
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 8);
        text.set_margin_top(10);
        text.set_margin_bottom(10);
        text.set_margin_start(14);
        text.set_margin_end(14);
        for (h, p) in data::LIC_TEXT {
            let hl = gtk::Label::builder().label(t(h)).xalign(0.0).wrap(true).build();
            hl.add_css_class("heading");
            text.append(&hl);
            text.append(&gtk::Label::builder().label(t(p)).xalign(0.0).wrap(true).build());
        }
        let frame = gtk::Frame::new(None);
        frame.set_child(Some(&scrolled(&text, 160, 220)));
        body.append(&frame);
        let yes = self.s.borrow().lic_yes;
        let desc = label(t(if yes { "lic.yes.d" } else { "lic.no.d" }), &["setup-desc"]);
        let d = desc.clone();
        let u = self.clone();
        let list = choice_list(&[(format!("{} ({})", t("lic.yes"), recommended()), String::new(), true), (t("lic.no").into(), String::new(), true)], if yes { 0 } else { 1 }, move |i| {
            u.s.borrow_mut().lic_yes = i == 0;
            d.set_label(t(if i == 0 { "lic.yes.d" } else { "lic.no.d" }));
        });
        body.append(&list);
        body.append(&desc);
        self.page(56, t("lic.title"), t("lic.sub"), Some(body.upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn v_region(self: &Rc<Self>) -> gtk::Widget {
        // Kraje alfabetycznie w języku interfejsu; `order` mapuje wiersz listy na indeks w `regs`.
        let (items, sel, order) = {
            let s = self.s.borrow();
            let loc = s.locale();
            let mut rows: Vec<(String, usize)> = s.regs.iter().enumerate().map(|(i, r)| (names::region_label(r, &loc), i)).collect();
            rows.sort_by_key(|(l, _)| sort_key(l));
            let order: Vec<usize> = rows.iter().map(|(_, i)| *i).collect();
            let sel = order.iter().position(|&i| i == s.region).unwrap_or(0);
            (rows.into_iter().map(|(l, _)| (l, String::new(), true)).collect::<Vec<_>>(), sel, order)
        };
        let tz_now = |s: &S| s.regs.get(s.region).map(|r| format!("{} · {}", r.tz.replace('_', " "), names::time_in(r.tz))).unwrap_or_default();
        let desc = label(&tz_now(&self.s.borrow()), &["setup-desc"]);
        let (u, d) = (self.clone(), desc.clone());
        let list = choice_list(&items, sel, move |i| {
            u.s.borrow_mut().region = order[i];
            d.set_label(&tz_now(&u.s.borrow()));
        });
        let d = desc.clone();
        self.timer(Duration::from_secs(15), move |u| {
            d.set_label(&tz_now(&u.s.borrow()));
            true
        });
        focus_selected(&list);
        let b = gtk::Box::new(gtk::Orientation::Vertical, 10);
        b.append(&scrolled(&list, 280, 380));
        b.append(&desc);
        self.page(56, t("region.title"), t("region.sub"), Some(b.upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn v_klawiatura(self: &Rc<Self>) -> gtk::Widget {
        let (items, sel) = {
            let s = self.s.borrow();
            let loc = s.locale();
            (s.kbs.iter().map(|k| (names::keyboard_label(k, &loc), String::new(), true)).collect::<Vec<_>>(), s.kb)
        };
        let u = self.clone();
        let pending: Rc<Cell<Option<glib::SourceId>>> = Rc::default();
        let list = choice_list(&items, sel, move |i| {
            u.s.borrow_mut().kb = i;
            if let Some(id) = pending.take() {
                id.remove();
            }
            let (u2, p2) = (u.clone(), pending.clone());
            pending.set(Some(glib::timeout_add_local_once(Duration::from_millis(250), move || {
                p2.set(None);
                let k = u2.s.borrow().kbs[i].keymap;
                u2.set_keymap(k);
            })));
        });
        focus_selected(&list);
        let test = adw::EntryRow::builder().title(t("kb.test")).build();
        let tl = boxed_list();
        tl.append(&test);
        let b = gtk::Box::new(gtk::Orientation::Vertical, 14);
        b.append(&scrolled(&list, 260, 340));
        b.append(&tl);
        self.page(56, t("kb.title"), "", Some(b.upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn v_internet(self: &Rc<Self>) -> gtk::Widget {
        let (nets, sel, secure_pass, optional) = {
            let s = self.s.borrow();
            (s.nets.clone(), s.net, s.net_pass.clone(), s.offline_repo)
        };
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let status = label("", &["setup-status"]);
        status.set_widget_name("nstat");
        if nets.is_empty() {
            body.append(&label(t("inet.none"), &["setup-desc"]));
            status.set_label(t("inet.scanning"));
            self.scan_loop(status.clone());
        } else {
            let items: Vec<(String, String, bool)> = nets.iter().map(|n| (n.ssid.clone(), if n.secure { t("net.secured").into() } else { t("net.open").into() }, true)).collect();
            let pw_list = boxed_list();
            let pw = adw::PasswordEntryRow::builder().build();
            pw.set_text(&secure_pass);
            pw.set_activates_default(true);
            pw_list.append(&pw);
            let open_note = label(t("net.open.note"), &["setup-desc"]);
            let upd = {
                let (pw, pwl, on) = (pw.clone(), pw_list.clone(), open_note.clone());
                move |n: &Network| {
                    pw.set_title(&tf("net.pass", &[("ssid", &n.ssid)]));
                    pwl.set_visible(n.secure);
                    on.set_visible(!n.secure);
                }
            };
            if let Some(n) = nets.get(sel) {
                upd(n);
            }
            let (u, nets2) = (self.clone(), nets.clone());
            let list = choice_list(&items, sel, move |i| {
                u.s.borrow_mut().net = i;
                if let Some(n) = nets2.get(i) {
                    upd(n);
                }
            });
            let u = self.clone();
            pw.connect_changed(move |e| u.s.borrow_mut().net_pass = e.text().to_string());
            body.append(&scrolled(&list, 160, 300));
            body.append(&pw_list);
            body.append(&open_note);
        }
        body.append(&status);
        let mut acts: Vec<gtk::Widget> = vec![];
        if !nets.is_empty() {
            acts.push(self.next_btn(t("net.connect")).upcast());
        }
        let st = status.clone();
        acts.push(self.link_btn(t("inet.rescan"), move |u| u.rescan(st.clone())).upcast());
        acts.push(self.link_btn(t("net.hiddenlink"), |u| {
            let st = u.s.borrow().step;
            u.go(st, 1)
        }).upcast());
        if optional {
            acts.push(self.link_btn(t("net.nonet"), |u| u.next()).upcast());
        }
        self.page(56, t("net.title"), if optional { t("inet.optional") } else { t("inet.sub2") }, Some(body.upcast()), &acts)
    }

    fn v_hidden(self: &Rc<Self>) -> gtk::Widget {
        let (ssid, hsec, hpass) = {
            let s = self.s.borrow();
            (s.ssid.clone(), s.hsec, s.hpass.clone())
        };
        let list = boxed_list();
        let name = adw::EntryRow::builder().title(t("net.ssid")).text(ssid).build();
        let sec = adw::ComboRow::builder().title(t("net.security")).model(&gtk::StringList::new(&[t("none"), "WPA2", "WPA3"])).selected(hsec as u32).build();
        let pw = adw::PasswordEntryRow::builder().title(t("account.pass")).text(hpass).build();
        pw.set_visible(hsec > 0);
        pw.set_activates_default(true);
        list.append(&name);
        list.append(&sec);
        list.append(&pw);
        let u = self.clone();
        name.connect_changed(move |e| u.s.borrow_mut().ssid = e.text().to_string());
        let (u, p) = (self.clone(), pw.clone());
        sec.connect_selected_notify(move |c| {
            u.s.borrow_mut().hsec = c.selected() as usize;
            p.set_visible(c.selected() > 0);
        });
        let u = self.clone();
        pw.connect_changed(move |e| u.s.borrow_mut().hpass = e.text().to_string());
        let status = label("", &["setup-status"]);
        let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
        b.append(&list);
        b.append(&status);
        let st = status.clone();
        let connect = pill(t("net.connect"), true);
        let u = self.clone();
        connect.connect_clicked(move |btn| u.connect_wifi(true, btn.clone(), st.clone()));
        self.win.set_default_widget(Some(&connect));
        let cancel = self.link_btn(t("cancel"), |u| {
            let st = u.s.borrow().step;
            u.go(st, 0)
        });
        self.page(56, t("net.hiddenlink"), t("net.hidden.sub"), Some(b.upcast()), &[connect.upcast(), cancel.upcast()])
    }

    fn v_dysk(self: &Rc<Self>) -> gtk::Widget {
        let s = self.s.borrow();
        let adv = self.link_btn(t("advanced"), |u| {
            let st = u.s.borrow().step;
            u.go(st, 1)
        });
        if s.disk_loading || s.disks.is_empty() && !self.demo && self.client().is_some() && s.disk_seq <= 1 {
            let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
            let sp = gtk::Spinner::new();
            sp.start();
            sp.set_size_request(32, 32);
            b.append(&sp);
            b.append(&label(t("disk.checking"), &["setup-desc"]));
            drop(s);
            return self.page(56, t("disk.where"), "", Some(b.upcast()), &[]);
        }
        let Some(disk) = s.disk().cloned() else {
            drop(s);
            return self.page(56, t("disk.where"), t("err.nodisk.d"), None, &[self.next_btn(t("err.retry")).upcast()]);
        };
        if disk.windows.is_none() {
            let other = s.disks.iter().any(|d| d.windows.is_some());
            let desc = tf("disk.whole", &[("name", &disk.name)]) + if other { " " } else { "" } + if other { t("disk.whole.keep") } else { "" };
            drop(s);
            return self.page(56, t("disk.where"), "", Some(label(&desc, &["setup-desc"]).upcast()), &[self.next_btn(t("next")).upcast(), adv.upcast()]);
        }
        let locked = s.locked();
        let place = s.place;
        let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
        let items = vec![(format!("{} ({})", t("place.beside"), recommended()), String::new(), !locked), (t("place.replace").to_string(), String::new(), true)];
        drop(s);
        let u = self.clone();
        let list = choice_list(&items, place, move |i| {
            if u.s.borrow().place != i {
                u.s.borrow_mut().place = i;
                u.render();
            }
        });
        body.append(&list);
        if place == 0 && !locked {
            body.append(&self.split_widget());
        } else {
            let d = if locked { format!("{} {}", t("place.replace.d"), t("place.locked")) } else { t("place.replace.d").to_string() };
            body.append(&label(&d, &["setup-desc"]));
        }
        self.page(56, t("disk.where"), "", Some(body.upcast()), &[self.next_btn(t("next")).upcast(), adv.upcast()])
    }

    /// Suwak podziału dysku: Windows z lewej, Shrimp z prawej.
    fn split_widget(self: &Rc<Self>) -> gtk::Widget {
        let (pool, wmin, smin, win) = {
            let s = self.s.borrow();
            (s.pool(), s.win_min(), s.shrimp_min(), s.win_gb)
        };
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let mk = |name: &str| {
            let b = gtk::Box::new(gtk::Orientation::Vertical, 2);
            b.add_css_class("split-label");
            b.append(&gtk::Label::new(Some(name)));
            let v = gtk::Label::new(None);
            v.add_css_class("value");
            b.append(&v);
            (b, v)
        };
        let (wb, wv) = mk("Windows");
        let (sb, sv) = mk("Shrimp");
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, pool as f64, 1.0);
        scale.set_hexpand(true);
        scale.set_draw_value(false);
        scale.set_value(win as f64);
        row.append(&wb);
        row.append(&scale);
        row.append(&sb);
        let upd = {
            let (wv, sv) = (wv.clone(), sv.clone());
            move |w: u64| {
                wv.set_label(&gb(w as f64));
                sv.set_label(&gb(pool.saturating_sub(w) as f64));
            }
        };
        upd(win);
        let u = self.clone();
        scale.connect_value_changed(move |sc| {
            let hi = pool.saturating_sub(smin).max(wmin);
            let w = (sc.value().round() as u64).clamp(wmin, hi);
            if (sc.value() - w as f64).abs() >= 1.0 {
                sc.set_value(w as f64);
            }
            u.s.borrow_mut().win_gb = w;
            upd(w);
        });
        row.upcast()
    }

    fn v_dysk_adv(self: &Rc<Self>) -> gtk::Widget {
        let (items, sel, enc, dp, dp2) = {
            let s = self.s.borrow();
            let items: Vec<(String, String, bool)> = s.disks.iter().map(|d| (d.name.clone(), format!("{} · {}", gb(d.size_gb as f64), if d.windows.is_some() { "Windows" } else { t("disk.empty") }), true)).collect();
            (items, s.disk, s.enc, s.dp.clone(), s.dp2.clone())
        };
        let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
        let hd = gtk::Label::builder().label(t("disk.drive")).xalign(0.0).build();
        hd.add_css_class("heading");
        body.append(&hd);
        let u = self.clone();
        body.append(&choice_list(&items, sel, move |i| {
            let mut s = u.s.borrow_mut();
            s.disk = i;
            s.place = 0;
            s.default_split();
        }));
        let el = boxed_list();
        let sw = adw::SwitchRow::builder().title(t("disk.encrypt")).active(enc).build();
        let p1 = adw::PasswordEntryRow::builder().title(t("disk.pass")).text(dp).build();
        let p2 = adw::PasswordEntryRow::builder().title(t("disk.pass2")).text(dp2).build();
        p1.set_visible(enc);
        p2.set_visible(enc);
        el.append(&sw);
        el.append(&p1);
        el.append(&p2);
        body.append(&el);
        let warn = label(t("disk.enc.warn"), &["setup-desc"]);
        warn.set_visible(enc);
        body.append(&warn);
        let (u, a, b, w) = (self.clone(), p1.clone(), p2.clone(), warn.clone());
        sw.connect_active_notify(move |sw| {
            u.s.borrow_mut().enc = sw.is_active();
            a.set_visible(sw.is_active());
            b.set_visible(sw.is_active());
            w.set_visible(sw.is_active());
        });
        let u = self.clone();
        p1.connect_changed(move |e| u.s.borrow_mut().dp = e.text().to_string());
        let u = self.clone();
        p2.connect_changed(move |e| u.s.borrow_mut().dp2 = e.text().to_string());
        let done = pill(t("done"), true);
        let u = self.clone();
        done.connect_clicked(move |_| {
            let (enc, ok, empty, step) = {
                let s = u.s.borrow();
                (s.enc, s.dp == s.dp2, s.dp.is_empty(), s.step)
            };
            if enc && empty {
                return;
            }
            if enc && !ok {
                return u.err(ErrKey::Dpass, "");
            }
            u.go(step, 0);
        });
        self.win.set_default_widget(Some(&done));
        self.page(56, t("advanced"), "", Some(body.upcast()), &[done.upcast()])
    }

    fn v_konto(self: &Rc<Self>) -> gtk::Widget {
        let (full, user, pass, pass2) = {
            let s = self.s.borrow();
            (s.full.clone(), s.user.clone(), s.pass.clone(), s.pass2.clone())
        };
        let list = boxed_list();
        let f = adw::EntryRow::builder().title(t("account.full")).text(full).build();
        let us = adw::EntryRow::builder().title(t("account.user")).text(user).build();
        let p1 = adw::PasswordEntryRow::builder().title(t("account.pass")).text(pass).build();
        let p2 = adw::PasswordEntryRow::builder().title(t("account.pass2")).text(pass2).build();
        p2.set_activates_default(true);
        for w in [f.upcast_ref::<gtk::Widget>(), us.upcast_ref(), p1.upcast_ref(), p2.upcast_ref()] {
            list.append(w);
        }
        let next = self.next_btn(t("next"));
        let sync = {
            let (u, n) = (self.clone(), next.clone());
            move || {
                let s = u.s.borrow();
                n.set_sensitive(!s.full.trim().is_empty() && !s.user.is_empty() && !s.pass.is_empty() && !s.pass2.is_empty());
            }
        };
        sync();
        let sync = Rc::new(sync);
        let (u, us2, sy) = (self.clone(), us.clone(), sync.clone());
        f.connect_changed(move |e| {
            let auto = {
                let mut s = u.s.borrow_mut();
                s.full = e.text().to_string();
                (!s.user_edited).then(|| logic::login_from(&s.full, ""))
            };
            if let Some(login) = auto {
                u.s.borrow_mut().user = login.clone();
                us2.set_text(&login);
                u.s.borrow_mut().user_edited = false;
            }
            sy();
        });
        let (u, sy) = (self.clone(), sync.clone());
        us.connect_changed(move |e| {
            let clean = logic::slug(&e.text());
            if clean != e.text().as_str() {
                e.set_text(&clean);
                return;
            }
            let mut s = u.s.borrow_mut();
            if s.user != clean {
                s.user = clean;
                s.user_edited = true;
            }
            drop(s);
            sy();
        });
        let (u, sy) = (self.clone(), sync.clone());
        p1.connect_changed(move |e| {
            u.s.borrow_mut().pass = e.text().to_string();
            sy();
        });
        let (u, sy) = (self.clone(), sync);
        p2.connect_changed(move |e| {
            u.s.borrow_mut().pass2 = e.text().to_string();
            sy();
        });
        let fw = f.clone();
        glib::idle_add_local_once(move || {
            fw.grab_focus();
        });
        self.page(56, t("account.title"), t("account.sub"), Some(list.upcast()), &[next.upcast()])
    }

    fn v_pliki(self: &Rc<Self>) -> gtk::Widget {
        let (folders, bytes, into) = {
            let s = self.s.borrow();
            let t = s.tr.clone().unwrap_or_default();
            (t.folders, t.bytes, s.files_into)
        };
        let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
        if !folders.is_empty() {
            let fl = boxed_list();
            for (key, _, b) in &folders {
                let (icon, name) = match key.as_str() {
                    "documents" => ("folder-documents-symbolic", t("folder.documents")),
                    "pictures" => ("folder-pictures-symbolic", t("folder.pictures")),
                    "desktop" => ("user-desktop-symbolic", t("folder.desktop")),
                    "music" => ("folder-music-symbolic", t("folder.music")),
                    "videos" => ("folder-videos-symbolic", t("folder.videos")),
                    "downloads" => ("folder-download-symbolic", t("folder.downloads")),
                    _ => ("folder-symbolic", t("folder.other")),
                };
                let row = adw::ActionRow::builder().title(name).subtitle(gb(*b as f64 / 1e9)).build();
                row.add_prefix(&gtk::Image::from_icon_name(icon));
                fl.append(&row);
            }
            body.append(&fl);
        }
        let desc = label(t(if into { "files.move.d" } else { "files.keep.d" }), &["setup-desc"]);
        let (u, d) = (self.clone(), desc.clone());
        body.append(&choice_list(&[(format!("{} ({})", t("files.move"), recommended()), String::new(), true), (t("files.keep").into(), String::new(), true)], if into { 0 } else { 1 }, move |i| {
            u.s.borrow_mut().files_into = i == 0;
            d.set_label(t(if i == 0 { "files.move.d" } else { "files.keep.d" }));
        }));
        body.append(&desc);
        let sub = t("files.sub").replace(" {files} ", " ").replace("{files} ", "").replace("{size}", &gb(bytes as f64 / 1e9));
        let sub = if folders.iter().any(|f| f.1 > 0) { tf("files.sub", &[("files", &folders.iter().map(|f| f.1).sum::<u64>().to_string()), ("size", &gb(bytes as f64 / 1e9))]) } else { sub };
        self.page(56, t("files.title"), &sub, Some(body.upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn v_pendrive(self: &Rc<Self>) -> gtk::Widget {
        let ((files, bytes), restore) = {
            let s = self.s.borrow();
            (s.usb.unwrap_or_default(), s.usb_restore)
        };
        let desc = label(t(if restore { "usb.choice.restore.d" } else { "usb.choice.keep.d" }), &["setup-desc"]);
        let (u, d) = (self.clone(), desc.clone());
        let list = choice_list(&[(format!("{} ({})", t("usb.choice.restore"), recommended()), String::new(), true), (t("usb.choice.keep").into(), String::new(), true)], if restore { 0 } else { 1 }, move |i| {
            u.s.borrow_mut().usb_restore = i == 0;
            d.set_label(t(if i == 0 { "usb.choice.restore.d" } else { "usb.choice.keep.d" }));
        });
        let b = gtk::Box::new(gtk::Orientation::Vertical, 12);
        b.append(&list);
        b.append(&desc);
        let sub = tf("usb.choice.sub", &[("files", &files.to_string()), ("size", &gb(bytes as f64 / 1e9))]);
        self.page(56, t("usb.choice.title"), &sub, Some(b.upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn v_przegladarka(self: &Rc<Self>) -> gtk::Widget {
        let (list_b, wb, sel) = {
            let s = self.s.borrow();
            (s.browsers(), s.win_browser(), s.browser)
        };
        let items: Vec<(String, String, bool)> = list_b
            .iter()
            .enumerate()
            .map(|(j, b)| {
                let meta = if Some(j) == wb { t("browser.windows").to_string() } else if j == 0 && wb.is_none() { recommended() } else { String::new() };
                (b.name.to_string(), meta, true)
            })
            .collect();
        let u = self.clone();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.append(&choice_list(&items, sel, move |i| u.s.borrow_mut().browser = i));
        if let Some(w) = wb {
            body.append(&label(&tf("browser.windows.note", &[("name", list_b[w].name)]), &["setup-desc"]));
        }
        self.page(56, t("browser.title"), t("browser.sub"), Some(body.upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn v_programy(self: &Rc<Self>) -> gtk::Widget {
        let (apps, none, sel) = {
            let s = self.s.borrow();
            (s.win_list(), s.no_alt.clone(), s.sel.clone())
        };
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let list = boxed_list();
        let base = adw::SwitchRow::builder().title(t("prog.base")).subtitle(t("prog.base.d")).active(true).sensitive(false).build();
        list.append(&base);
        for a in &apps {
            let sub = if a.windows == a.name { String::new() } else { tf("prog.instead", &[("name", &a.windows)]) };
            let row = adw::SwitchRow::builder().title(glib::markup_escape_text(&a.name).as_str()).active(sel.get(&a.windows) != Some(&false)).build();
            if !sub.is_empty() {
                row.set_subtitle(glib::markup_escape_text(&sub).as_str());
            }
            let (u, key) = (self.clone(), a.windows.clone());
            row.connect_active_notify(move |r| {
                u.s.borrow_mut().sel.insert(key.clone(), r.is_active());
            });
            list.append(&row);
        }
        body.append(&list);
        body.append(&label(t("prog.after"), &["setup-desc"]));
        if !none.is_empty() {
            let h = gtk::Label::builder().label(t("prog.none.title")).xalign(0.0).build();
            h.add_css_class("heading");
            h.set_margin_top(10);
            body.append(&h);
            body.append(&gtk::Label::builder().label(t("prog.none.sub")).xalign(0.0).wrap(true).css_classes(["setup-desc"]).build());
            let nl = boxed_list();
            for n in none.iter().take(8) {
                let r = adw::ActionRow::builder().title(glib::markup_escape_text(n).as_str()).build();
                r.add_prefix(&gtk::Image::from_icon_name("dialog-warning-symbolic"));
                nl.append(&r);
            }
            if none.len() > 8 {
                nl.append(&adw::ActionRow::builder().title(tf("prog.more", &[("n", &(none.len() - 8).to_string())])).build());
            }
            body.append(&nl);
        }
        let has = !apps.is_empty();
        self.page(56, t(if has { "win.title" } else { "prog.title" }), t(if has { "win.sub" } else { "prog.nowin" }), Some(body.upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn apply_look(&self) {
        let s = self.s.borrow();
        adw::StyleManager::default().set_color_scheme(if s.dark { adw::ColorScheme::ForceDark } else { adw::ColorScheme::ForceLight });
        let css = match s.accent {
            Some(i) => format!("@define-color accent_bg_color {c}; @define-color accent_color {c}; @define-color accent_fg_color #ffffff;", c = ACCENTS[i].2),
            None => String::new(),
        };
        self.accent_css.load_from_string(&css);
    }

    fn v_wyglad(self: &Rc<Self>) -> gtk::Widget {
        let (dark, accent, scale, night, auto) = {
            let s = self.s.borrow();
            (s.dark, s.accent, s.text_scale, s.night, s.auto)
        };
        let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
        // Jasny / ciemny: dwa kafelki, podgląd od razu w oknie Setup.
        let cards = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        cards.set_halign(gtk::Align::Center);
        let light_b = gtk::ToggleButton::new();
        let dark_b = gtk::ToggleButton::new();
        for (b, name, icon, cls) in [(&light_b, t("look.light"), "weather-clear-symbolic", "light"), (&dark_b, t("look.dark"), "weather-clear-night-symbolic", "dark")] {
            let c = gtk::Box::new(gtk::Orientation::Vertical, 8);
            c.add_css_class("look-card");
            c.add_css_class(cls);
            let i = gtk::Image::from_icon_name(icon);
            i.set_pixel_size(32);
            c.append(&i);
            c.append(&gtk::Label::new(Some(name)));
            b.set_child(Some(&c));
            b.add_css_class("flat");
            cards.append(b);
        }
        dark_b.set_group(Some(&light_b));
        if dark { dark_b.set_active(true) } else { light_b.set_active(true) }
        let u = self.clone();
        dark_b.connect_toggled(move |b| {
            u.s.borrow_mut().dark = b.is_active();
            u.apply_look();
        });
        body.append(&cards);
        let h = gtk::Label::builder().label(t("look.color")).build();
        h.add_css_class("heading");
        body.append(&h);
        let sw = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        sw.set_halign(gtk::Align::Center);
        let accname = label(accent.map(|i| t(ACCENTS[i].0)).unwrap_or(""), &["setup-desc"]);
        let mut first: Option<gtk::ToggleButton> = None;
        for (i, (key, name, _)) in ACCENTS.iter().enumerate() {
            let b = gtk::ToggleButton::new();
            b.add_css_class("swatch");
            b.add_css_class(name);
            b.set_tooltip_text(Some(t(key)));
            b.set_active(accent == Some(i));
            if let Some(f) = &first {
                b.set_group(Some(f));
            } else {
                first = Some(b.clone());
            }
            let (u, an) = (self.clone(), accname.clone());
            b.connect_toggled(move |b| {
                if b.is_active() {
                    u.s.borrow_mut().accent = Some(i);
                    an.set_label(t(ACCENTS[i].0));
                    u.apply_look();
                }
            });
            sw.append(&b);
        }
        body.append(&sw);
        body.append(&accname);
        let list = boxed_list();
        let ts = adw::ActionRow::builder().title(t("look.text")).build();
        let sc = gtk::Scale::with_range(gtk::Orientation::Horizontal, 80.0, 160.0, 10.0);
        sc.set_value(scale * 100.0);
        sc.set_size_request(220, -1);
        sc.set_valign(gtk::Align::Center);
        sc.set_value_pos(gtk::PositionType::Right);
        sc.set_format_value_func(|_, v| format!("{v:.0}%"));
        for m in [100.0] {
            sc.add_mark(m, gtk::PositionType::Bottom, None);
        }
        ts.add_suffix(&sc);
        let u = self.clone();
        sc.connect_value_changed(move |s| {
            let v = (s.value() / 10.0).round() * 10.0;
            u.s.borrow_mut().text_scale = v / 100.0;
        });
        list.append(&ts);
        let n = adw::SwitchRow::builder().title(t("look.night")).subtitle(t("look.night.d")).active(night).build();
        let u = self.clone();
        n.connect_active_notify(move |r| u.s.borrow_mut().night = r.is_active());
        list.append(&n);
        let a = adw::SwitchRow::builder().title(t("look.auto")).subtitle(t("look.auto.d")).active(auto).build();
        let u = self.clone();
        a.connect_active_notify(move |r| u.s.borrow_mut().auto = r.is_active());
        list.append(&a);
        body.append(&list);
        self.page(0, t("look.title"), t("look.note"), Some(body.upcast()), &[self.next_btn(t("next")).upcast()])
    }

    fn v_podsumowanie(self: &Rc<Self>) -> gtk::Widget {
        let s = self.s.borrow();
        let loc = s.locale();
        let mut rows: Vec<(&str, String)> = vec![
            (t("sum.lang"), s.langs.get(s.lang).map(|l| l.name.clone()).unwrap_or_default()),
            (t("sum.region"), s.regs.get(s.region).map(|r| names::region_label(r, &loc)).unwrap_or_default()),
            (t("sum.kb"), s.kbs.get(s.kb).map(|k| names::keyboard_label(k, &loc)).unwrap_or_default()),
            (t("sum.disk"), s.disk().map(|d| d.name.clone()).unwrap_or_default()),
            (
                t("sum.how"),
                match s.place_kind() {
                    Place::Whole => t("sum.whole").into(),
                    Place::Beside => t("place.beside").into(),
                    Place::Replace => t("place.replace").into(),
                },
            ),
        ];
        if s.beside() {
            rows.push((t("sum.split"), tf("sum.split.v", &[("win", &gb(s.win_gb as f64)), ("shrimp", &gb(s.pool().saturating_sub(s.win_gb) as f64))])));
        }
        rows.push((t("sum.account"), format!("{} ({})", s.full.trim(), s.user)));
        if s.enc {
            rows.push((t("sum.enc"), t("enc.on").into()));
        }
        if s.files() > 0 {
            rows.push((t("sum.files"), t(if s.files_into { "files.move" } else { "files.keep" }).into()));
        }
        if s.usb.is_some() {
            rows.push((t("sum.usb"), t(if s.usb_restore { "usb.choice.restore" } else { "usb.choice.keep" }).into()));
        }
        rows.push((t("sum.browser"), s.browsers().get(s.browser).map(|b| b.name.to_string()).unwrap_or_default()));
        let n = s.win_list().iter().filter(|a| s.sel.get(&a.windows) != Some(&false)).count();
        rows.push((t("sum.apps"), if n > 0 { format!("{} + {n}", t("prog.base")) } else { t("prog.base").into() }));
        rows.push((t("sum.look"), format!("{}{}", t(if s.dark { "look.dark" } else { "look.light" }), s.accent.map(|i| format!(", {}", t(ACCENTS[i].0))).unwrap_or_default())));
        drop(s);
        let list = boxed_list();
        for (k, v) in rows {
            let r = adw::ActionRow::builder().title(k).subtitle(glib::markup_escape_text(&v).as_str()).build();
            r.add_css_class("property");
            list.append(&r);
        }
        self.page(56, t("sum.title"), t("sum.sub"), Some(scrolled(&list, 200, 400).upcast()), &[self.next_btn(t("sum.go")).upcast()])
    }

    fn v_instalacja(self: &Rc<Self>) -> gtk::Widget {
        let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let bar = gtk::ProgressBar::new();
        bar.set_margin_top(12);
        let cap = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let phase = gtk::Label::builder().label(t("inst.disk")).xalign(0.0).hexpand(true).build();
        phase.add_css_class("heading");
        let eta = gtk::Label::builder().label("").xalign(1.0).build();
        eta.add_css_class("setup-desc");
        cap.append(&phase);
        cap.append(&eta);
        body.append(&bar);
        body.append(&cap);
        // Slajdy w czasie instalacji: to, co dawniej pokazywał koniec pierwszej konfiguracji.
        let slides = data::slides(self.s.borrow().beside());
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.set_margin_top(36);
        let st = label("", &["slide-title"]);
        let sd = label("", &["setup-desc"]);
        card.append(&st);
        card.append(&sd);
        body.append(&card);
        let beside = self.s.borrow().beside();
        let show = move |i: usize| {
            let s = slides[i % slides.len()];
            st.set_label(t(s.t));
            let extra = if beside { s.extra.map(|e| format!(" {}", t(e))).unwrap_or_default() } else { String::new() };
            sd.set_label(&format!("{}{extra}", t(s.d)));
        };
        show(0);
        let k = Rc::new(Cell::new(0usize));
        self.timer(Duration::from_secs(12), move |_| {
            k.set(k.get() + 1);
            show(k.get());
            true
        });
        let progress = self.s.borrow().progress.clone();
        self.timer(Duration::from_millis(400), move |_| {
            let (p, ph, e) = *progress.lock().unwrap();
            bar.set_fraction(p / 100.0);
            phase.set_label(t(if ph == "done" { "inst.files" } else { ph }));
            eta.set_label(&match e {
                Some(m) if m <= 1 => t("eta.soon").to_string(),
                Some(m) => tf("eta.left", &[("min", &m.to_string())]),
                None => String::new(),
            });
            true
        });
        self.page(80, t("inst.title"), "", Some(body.upcast()), &[])
    }

    fn v_restart(self: &Rc<Self>) -> gtk::Widget {
        self.page(128, t("restart.title"), t("restart.sub"), None, &[self.next_btn(t("restart.go")).upcast()])
    }

    // ---------- sieć ----------

    /// Brak sieci na liście: szukamy dalej i mówimy, czego brakuje (karta, radio, zasięg).
    fn scan_loop(self: &Rc<Self>, status: gtk::Label) {
        let my = self.seq.get() + 1;
        let u = self.clone();
        bg(system::wifi_diag, {
            let (u, st) = (u.clone(), status.clone());
            move |d| {
                eprintln!("shrimp-setup: Wi-Fi, brak sieci: {}", d.summary());
                if u.seq.get() != my {
                    return;
                }
                st.set_label(&if d.devices.is_empty() {
                    format!("{}{}", t("inet.nodev"), if d.hardware.is_empty() { String::new() } else { format!(" ({})", d.hardware.chars().take(120).collect::<String>()) })
                } else if !d.blocked.is_empty() || d.radio == "disabled" {
                    t("inet.off").to_string()
                } else {
                    t("inet.scanning").to_string()
                });
            }
        });
        fn again(u: Rc<Ui>, my: u64) {
            bg(system::networks, move |n| {
                if u.seq.get() != my || u.s.borrow().cur() != Internet {
                    return;
                }
                if !n.is_empty() {
                    u.s.borrow_mut().nets = n;
                    u.s.borrow_mut().net = 0;
                    u.render();
                } else {
                    glib::timeout_add_local_once(Duration::from_millis(1500), move || again(u, my));
                }
            });
        }
        again(u, my);
    }

    fn rescan(self: &Rc<Self>, status: gtk::Label) {
        status.set_label(t("inet.scanning"));
        let u = self.clone();
        bg(system::networks, move |n| {
            let was = u.s.borrow().nets.get(u.s.borrow().net).map(|x| x.ssid.clone());
            {
                let mut s = u.s.borrow_mut();
                s.net = was.and_then(|w| n.iter().position(|x| x.ssid == w)).unwrap_or(0);
                s.nets = n;
            }
            u.render();
        });
    }

    fn connect_wifi(self: &Rc<Self>, hidden: bool, btn: gtk::Button, status: gtk::Label) {
        if self.s.borrow().busy {
            return;
        }
        let c = {
            let s = self.s.borrow();
            if hidden {
                if s.ssid.trim().is_empty() || s.hsec > 0 && s.hpass.is_empty() {
                    return;
                }
                system::Connect { ssid: s.ssid.trim().to_string(), password: (s.hsec > 0).then(|| s.hpass.clone()), hidden: true, security: Some(["none", "wpa2", "wpa3"][s.hsec].into()) }
            } else {
                let Some(n) = s.nets.get(s.net) else { return };
                if n.secure && s.net_pass.is_empty() {
                    return;
                }
                system::Connect { ssid: n.ssid.clone(), password: n.secure.then(|| s.net_pass.clone()), hidden: false, security: None }
            }
        };
        self.s.borrow_mut().busy = true;
        btn.set_sensitive(false);
        status.set_label(&tf("inet.connecting", &[("ssid", &c.ssid)]));
        let (u, ssid) = (self.clone(), c.ssid.clone());
        let client = self.client();
        let demo = self.demo;
        bg(
            move || -> Result<String, (ErrKey, String)> {
                if demo {
                    std::thread::sleep(Duration::from_secs(1));
                    return Ok(String::new());
                }
                system::connect(&c).map_err(|e| (ErrKey::Wifi, e.0))?;
                // Internet po połączeniu z Wi-Fi (do 60 s), potem Agama czyta repozytoria od nowa.
                let mut ok = false;
                for _ in 0..30 {
                    if system::online() {
                        ok = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
                eprintln!("shrimp-setup: internet po połączeniu z Wi-Fi: {}", if ok { "jest" } else { "brak" });
                if !ok {
                    return Err((ErrKey::Offline, t("err.offline.nonet").into()));
                }
                let issue = client.map(|c| c.reprobe()).unwrap_or_default();
                if !issue.is_empty() {
                    return Err((ErrKey::Repo, issue.chars().take(160).collect()));
                }
                Ok(ssid)
            },
            move |r| {
                u.s.borrow_mut().busy = false;
                btn.set_sensitive(true);
                status.set_label("");
                match r {
                    Ok(_) => {
                        u.s.borrow_mut().online = true;
                        let n = idx(Internet) + 1;
                        u.go(n, 0);
                    }
                    Err((k, why)) => {
                        eprintln!("shrimp-setup: Wi-Fi nie połączone: {why}");
                        u.err(k, &why);
                    }
                }
            },
        );
    }

    // ---------- przejścia ----------

    fn act_next(self: &Rc<Self>) {
        if self.s.borrow().busy {
            return;
        }
        let cur = self.s.borrow().cur();
        match cur {
            Jezyk => {
                let locale = self.s.borrow().locale();
                let c = self.client();
                let l2 = locale.clone();
                bg(move || if let Some(c) = c { c.set_language(&l2) }, |_| {});
                {
                    let mut s = self.s.borrow_mut();
                    Self::lang_defaults(&mut s);
                }
                let kb = { let s = self.s.borrow(); s.kbs.get(s.kb).map(|k| k.keymap) };
                if let Some(k) = kb {
                    self.set_keymap(k);
                }
                self.next();
            }
            Internet => {
                // Przycisk „Połącz”: znaleźć etykietę stanu w bieżącym widoku.
                let status = self.find_status();
                if let Some(btn) = self.win.default_widget().and_then(|w| w.downcast::<gtk::Button>().ok()) {
                    self.connect_wifi(false, btn, status);
                }
            }
            Dysk => {
                let (none, space) = {
                    let s = self.s.borrow();
                    let free = if s.beside() { s.pool().saturating_sub(s.win_gb) } else { s.disk().map(|d| d.size_gb).unwrap_or(0) };
                    (s.disk().is_none(), free < s.shrimp_min())
                };
                if none {
                    self.load_disks(true);
                    return self.render();
                }
                if space {
                    return self.err(ErrKey::Space, "");
                }
                self.next();
            }
            Konto => {
                let (same, reserved) = {
                    let s = self.s.borrow();
                    (s.pass == s.pass2, logic::is_reserved_user(&s.user))
                };
                if !same {
                    return self.err(ErrKey::Pass, "");
                }
                if reserved {
                    return self.err(ErrKey::User, "");
                }
                self.next();
            }
            Podsumowanie => {
                let Some(plan) = self.s.borrow().plan() else { return self.err(ErrKey::NoDisk, "") };
                let c = self.client();
                self.s.borrow_mut().busy = true;
                if let Some(b) = self.win.default_widget() {
                    b.set_sensitive(false);
                }
                let u = self.clone();
                bg(move || c.map(|c| c.check(&plan)).unwrap_or(Ok(())), move |r| {
                    u.s.borrow_mut().busy = false;
                    match r {
                        Ok(()) => {
                            u.next();
                            u.run_install(false);
                        }
                        Err(e) => u.install_err(e),
                    }
                });
            }
            Restart => {
                let c = self.client();
                if let Some(b) = self.win.default_widget() {
                    b.set_sensitive(false);
                }
                let u = self.clone();
                bg(move || c.map(|c| c.reboot()).unwrap_or(Ok(())), move |r| match r {
                    Ok(()) if u.demo => u.win.application().map(|a| a.quit()).unwrap_or(()),
                    Ok(()) => {}
                    Err(e) => u.err(ErrKey::Install, &e),
                });
            }
            _ => self.next(),
        }
    }

    fn find_status(&self) -> gtk::Label {
        fn walk(w: &gtk::Widget) -> Option<gtk::Label> {
            if w.widget_name() == "nstat" {
                return w.clone().downcast::<gtk::Label>().ok();
            }
            let mut c = w.first_child();
            while let Some(ch) = c {
                if let Some(l) = walk(&ch) {
                    return Some(l);
                }
                c = ch.next_sibling();
            }
            None
        }
        self.stack.visible_child().and_then(|v| walk(&v)).unwrap_or_else(|| gtk::Label::new(None))
    }

    fn run_install(self: &Rc<Self>, watch_only: bool) {
        let plan = self.s.borrow().plan();
        let c = self.client();
        let progress = self.s.borrow().progress.clone();
        let demo = self.demo;
        let u = self.clone();
        bg(
            move || -> Result<(), InstallError> {
                let set = |p: f64, ph: &'static str, e: Option<u64>| *progress.lock().unwrap() = (p, ph, e);
                match c {
                    Some(c) => {
                        if !watch_only {
                            c.start_install(plan.as_ref().unwrap())?;
                        }
                        c.watch(set)
                    }
                    None if demo => {
                        for i in 0..=100 {
                            std::thread::sleep(Duration::from_millis(120));
                            set(i as f64, if i < 20 { "inst.disk" } else if i < 85 { "inst.system" } else { "inst.files" }, Some(((100 - i) / 9) as u64));
                        }
                        Ok(())
                    }
                    None => Err(InstallError::Agama("brak Agamy".into())),
                }
            },
            move |r| match r {
                Ok(()) => {
                    let n = idx(Restart);
                    u.go(n, 0);
                }
                Err(e) => u.install_err(e),
            },
        );
    }

    fn install_err(self: &Rc<Self>, e: InstallError) {
        match e {
            InstallError::Space(m) => self.err(ErrKey::Space, &m),
            InstallError::Install(m) => self.err(ErrKey::Install, &m),
            InstallError::Stall => self.err(ErrKey::Stall, ""),
            InstallError::Agama(m) => self.err(ErrKey::Agama, &m),
        }
    }

    // ---------- okna błędów ----------

    fn err(self: &Rc<Self>, k: ErrKey, detail: &str) {
        let (tk, dk, pk, sk) = data::err_texts(k);
        let mut body = t(dk).to_string();
        if k == ErrKey::Wifi {
            if let Some(n) = { let s = self.s.borrow(); (s.sub == 0).then(|| s.nets.get(s.net).map(|n| n.ssid.clone())).flatten() } {
                body = tf("err.wifi.ssid", &[("ssid", &n)]);
            }
        }
        if !detail.is_empty() && k == ErrKey::Offline {
            body = detail.to_string();
        } else if !detail.is_empty() {
            body += &format!(" ({})", detail.chars().take(220).collect::<String>());
        }
        let d = adw::AlertDialog::new(Some(t(tk)), Some(&body));
        d.add_response("primary", t(pk));
        d.set_response_appearance("primary", adw::ResponseAppearance::Suggested);
        d.set_default_response(Some("primary"));
        if let Some(s) = sk {
            d.add_response("secondary", t(s));
        }
        d.set_close_response(if sk.is_some() { "secondary" } else { "primary" });
        let u = self.clone();
        d.connect_response(None, move |_, r| u.err_response(k, r == "primary"));
        d.present(Some(&self.win));
    }

    fn err_response(self: &Rc<Self>, k: ErrKey, primary: bool) {
        let step = self.s.borrow().step;
        match (k, primary) {
            (ErrKey::Pass, _) => {
                self.s.borrow_mut().pass2.clear();
                self.render();
            }
            (ErrKey::Dpass, _) => {
                self.s.borrow_mut().dp2.clear();
                self.render();
            }
            (ErrKey::Space, true) => {
                self.s.borrow_mut().default_split();
                self.go(idx(Dysk), 0);
            }
            (ErrKey::Space, false) => self.go(idx(Dysk), 0),
            (ErrKey::NoDisk, _) => self.go(idx(Dysk), 0),
            (ErrKey::Install, true) => self.go(step, 0),
            (ErrKey::Install | ErrKey::Stall | ErrKey::Agama, false) => self.go(0, 0),
            (ErrKey::Stall | ErrKey::Agama, true) => self.run_install(true),
            (ErrKey::User, _) => {
                self.s.borrow_mut().user_edited = true;
                self.render();
            }
            (ErrKey::Wifi, _) => {
                {
                    let mut s = self.s.borrow_mut();
                    if s.sub == 0 {
                        s.net_pass.clear();
                    } else {
                        s.hpass.clear();
                    }
                }
                self.render();
            }
            (ErrKey::Offline | ErrKey::Repo, _) => {
                self.s.borrow_mut().online = false;
                self.go(idx(Internet), 0);
            }
        }
    }
}
