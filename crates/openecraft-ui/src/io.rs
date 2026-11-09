//! Platform specific file handling: native file dialogs on desktop, file picker and
//! downloads in the browser. Generic over the application's kinds of files.

use std::sync::{Arc, Mutex};

/// Runs work off the desktop UI thread, including on single-core machines.
/// The browser has no worker threads and runs it immediately.
pub fn background(work: impl FnOnce() + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(work);
    #[cfg(target_arch = "wasm32")]
    work();
}

/// What a file is opened for; provides the file dialog filter.
pub trait FileKind: Copy + Send + 'static {
    /// Filter name and extensions.
    fn filter(self) -> (&'static str, &'static [&'static str]);
}

pub struct Loaded<K> {
    pub purpose: K,
    /// Native: full path. Web: file name.
    pub name: String,
    /// Raw file content (binary STL files are not text).
    pub bytes: Vec<u8>,
}

impl<K> Loaded<K> {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

pub struct FileIo<K> {
    inbox: Arc<Mutex<Vec<Loaded<K>>>>,
}

impl<K> Default for FileIo<K> {
    fn default() -> Self {
        FileIo { inbox: Arc::default() }
    }
}

impl<K> Clone for FileIo<K> {
    fn clone(&self) -> Self {
        FileIo { inbox: self.inbox.clone() }
    }
}

impl<K: FileKind> FileIo<K> {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(&self, _ctx: &egui::Context, purpose: K) {
        let (name, ext) = purpose.filter();
        let Some(path) = rfd::FileDialog::new()
            .add_filter(name, ext)
            .pick_file()
        else {
            return;
        };
        self.open_path(&path.display().to_string(), purpose);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn open(&self, ctx: &egui::Context, purpose: K) {
        let inbox = self.inbox.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let (name, ext) = purpose.filter();
            let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter(name, ext)
                .pick_file()
                .await
            else {
                return;
            };
            let bytes = file.read().await;
            if let Ok(mut inbox) = inbox.lock() {
                inbox.push(Loaded { purpose, name: file.file_name(), bytes });
            }
            ctx.request_repaint();
        });
    }
}

impl<K> FileIo<K> {
    /// Takes a file that finished loading (the web picker is asynchronous).
    pub fn poll(&self) -> Option<Loaded<K>> {
        self.inbox.lock().ok()?.pop()
    }

    /// Reads a file by path, e.g. a project passed on the command line (desktop only).
    pub fn open_path(&self, path: &str, purpose: K) {
        match read_file(path) {
            Ok(bytes) => {
                if let Ok(mut inbox) = self.inbox.lock() {
                    inbox.push(Loaded { purpose, name: path.to_string(), bytes });
                }
            }
            Err(e) => log::error!("Could not read {path}: {e}"),
        }
    }

    /// Asks for a location and writes the file. Returns the chosen path / name.
    pub fn save_as(&self, suggested: &str, filter: (&str, &str), content: &str) -> Result<Option<String>, String> {
        self.save_bytes_as(suggested, filter, content.as_bytes())
    }

    /// Like [`FileIo::save_as`] for binary content.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_bytes_as(&self, suggested: &str, filter: (&str, &str), content: &[u8]) -> Result<Option<String>, String> {
        let Some(path) = rfd::FileDialog::new().add_filter(filter.0, &[filter.1]).set_file_name(suggested).save_file()
        else {
            return Ok(None);
        };
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    }

    /// In the browser "save as" is a download.
    #[cfg(target_arch = "wasm32")]
    pub fn save_bytes_as(&self, suggested: &str, filter: (&str, &str), content: &[u8]) -> Result<Option<String>, String> {
        download(suggested, mime_type(filter.1), content).map_err(|e| format!("{e:?}"))?;
        Ok(Some(suggested.to_string()))
    }

    /// Writes to a known location (native) or downloads again (web).
    pub fn save_to(&self, path: &str, content: &str) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::fs::write(path, content).map_err(|e| e.to_string())
        }
        #[cfg(target_arch = "wasm32")]
        {
            download(path, "application/json", content.as_bytes()).map_err(|e| format!("{e:?}"))
        }
    }

    pub fn can_save_in_place() -> bool {
        cfg!(not(target_arch = "wasm32"))
    }

    /// The app runs in a window of its own (desktop), not in a browser tab.
    pub fn has_own_window() -> bool {
        cfg!(not(target_arch = "wasm32"))
    }
}

