//! Lokalny serwer pierwszej konfiguracji: serwuje frontend (setup/web/dist) i /shrimp/api/*
//! tylko na 127.0.0.1, z losowym kluczem sesji w nagłówku X-Shrimp-Key.
use crate::system;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

/// Ustawiane po „Zaczynajmy”, gdy okno ma się zamknąć (main.rs sprawdza je co 300 ms).
pub static QUIT: AtomicBool = AtomicBool::new(false);

#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Job {
    state: &'static str,
    percent: f64,
    phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    eta_min: Option<u64>,
    /// Programy po kolei (onboarding pokazuje każdy w osobnym wierszu): "wait", "run", "done", "fail", "later".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    items: Vec<JobItem>,
}

#[derive(Clone, Serialize)]
struct JobItem {
    id: String,
    state: &'static str,
}

impl Job {
    fn mark(&mut self, id: &str, state: &'static str) {
        if let Some(i) = self.items.iter_mut().find(|i| i.id == id) {
            i.state = state;
        }
    }
    fn mark_rest(&mut self, state: &'static str) {
        for i in self.items.iter_mut().filter(|i| i.state == "wait" || i.state == "run") {
            i.state = state;
        }
    }
}

#[derive(Default)]
struct Inner {
    codecs: bool,
    nvidia: String,
    apps: Job,
    usb: Job,
    /// Programy wybrane bez internetu: shrimp-setup-apply zapisuje je na później (shrimp-setup-later).
    later_apps: Vec<String>,
    /// Automatyczne aktualizacje w tle (Cocktail), domyślnie włączone.
    auto_updates: bool,
}

#[derive(Clone)]
struct App {
    key: Arc<String>,
    inner: Arc<Mutex<Inner>>,
}

/// Uruchamia serwer w osobnym wątku. Zwraca port i klucz sesji.
pub fn spawn(ui_dir: PathBuf) -> std::io::Result<(u16, String)> {
    let key: String = {
        use rand::RngExt;
        let mut r = rand::rng();
        (0..32).map(|_| format!("{:x}", r.random_range(0..16u8))).collect()
    };
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    std_listener.set_nonblocking(true)?;
    let port = std_listener.local_addr()?.port();
    let state = App {
        key: Arc::new(key.clone()),
        inner: Arc::new(Mutex::new(Inner { codecs: false, nvidia: "none".into(), apps: Job { state: "idle", ..Default::default() }, usb: Job { state: "idle", ..Default::default() }, later_apps: Vec::new(), auto_updates: true })),
    };
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio");
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(std_listener).expect("listener");
            axum::serve(listener, router(state, ui_dir)).await.expect("serve");
        });
    });
    Ok((port, key))
}

fn router(state: App, ui_dir: PathBuf) -> Router {
    let api = Router::new()
        .route("/info", get(info))
        .route("/online", get(online))
        .route("/networks", get(networks))
        .route("/network/diag", get(wifi_diag))
        .route("/network/connect", post(connect))
        .route("/network/advanced", post(advanced))
        .route("/appearance", post(appearance))
        .route("/keymap", post(keymap))
        .route("/drivers", post(drivers))
        .route("/browser", post(browser))
        .route("/prefs", post(prefs))
        .route("/apps", post(apps_start).get(apps_status))
        .route("/usb", get(usb_get).post(usb_start))
        .route("/files", get(files_get).post(files_place))
        .route("/finish", post(finish))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(middleware::from_fn(cors))
        .with_state(state);
    Router::new().nest("/shrimp/api", api).fallback_service(ServeDir::new(ui_dir))
}

async fn auth(State(s): State<App>, req: Request, next: Next) -> Response {
    let ok = req.headers().get("x-shrimp-key").and_then(|v| v.to_str().ok()) == Some(s.key.as_str());
    if ok { next.run(req).await } else { StatusCode::FORBIDDEN.into_response() }
}

/// Na nośniku stronę Setup serwuje Agama (http://127.0.0.1), a sieć Wi-Fi ustawia ten serwer
/// (inny port), więc strona Agamy może go wołać: tylko z tego jednego adresu, dalej z kluczem sesji.
const AGAMA_ORIGIN: &str = "http://127.0.0.1";

async fn cors(req: Request, next: Next) -> Response {
    use axum::http::{header, HeaderValue, Method};
    let from_agama = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()) == Some(AGAMA_ORIGIN);
    let mut res = if req.method() == Method::OPTIONS && from_agama {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(req).await
    };
    if from_agama {
        let h = res.headers_mut();
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static(AGAMA_ORIGIN));
        h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("x-shrimp-key, content-type"));
        h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET, POST"));
    }
    res
}

fn err(kind: &str, msg: impl ToString) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": kind, "message": msg.to_string() }))).into_response()
}

async fn info() -> Json<system::Info> { Json(system::info().await) }
async fn networks() -> Json<Vec<system::Network>> { Json(system::networks().await) }
async fn wifi_diag() -> Json<system::WifiDiag> { Json(system::wifi_diag().await) }
/// Na nośniku z repozytorium pakietów (shrimp-offline-repo) system instaluje się bez internetu,
/// więc krok „Połącz z internetem” jest zbędny.
async fn online() -> Json<serde_json::Value> { Json(json!({ "online": system::offline_repo() || system::online().await })) }

