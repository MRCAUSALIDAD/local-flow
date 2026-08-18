mod audio;
pub mod config;
pub mod dsp;
pub mod loopback;
pub mod live;
pub mod session;
pub mod stream;
mod models;
pub mod whisper;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

use audio::Recorder;
use config::Config;
use live::LiveSession;
use session::Session;
use whisper::Transcriber;

#[derive(Clone)]
pub struct AppState {
    recorder: Recorder,
    transcriber: Arc<Mutex<Option<Transcriber>>>,
    config: Arc<Mutex<Config>>,
    config_dir: Arc<PathBuf>,
    data_dir: Arc<PathBuf>,
    live: Arc<LiveSession>,
    session: Arc<Session>,
}

fn emit_status(app: &AppHandle, status: &str) {
    let _ = app.emit("flow-status", status);
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn emit_error(app: &AppHandle, message: &str) {
    eprintln!("[flow] {message}");
    let _ = app.emit("flow-error", message);
    let _ = app.emit("flow-status", "idle");
}

fn begin(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.recorder.is_recording() {
        return;
    }
    // Dictation types into whatever window has focus. During a live session
    // that is most likely the call being transcribed, so refuse rather than
    // spray text into it. The microphone is already captured as its own track.
    if state.live.is_running() {
        emit_error(app, "Stop listening before dictating.");
        return;
    }
    if state.transcriber.lock().unwrap().is_none() {
        emit_error(app, "No voice model yet. Download one in Settings.");
        return;
    }
    let device = state.config.lock().unwrap().input_device.clone();
    state.recorder.start(device);
    emit_status(app, "listening");
}

fn finish(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.recorder.is_recording() {
        return;
    }
    emit_status(app, "transcribing");
    let audio = state.recorder.stop();

    let ctx = match state.transcriber.lock().unwrap().as_ref() {
        Some(t) => t.ctx.clone(),
        None => {
            emit_status(app, "idle");
            return;
        }
    };
    let cfg = state.config.lock().unwrap().clone();
    let app = app.clone();
    let audio_secs = audio.len() as f64 / 16_000.0;

    tauri::async_runtime::spawn_blocking(move || {
        if audio.len() < 1_600 {
            emit_error(
                &app,
                "No audio captured. Check microphone permission and the selected mic in Settings.",
            );
            return;
        }
        let t0 = std::time::Instant::now();
        let result = whisper::transcribe(ctx, &audio, &cfg.language);
        let transcribe_secs = t0.elapsed().as_secs_f64();
        match result {
            Ok(text) if text.is_empty() => {
                emit_error(&app, "No speech detected. Try speaking closer to the mic.")
            }
            Ok(text) => {
                let speed = if transcribe_secs > 0.0 {
                    audio_secs / transcribe_secs
                } else {
                    0.0
                };
                let rtf = if audio_secs > 0.0 {
                    transcribe_secs / audio_secs
                } else {
                    0.0
                };
                let _ = app.emit(
                    "flow-metrics-transcribe",
                    serde_json::json!({
                        "audioSecs": audio_secs,
                        "transcribeMs": transcribe_secs * 1000.0,
                        "rtf": rtf,
                        "speed": speed,
                    }),
                );
                if cfg.copy_to_clipboard {
                    let _ = app.clipboard().write_text(text.clone());
                }
                if cfg.auto_type {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    let trusted = accessibility_ok();
                    match type_text(&text) {
                        Ok(()) => {
                            let msg = format!(
                                "auto_type ok: {} chars typed (accessibility_trusted={trusted})",
                                text.chars().count()
                            );
                            eprintln!("[flow] {msg}");
                            let _ = app.emit("flow-debug", msg);
                        }
                        Err(e) => {
                            let msg = format!(
                                "auto_type failed: {e} (accessibility_trusted={trusted})"
                            );
                            eprintln!("[flow] {msg}");
                            let _ = app.emit("flow-debug", msg.clone());
                            emit_error(&app, &format!("Typing failed: {e}"));
                        }
                    }
                }
                let _ = app.emit("flow-transcript", &text);
                emit_status(&app, "idle");
            }
            Err(e) => emit_error(&app, &format!("Transcription failed: {e}")),
        }
    });
}

fn type_text(text: &str) -> Result<(), String> {
    use enigo::{Enigo, Keyboard, Settings};
    let settings = Settings {
        open_prompt_to_get_permissions: false,
        ..Settings::default()
    };
    let mut enigo = Enigo::new(&settings).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn accessibility_ok() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos_accessibility_client::accessibility::application_is_trusted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[tauri::command]
fn request_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[tauri::command]
fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn model_loaded(state: State<AppState>) -> bool {
    state.transcriber.lock().unwrap().is_some()
}

#[tauri::command]
fn set_config(app: AppHandle, state: State<AppState>, config: Config) -> Result<(), String> {
    let previous = state.config.lock().unwrap().model_path.clone();
    let model_changed = previous != config.model_path;

    *state.config.lock().unwrap() = config.clone();
    config::save(&state.config_dir, &config).map_err(|e| e.to_string())?;

    if model_changed {
        match &config.model_path {
            Some(path) => match Transcriber::new(path) {
                Ok(t) => *state.transcriber.lock().unwrap() = Some(t),
                Err(e) => {
                    *state.transcriber.lock().unwrap() = None;
                    return Err(format!("Failed to load model: {e}"));
                }
            },
            None => *state.transcriber.lock().unwrap() = None,
        }
    }

    apply_overlay_layout(&app, &config);
    Ok(())
}

#[tauri::command]
fn list_microphones() -> Vec<String> {
    audio::list_input_devices()
}

#[tauri::command]
fn list_models(state: State<AppState>) -> Vec<models::ModelInfo> {
    let active = state.config.lock().unwrap().model_path.clone();
    models::list(&state.data_dir, active.as_deref())
}

#[tauri::command]
fn download_model(app: AppHandle, id: String) {
    let state = app.state::<AppState>();
    let data_dir = (*state.data_dir).clone();
    let config_dir = (*state.config_dir).clone();
    let config = state.config.clone();
    let transcriber = state.transcriber.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let _ = app.emit("model-progress", (id.clone(), 0u64, 0u64));
        let mut last_pct = -1i32;
        let result = models::download(&data_dir, &id, |received, total| {
            let pct = if total > 0 {
                (received * 100 / total) as i32
            } else {
                -1
            };
            if pct != last_pct {
                last_pct = pct;
                let _ = app.emit("model-progress", (id.clone(), received, total));
            }
        });

        match result {
            Ok(path) => {
                let path_str = path.to_string_lossy().to_string();
                match Transcriber::new(&path_str) {
                    Ok(t) => {
                        *transcriber.lock().unwrap() = Some(t);
                        let mut cfg = config.lock().unwrap();
                        cfg.model_path = Some(path_str);
                        let _ = config::save(&config_dir, &cfg);
                        drop(cfg);
                        let _ = app.emit("model-ready", &id);
                    }
                    Err(e) => emit_error(&app, &format!("Model downloaded but failed to load: {e}")),
                }
            }
            Err(e) => emit_error(&app, &format!("Download failed: {e}")),
        }
    });
}