/// MIME type of a download by file extension.
pub fn mime_type(ext: &str) -> &'static str {
    match ext {
        "svg" => "image/svg+xml",
        "html" => "text/html",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        "dxf" => "application/dxf",
        "dwg" => "image/vnd.dwg",
        "igs" | "iges" => "model/iges",
        "x_t" => "model/vnd.parasolid.transmit.text",
        "x_b" => "model/vnd.parasolid.transmit.binary",
        "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "png" | "jpg" | "jpeg" | "zip" | "jt" | "ifc" | "oecz" => "application/octet-stream",
        _ => "application/json",
    }
}

#[cfg(target_arch = "wasm32")]
fn download(name: &str, mime: &str, content: &[u8]) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(content));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    web_sys::Url::revoke_object_url(&url)?;
    Ok(())
}

/// Script added to a print document: opens the print dialog once the page is loaded.
const PRINT_SCRIPT: &str = "<script>window.addEventListener('load', () => setTimeout(() => window.print(), 300));</script>";

/// The HTML document with the print script (before `</body>`, or at the end).
pub fn with_print_script(html: &str) -> String {
    match html.rfind("</body>") {
        Some(i) => format!("{}{PRINT_SCRIPT}{}", &html[..i], &html[i..]),
        None => format!("{html}{PRINT_SCRIPT}"),
    }
}

/// Shows an HTML print document with the print dialog: in a new browser tab (web) or
/// in the default browser through a temporary file (desktop).
pub fn print_html(html: &str) -> Result<(), String> {
    let html = with_print_script(html);
    #[cfg(target_arch = "wasm32")]
    {
        open_in_tab(&html).map_err(|e| format!("{e:?}"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::env::temp_dir().join("openecraft_print.html");
        std::fs::write(&path, html).map_err(|e| e.to_string())?;
        let file = path.to_string_lossy().to_string();
        let status = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd").args(["/C", "start", "", &file]).spawn()
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg(&file).spawn()
        } else {
            std::process::Command::new("xdg-open").arg(&file).spawn()
        };
        status.map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
fn open_in_tab(html: &str) -> Result<(), wasm_bindgen::JsValue> {
    let parts = js_sys::Array::new();
    parts.push(&wasm_bindgen::JsValue::from_str(html));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("text/html");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    web_sys::window().ok_or("no window")?.open_with_url_and_target(&url, "_blank")?;
    Ok(())
}

/// Sets the title of the window (the browser tab on the web, where eframe does not).
pub fn set_title(ctx: &egui::Context, title: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            doc.set_title(title);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.to_string()));
}

/// Today's date `YYYY-MM-DD` (UTC).
pub fn today() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    #[cfg(target_arch = "wasm32")]
    let secs = (js_sys::Date::now() / 1000.0) as u64;
    openecraft_parts::date_from_unix(secs)
}

/// The local file system can be searched and written by path (desktop, not the
/// browser).
pub fn has_file_system() -> bool {
    cfg!(not(target_arch = "wasm32"))
}

/// Asks for a folder (desktop only).
pub fn pick_folder() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        rfd::FileDialog::new().pick_folder().map(|p| p.display().to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Files in a folder and its subfolders whose path passes `keep`, sorted (desktop only).
pub fn files_in(folder: &str, keep: impl Fn(&str) -> bool) -> Vec<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut out = Vec::new();
        let mut dirs = vec![std::path::PathBuf::from(folder)];
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let path = e.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if keep(&path.to_string_lossy()) {
                    out.push(path.display().to_string());
                }
            }
        }
        out.sort();
        out
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (folder, keep);
        Vec::new()
    }
}