async fn connect(Json(c): Json<system::Connect>) -> Response {
    match system::connect(&c).await {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => err("wifi", e.0),
    }
}

#[derive(Deserialize)]
struct Keymap { keymap: String }

async fn keymap(Json(k): Json<Keymap>) -> Response {
    match system::input_source(&k.keymap).await {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => err("system", e.0),
    }
}

async fn advanced(Json(a): Json<system::Advanced>) -> Response {
    match system::advanced(&a).await {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => err("system", e.0),
    }
}

#[derive(Deserialize)]
struct Appearance { scheme: String, accent: Option<String> }
async fn appearance(Json(a): Json<Appearance>) -> Response {
    match system::appearance(&a.scheme, a.accent.as_deref()).await {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => err("system", e.0),
    }
}

#[derive(Deserialize)]
struct Drivers { install: bool, nvidia: Option<String> }
/// Wybór jest tylko zapamiętywany; instalacja idzie w jednej migawce przy „Zaczynajmy”.
async fn drivers(State(s): State<App>, Json(d): Json<Drivers>) -> Json<serde_json::Value> {
    let mut i = s.inner.lock().await;
    i.codecs = d.install;
    i.nvidia = match (d.install, d.nvidia.as_deref()) {
        (true, Some("nvidia")) => "nvidia",
        (true, Some("open")) => "open",
        _ => "none",
    }
    .into();
    Json(json!({}))
}

#[derive(Deserialize)]
struct Apps {
    ids: Vec<String>,
    /// Przeglądarka z kroku „Przeglądarka”: instaluje się pierwsza.
    #[serde(default)]
    browser: Option<String>,
}

/// Wiersz zestawu podstawowego (aplikacje GNOME, LibreOffice, Steam) na liście postępu.
const BASE: &str = "base";

async fn apps_start(State(s): State<App>, Json(a): Json<Apps>) -> Response {
    // Kolejność: przeglądarka, zestaw podstawowy, programy z Windows. Bez powtórzeń, tylko poprawne identyfikatory.
    let mut order: Vec<String> = Vec::new();
    for id in a.browser.iter().chain(a.ids.iter()) {
        if system::valid_app_id(id) && !order.contains(id) {
            order.push(id.clone());
        }
    }
    let browser = a.browser.clone().filter(|b| system::valid_app_id(b));
    {
        let mut i = s.inner.lock().await;
        if i.apps.state == "running" {
            return Json(json!({})).into_response();
        }
        let mut items: Vec<JobItem> = Vec::new();
        if let Some(b) = &browser {
            items.push(JobItem { id: b.clone(), state: "wait" });
        }
        items.push(JobItem { id: BASE.into(), state: "wait" });
        items.extend(order.iter().filter(|id| Some(*id) != browser.as_ref()).map(|id| JobItem { id: id.clone(), state: "wait" }));
        i.apps = Job { state: "running", percent: 0.0, phase: "apps.phase".into(), eta_min: None, items };
    }
    let inner = s.inner.clone();
    tokio::spawn(async move {
        // Bez internetu (instalacja offline, brak Wi-Fi): wybór czeka, aż komputer połączy się z siecią.
        let later = |ids: &[String]| {
            let ids = ids.to_vec();
            let inner = inner.clone();
            async move {
                let mut i = inner.lock().await;
                i.later_apps = ids;
                i.apps.mark_rest("later");
                i.apps.state = "later";
                i.apps.percent = 100.0;
                i.apps.phase = "apps.later".into();
                i.apps.eta_min = None;
            }
        };
        if !system::online().await {
            later(&order).await;
            return;
        }
        let total = order.len() + 1;
        let t0 = Instant::now();
        let mut done = 0usize;
        let mut failed = 0;
        let step = |inner: Arc<Mutex<Inner>>, done: usize| async move {
            let per = t0.elapsed().as_secs_f64() / done.max(1) as f64;
            let mut i = inner.lock().await;
            i.apps.percent = done as f64 / total as f64 * 100.0;
            i.apps.eta_min = Some(((per * (total - done) as f64) / 60.0).ceil() as u64);
        };
        // 1. przeglądarka (od razu jest czym przeglądać internet), 2. zestaw podstawowy, 3. reszta.
        let mut queue: Vec<String> = Vec::new();
        if let Some(b) = &browser {
            queue.push(b.clone());
        }
        queue.push(BASE.into());
        queue.extend(order.iter().filter(|id| Some(*id) != browser.as_ref()).cloned());
        for (n, id) in queue.iter().enumerate() {
            inner.lock().await.apps.mark(id, "run");
            let r = if id == BASE { system::install_base_apps().await } else { system::install_app(id).await };
            match r {
                Ok(()) => inner.lock().await.apps.mark(id, "done"),
                Err(e) => {
                    eprintln!("shrimp-setup: {id}: {}", e.0);
                    if !system::online().await {
                        let rest: Vec<String> = queue[n..].iter().filter(|x| *x != BASE).cloned().collect();
                        later(&rest).await;
                        return;
                    }
                    // Jeden program, którego nie ma we Flathub (albo nie da się go zainstalować), nie zatrzymuje reszty.
                    // Zestaw podstawowy zgłasza błąd, gdy choć jedna aplikacja nie weszła: reszta i tak jest.
                    if id != BASE {
                        failed += 1;
                    }
                    inner.lock().await.apps.mark(id, if id == BASE { "done" } else { "fail" });
                }
            }
            done += 1;
            step(inner.clone(), done).await;
        }
        let mut i = inner.lock().await;
        i.apps.state = if failed > 0 && failed == order.len() { "failed" } else { "done" };
        i.apps.percent = 100.0;
    });
    Json(json!({})).into_response()
}

