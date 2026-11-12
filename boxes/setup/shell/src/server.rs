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
}

#[derive(Default)]
struct Inner {
    codecs: bool,
    nvidia: String,
    apps: Job,
    usb: Job,
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
        inner: Arc::new(Mutex::new(Inner { codecs: false, nvidia: "none".into(), apps: Job { state: "idle", ..Default::default() }, usb: Job { state: "idle", ..Default::default() } })),
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
        .route("/apps", post(apps_start).get(apps_status))
        .route("/usb", get(usb_get).post(usb_start))
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
async fn online() -> Json<serde_json::Value> { Json(json!({ "online": system::online().await })) }

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
struct Apps { ids: Vec<String> }

async fn apps_start(State(s): State<App>, Json(a): Json<Apps>) -> Response {
    {
        let mut i = s.inner.lock().await;
        if i.apps.state == "running" {
            return Json(json!({})).into_response();
        }
        i.apps = Job { state: "running", percent: 0.0, phase: "apps.phase".into(), eta_min: None };
    }
    let inner = s.inner.clone();
    tokio::spawn(async move {
        if !system::online().await {
            inner.lock().await.apps.state = "offline";
            return;
        }
        // Najpierw zestaw podstawowy ze sklepu Shrimp; niepowodzenie nie zatrzymuje wyboru użytkownika.
        if let Err(e) = system::install_base_apps().await {
            eprintln!("shrimp-setup: zestaw podstawowy: {}", e.0);
            if !system::online().await {
                inner.lock().await.apps.state = "offline";
                return;
            }
        }
        let total = a.ids.len().max(1);
        let t0 = Instant::now();
        let mut failed = 0;
        for (n, id) in a.ids.iter().enumerate() {
            if let Err(e) = system::install_app(id).await {
                eprintln!("shrimp-setup: {id}: {}", e.0);
                if !system::online().await {
                    inner.lock().await.apps.state = "offline";
                    return;
                }
                // Jeden program, którego nie ma we Flathub (albo nie da się go zainstalować), nie zatrzymuje reszty.
                failed += 1;
            }
            let done = n + 1;
            let per = t0.elapsed().as_secs_f64() / done as f64;
            let mut i = inner.lock().await;
            i.apps.percent = done as f64 / total as f64 * 100.0;
            i.apps.eta_min = Some(((per * (total - done) as f64) / 60.0).ceil() as u64);
        }
        let mut i = inner.lock().await;
        i.apps.state = if failed > 0 && failed == a.ids.len() { "failed" } else { "done" };
        i.apps.percent = 100.0;
    });
    Json(json!({})).into_response()
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
        i.usb = Job { state: "running", percent: 0.0, phase: "usb.phase".into(), eta_min: None };
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

async fn finish(State(s): State<App>) -> Response {
    let (codecs, nvidia) = { let i = s.inner.lock().await; (i.codecs, i.nvidia.clone()) };
    match system::apply(codecs, &nvidia).await {
        Ok(reboot) => {
            if reboot { system::reboot().await; }
            QUIT.store(true, std::sync::atomic::Ordering::SeqCst);
            Json(json!({ "reboot": reboot })).into_response()
        }
        Err(e) => err("system", e.0),
    }
}