#[tauri::command]
fn start_dictation(app: AppHandle) {
    begin(&app);
}

#[tauri::command]
fn stop_dictation(app: AppHandle) {
    finish(&app);
}

// --- live listening (system audio) --------------------------------------------

/// `None` when system audio capture is usable, otherwise the reason it is not.
#[tauri::command]
fn system_audio_status() -> Option<String> {
    loopback::availability().err()
}

#[tauri::command]
fn list_loopback_sources() -> Vec<loopback::LoopbackSource> {
    loopback::list_sources()
}

#[tauri::command]
fn start_listening(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if state.live.is_running() {
        return Err("Already listening.".into());
    }
    let ctx = match state.transcriber.lock().unwrap().as_ref() {
        Some(t) => t.ctx.clone(),
        None => return Err("No voice model yet. Download one in Settings.".into()),
    };
    let cfg = state.config.lock().unwrap().clone();

    state.session.begin();

    let segment_app = app.clone();
    let segment_session = state.session.clone();
    let state_app = app.clone();

    state.live.start(
        ctx,
        &cfg,
        move |segment| {
            let entry = segment_session.push(segment);
            let _ = segment_app.emit("flow-live-segment", entry);
        },
        move |live_state| {
            let _ = state_app.emit("flow-live-state", live_state);
        },
    )?;

    emit_status(&app, "listening-system");
    Ok(())
}

#[tauri::command]
fn stop_listening(app: AppHandle, state: State<AppState>) {
    state.live.stop();
    emit_status(&app, "idle");
}

#[tauri::command]
fn live_entries(state: State<AppState>) -> Vec<session::Entry> {
    state.session.entries()
}

#[tauri::command]
fn clear_session(state: State<AppState>) {
    state.session.clear();
}

#[tauri::command]
fn session_text(state: State<AppState>) -> String {
    state.session.to_text()
}