/// Reads a file by path (desktop only).
pub fn read_file(path: &str) -> Result<Vec<u8>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read(path).map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err(format!("{path}: no file system in the browser"))
    }
}

/// Writes a file by path (desktop only).
pub fn write_file(path: &str, content: &[u8]) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::write(path, content).map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = content;
        Err(format!("{path}: no file system in the browser"))
    }
}

/// Large values an app keeps between sessions (the autosaved project, the parts
/// database): IndexedDB in the browser, whose local storage holds only about 5 MB, and a
/// file per key in the app's data folder on the desktop. Reading is asynchronous in the
/// browser: values arrive through [`Store::poll`] (on the desktop at once). Writes are
/// queued and done in order; a later value of a key replaces one not yet written.
#[derive(Clone)]
pub struct Store {
    /// Database name (browser) or folder (desktop).
    place: String,
    inbox: Arc<Mutex<Vec<Stored>>>,
    queue: Arc<Mutex<StoreQueue>>,
}

/// A key with its value as read from a [`Store`] (`None`: nothing stored).
pub type Stored = (String, Option<Vec<u8>>);

#[derive(Default)]
struct StoreQueue {
    pending: Vec<(String, Vec<u8>)>,
    writing: bool,
    /// Result of the last write (it fails e.g. without IndexedDB in a private window).
    last_ok: Option<bool>,
}

impl Store {
    /// The store of the app `app` (e.g. "OpenECraft").
    pub fn new(app: &str) -> Store {
        #[cfg(not(target_arch = "wasm32"))]
        let place = data_dir(app).join("store").display().to_string();
        #[cfg(target_arch = "wasm32")]
        let place = format!("{app}-store");
        Store::at(place)
    }

    /// A store in the folder (desktop) or IndexedDB database (browser) `place`.
    pub fn at(place: impl Into<String>) -> Store {
        Store { place: place.into(), inbox: Arc::default(), queue: Arc::default() }
    }