#[derive(Deserialize)]
struct Browser { id: String }

/// Przeglądarka z kroku „Przeglądarka”: domyślna dla stron i pierwsza w doku (instaluje się z programami).
async fn browser(Json(b): Json<Browser>) -> Response {
    match system::set_browser(&b.id).await {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => err("system", e.0),
    }
}

/// Rozmiar tekstu i tryb nocny od razu; automatyczne aktualizacje przy „Zaczynajmy” (shrimp-setup-apply).
async fn prefs(State(s): State<App>, Json(p): Json<system::Prefs>) -> Response {
    s.inner.lock().await.auto_updates = p.auto_updates;
    match system::prefs(&p).await {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => err("system", e.0),
    }
}

async fn apps_status(State(s): State<App>) -> Json<Job> { Json(s.inner.lock().await.apps.clone()) }

#[derive(Serialize)]
struct UsbReply {
    #[serde(flatten)]
    usb: system::Usb,
    job: Job,
}

async fn usb_get(State(s): State<App>) -> Json<UsbReply> {
    let job = s.inner.lock().await.usb.clone();
    Json(UsbReply { usb: system::usb_status().await, job })
}

#[derive(Deserialize)]
struct UsbAction { action: String }

/// „restore”: formatuje pendrive i wgrywa pliki; błąd albo „home” zostawia pliki w ~/Pendrive.
async fn usb_start(State(s): State<App>, Json(a): Json<UsbAction>) -> Response {
    let st = system::usb_status().await;
    if !st.pending || (a.action == "restore" && !st.present) || !matches!(a.action.as_str(), "restore" | "home") {
        return err("usb", "brak pendrive'a do przywrócenia");
    }
    {
        let mut i = s.inner.lock().await;
        if i.usb.state == "running" {
            return Json(json!({})).into_response();
        }
        i.usb = Job { state: "running", percent: 0.0, phase: "usb.phase".into(), ..Default::default() };
    }
    let inner = s.inner.clone();
    tokio::spawn(async move {
        let mut state = "done";
        if a.action == "restore" {
            if let Err(e) = system::usb_restore(&st.device).await {
                eprintln!("shrimp-setup: pendrive: {}", e.0);
                // Kopia na dysku komputera została: ratujemy pliki w katalogu domowym.
                state = if system::usb_to_home().await.is_ok() { "home" } else { "failed" };
            }
        } else if let Err(e) = system::usb_to_home().await {
            eprintln!("shrimp-setup: ~/Pendrive: {}", e.0);
            state = "failed";
        }
        let mut i = inner.lock().await;
        i.usb.state = state;
        i.usb.percent = 100.0;
    });
    Json(json!({})).into_response()
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Pliki z Windows czekające na zgodę (krok „Pliki z Windows”).
async fn files_get() -> Json<serde_json::Value> {
    let h = home();
    let (files, bytes) = crate::transfer::staged(&h);
    let folders: Vec<serde_json::Value> = crate::transfer::staged_folders(&h)
        .into_iter()
        .map(|(name, files, bytes)| json!({ "name": name, "files": files, "bytes": bytes }))
        .collect();
    Json(json!({ "files": files, "bytes": bytes, "folders": folders }))
}

#[derive(Deserialize)]
struct FilesAction { into_folders: bool, folder: String }

async fn files_place(Json(a): Json<FilesAction>) -> Response {
    let folder = a.folder.replace(['/', '\\'], "").trim().to_string();
    let folder = if folder.is_empty() || folder.starts_with('.') { "Windows".to_string() } else { folder };
    match tokio::task::spawn_blocking(move || crate::transfer::place(&home(), a.into_folders, &folder)).await {
        Ok(Ok(st)) => Json(json!({ "files": st.files })).into_response(),
        Ok(Err(e)) => err("files", e),
        Err(e) => err("files", e),
    }
}

async fn finish(State(s): State<App>) -> Response {
    let (codecs, nvidia, later, auto) = { let i = s.inner.lock().await; (i.codecs, i.nvidia.clone(), i.later_apps.clone(), i.auto_updates) };
    match system::apply(codecs, &nvidia, &later, auto).await {
        Ok(reboot) => {
            if reboot { system::reboot().await; }
            QUIT.store(true, std::sync::atomic::Ordering::SeqCst);
            Json(json!({ "reboot": reboot })).into_response()
        }
        Err(e) => err("system", e.0),
    }
}