/// Writes the transcript into the app data directory, returning its path.
#[tauri::command]
fn export_session(state: State<AppState>, format: String) -> Result<String, String> {
    if state.session.is_empty() {
        return Err("Nothing to export yet.".into());
    }
    let format = session::Format::parse(&format)
        .ok_or_else(|| format!("Unknown export format: {format}"))?;
    state
        .session
        .write(&state.data_dir, format)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("Could not write the transcript: {e}"))
}

const OVERLAY_MARGIN: f64 = 130.0;

fn position_overlay(window: &tauri::WebviewWindow, w: f64, h: f64) {
    let _ = window.set_size(tauri::LogicalSize::new(w, h));
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let x = (size.width as f64 - w * scale) / 2.0;
        let y = size.height as f64 - (OVERLAY_MARGIN + h) * scale;
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

fn position_overlay_corner(window: &tauri::WebviewWindow, w: f64, h: f64, corner: &str) {
    let _ = window.set_size(tauri::LogicalSize::new(w, h));
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let m = 24.0;
        let (x, y) = match corner {
            "top-left" => (m * scale, m * scale),
            "top-right" => (size.width as f64 - (w + m) * scale, m * scale),
            "bottom-left" => (m * scale, size.height as f64 - (h + m) * scale),
            _ => (
                size.width as f64 - (w + m) * scale,
                size.height as f64 - (h + m) * scale,
            ),
        };
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

fn apply_overlay_layout(app: &AppHandle, cfg: &Config) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        if cfg.show_metrics {
            position_overlay_corner(&overlay, 260.0, 200.0, &cfg.metrics_corner);
        } else {
            position_overlay(&overlay, 34.0, 34.0);
        }
    }
    let _ = app.emit("flow-config", cfg);
}

#[tauri::command]
fn overlay_expand(app: AppHandle) {
    let pinned = app.state::<AppState>().config.lock().unwrap().show_metrics;
    if pinned {
        return;
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        position_overlay(&overlay, 260.0, 200.0);
    }
}

#[tauri::command]
fn overlay_collapse(app: AppHandle) {
    let pinned = app.state::<AppState>().config.lock().unwrap().show_metrics;
    if pinned {
        return;
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        position_overlay(&overlay, 34.0, 34.0);
    }
}

fn start_metrics_sampler(app: AppHandle) {
    std::thread::spawn(move || {
        use sysinfo::{ProcessesToUpdate, System};
        let pid = match sysinfo::get_current_pid() {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut sys = System::new();
        loop {
            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            if let Some(proc_) = sys.process(pid) {
                let cpu = proc_.cpu_usage();
                let ram_mb = proc_.memory() as f64 / 1024.0 / 1024.0;
                let _ = app.emit(
                    "flow-metrics",
                    serde_json::json!({ "cpuPct": cpu, "ramMb": ram_mb }),
                );
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let hotkey = Shortcut::new(Some(Modifiers::ALT), Code::Space);
    let handler_hotkey = hotkey.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if *shortcut == handler_hotkey {
                        match event.state() {
                            ShortcutState::Pressed => begin(app),
                            ShortcutState::Released => finish(app),
                        }
                    }
                })
                .build(),
        )
        .setup(move |app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| config_dir.clone());
            let cfg = config::load(&config_dir);

            let transcriber = cfg
                .model_path
                .as_deref()
                .and_then(|p| match Transcriber::new(p) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        eprintln!("[flow] model load failed at startup: {e}");
                        None
                    }
                });

            let cfg_layout = cfg.clone();

            app.manage(AppState {
                recorder: Recorder::new(),
                live: Arc::new(LiveSession::new()),
                session: Arc::new(Session::new()),
                transcriber: Arc::new(Mutex::new(transcriber)),
                config: Arc::new(Mutex::new(cfg)),
                config_dir: Arc::new(config_dir),
                data_dir: Arc::new(data_dir),
            });

            app.global_shortcut().register(hotkey.clone())?;

            let show = MenuItem::with_id(app, "show", "Show Local Flow", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Local Flow")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_always_on_top(true);
                let _ = overlay.set_visible_on_all_workspaces(true);
                let _ = overlay.show();
            }
            apply_overlay_layout(app.handle(), &cfg_layout);

            start_metrics_sampler(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            model_loaded,
            list_microphones,
            list_models,
            download_model,
            accessibility_ok,
            request_accessibility,
            open_accessibility_settings,
            start_dictation,
            stop_dictation,
            system_audio_status,
            list_loopback_sources,
            start_listening,
            stop_listening,
            live_entries,
            clear_session,
            session_text,
            export_session,
            overlay_expand,
            overlay_collapse,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
