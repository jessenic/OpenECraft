//! OpenECraft – an open source electrical CAE editor (schematics, device tags,
//! cross references, reports) inspired by EPLAN Electric P8.
//!
//! Runs natively on Windows, Linux and macOS and in the browser (WebAssembly).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(not(test), warn(clippy::too_many_lines))]

mod app;
mod autosave;
mod shutdown;
mod commands;
mod dock;
#[cfg(test)]
mod conventions;
mod editor;
mod frame_stats;
#[cfg(test)]
mod glyphs;
mod insert_center;
mod io;
mod jobs;
mod mounting_ui;
mod panel_ui;
mod plugin_commands;
mod shortcuts;
mod workspaces;
mod parts_ui;
mod remote;
mod ribbon;
mod style;
mod vault;
mod session;
mod wiring_ui;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenECraft")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([800.0, 500.0])
            .with_icon(icon()),
        ..Default::default()
    };
    // `--wiring` opens the wiring assistant (touch screens in the workshop), another
    // argument is a project file to open (file association of the installer).
    let wiring = std::env::args().any(|a| a == "--wiring");
    let file = std::env::args().skip(1).find(|a| !a.starts_with("--"));
    eframe::run_native("OpenECraft", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc).start(wiring, file)))))
}

/// Window and taskbar icon (`assets/icon.png`, generated with `installer/icon.py`).
#[cfg(not(target_arch = "wasm32"))]
fn icon() -> egui::IconData {
    let img = image::load_from_memory(include_bytes!("../../assets/icon.png")).expect("bundled icon").into_rgba8();
    egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if let Some(lang) = web_sys::window().and_then(|w| w.navigator().language()).and_then(|code| openecraft_i18n::Lang::from_code(&code)) {
        openecraft_i18n::set_lang(lang);
    }
    // `…/#wiring` opens the wiring assistant (tablets in the workshop).
    let wiring = web_sys::window().and_then(|w| w.location().hash().ok()).is_some_and(|h| h == "#wiring");
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("openecraft_canvas")
            .expect("canvas missing")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("not a canvas");
        let result = eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(move |cc| Ok(Box::new(app::App::new(cc).start(wiring, None)))))
            .await;
        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => {
                    loading.set_inner_html("<p>OpenECraft failed to start. See the browser console for details.</p>");
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