    /// Starts reading `key`; the value (`None` if there is none) arrives through
    /// [`Store::poll`].
    pub fn load(&self, ctx: &egui::Context, key: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ctx;
            let value = std::fs::read(self.path(key)).ok();
            if let Ok(mut inbox) = self.inbox.lock() {
                inbox.push((key.to_string(), value));
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let (store, key, ctx) = (self.clone(), key.to_string(), ctx.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let value = match idb::get(&store.place, &key).await {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!("Could not read {key} from IndexedDB: {e:?}");
                        None
                    }
                };
                if let Ok(mut inbox) = store.inbox.lock() {
                    inbox.push((key, value));
                }
                ctx.request_repaint();
            });
        }
    }

    /// A value that finished loading: (key, value).
    pub fn poll(&self) -> Option<Stored> {
        let mut inbox = self.inbox.lock().ok()?;
        (!inbox.is_empty()).then(|| inbox.remove(0))
    }

    /// Writes `value` under `key` (in the background in the browser).
    pub fn store(&self, key: &str, value: Vec<u8>) {
        let Ok(mut q) = self.queue.lock() else { return };
        q.pending.retain(|(k, _)| k != key);
        q.pending.push((key.to_string(), value));
        if q.writing {
            return;
        }
        q.writing = true;
        drop(q);
        #[cfg(not(target_arch = "wasm32"))]
        self.write_pending();
        #[cfg(target_arch = "wasm32")]
        {
            let store = self.clone();
            wasm_bindgen_futures::spawn_local(async move { store.write_pending().await });
        }
    }

    /// The last write succeeded. Until then (writes are asynchronous in the browser) the
    /// caller should keep its values elsewhere as well.
    pub fn works(&self) -> bool {
        self.queue.lock().is_ok_and(|q| q.last_ok == Some(true))
    }

    fn next_pending(&self) -> Option<(String, Vec<u8>)> {
        let mut q = self.queue.lock().ok()?;
        if q.pending.is_empty() {
            q.writing = false;
            return None;
        }
        Some(q.pending.remove(0))
    }

    fn written(&self, ok: bool) {
        if let Ok(mut q) = self.queue.lock() {
            q.last_ok = Some(ok);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn write_pending(&self) {
        while let Some((key, value)) = self.next_pending() {
            let path = self.path(&key);
            let tmp = path.with_extension("tmp");
            let result = std::fs::create_dir_all(&self.place).and_then(|()| std::fs::write(&tmp, &value)).and_then(|()| std::fs::rename(&tmp, &path));
            if let Err(e) = &result {
                log::error!("Could not write {}: {e}", path.display());
            }
            self.written(result.is_ok());
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn write_pending(&self) {
        while let Some((key, value)) = self.next_pending() {
            let result = idb::put(&self.place, &key, &value).await;
            if let Err(e) = &result {
                log::error!("Could not write {key} to IndexedDB: {e:?}");
            }
            self.written(result.is_ok());
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn path(&self, key: &str) -> std::path::PathBuf {
        let name: String = key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
        std::path::Path::new(&self.place).join(name)
    }
}

/// Folder for the data of the app `app`, as eframe chooses it for its own storage.
#[cfg(not(target_arch = "wasm32"))]
fn data_dir(app: &str) -> std::path::PathBuf {
    let env = |v: &str| std::env::var_os(v).filter(|s| !s.is_empty()).map(std::path::PathBuf::from);
    let home = env("HOME").or_else(|| env("USERPROFILE")).unwrap_or_else(std::env::temp_dir);
    let base = if cfg!(target_os = "windows") {
        env("APPDATA").unwrap_or_else(|| home.join("AppData").join("Roaming"))
    } else if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        env("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local").join("share"))
    };
    base.join(app).join("data")
}

/// The key-value store of [`Store`] in IndexedDB: one object store of byte arrays.
#[cfg(target_arch = "wasm32")]
mod idb {
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};

    const VALUES: &str = "values";

    /// The result of `req` once it succeeded.
    async fn finished(req: &web_sys::IdbRequest) -> Result<JsValue, JsValue> {
        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            req.set_onsuccess(Some(&resolve));
            req.set_onerror(Some(&reject));
        });
        wasm_bindgen_futures::JsFuture::from(promise).await?;
        req.result()
    }

    async fn open(name: &str) -> Result<web_sys::IdbDatabase, JsValue> {
        let factory = web_sys::window().ok_or("no window")?.indexed_db()?.ok_or("no IndexedDB")?;
        let req = factory.open_with_u32(name, 1)?;
        let upgrade = Closure::<dyn FnMut(web_sys::IdbVersionChangeEvent)>::new(|e: web_sys::IdbVersionChangeEvent| {
            let db = e.target().and_then(|t| t.dyn_into::<web_sys::IdbOpenDbRequest>().ok()).and_then(|r| r.result().ok()).and_then(|r| r.dyn_into::<web_sys::IdbDatabase>().ok());
            if let Some(db) = db.filter(|db| !db.object_store_names().contains(VALUES)) {
                if let Err(e) = db.create_object_store(VALUES) {
                    log::error!("Could not create the IndexedDB store: {e:?}");
                }
            }
        });
        req.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
        let db = finished(&req).await?;
        drop(upgrade);
        db.dyn_into()
    }

    pub async fn get(name: &str, key: &str) -> Result<Option<Vec<u8>>, JsValue> {
        let db = open(name).await?;
        let req = db.transaction_with_str(VALUES)?.object_store(VALUES)?.get(&JsValue::from_str(key))?;
        let value = finished(&req).await?;
        db.close();
        Ok((!value.is_undefined() && !value.is_null()).then(|| js_sys::Uint8Array::new(&value).to_vec()))
    }

    pub async fn put(name: &str, key: &str, value: &[u8]) -> Result<(), JsValue> {
        let db = open(name).await?;
        let store = db.transaction_with_str_and_mode(VALUES, web_sys::IdbTransactionMode::Readwrite)?.object_store(VALUES)?;
        let req = store.put_with_key(&js_sys::Uint8Array::from(value), &JsValue::from_str(key))?;
        finished(&req).await?;
        db.close();
        Ok(())
    }
}

/// File name without directories, for window titles.
pub fn short_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests;
