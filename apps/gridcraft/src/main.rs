//! GridCraft desktop app.
//!
//! Usage: `gridcraft [--control <port> [--control-token <token>]] [--sample <name>] [files…]`
//!
//! `--control <port>` (or `GRIDCRAFT_CONTROL_PORT`) starts a localhost JSON-lines control
//! server: `{"id":1,"method":"ui.inspect","params":{}}` → `{"id":1,"ok":true,"result":…}`.
//! The first line of each connection must carry the token (`docs/control-protocol.md`).
//! See `gridcraft_ui_egui::control` and `docs/control-protocol.md` for the methods.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "macos")]
mod apple_events;
mod control_server;
#[cfg(any(target_os = "windows", target_os = "macos", test))]
mod graphics;
mod logging;
#[cfg(target_os = "macos")]
mod native_menu;
mod print;

use std::cell::RefCell;
use std::rc::Rc;

use gridcraft_engine::Session;
use gridcraft_ui_egui::doc_windows::DocWindows;
use gridcraft_ui_egui::{Services, SheetApp};
use serde_json::json;

/// The app, its native window per workbook window, and (macOS) the menu bar and Finder events.
struct App(SheetApp, DocWindows, #[cfg(target_os = "macos")] Option<native_menu::NativeMenu>, #[cfg(target_os = "macos")] fmv_macos_events::Inbox);

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        {
            if self.2.is_none() && std::env::var_os("GRIDCRAFT_NO_NATIVE_MENU").is_none() {
                self.2 = Some(native_menu::NativeMenu::install(ctx));
            }
            if let Some(m) = &mut self.2 {
                m.poll(&mut self.0, ctx, self.1.focused_viewport());
            }
            apple_events::poll(&self.3, &mut self.0, ctx);
        }
        self.0.logic(ctx);
        self.1.logic(&mut self.0, ctx);
        if ctx.input(|input| input.viewport().close_requested()) && self.0.printing.busy() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }
    /// Forwards raw input to the shared app, then opens every non-empty dropped file path as a workbook.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.0.raw_input_hook(ctx, raw);
        // Files dropped on the window open as workbooks.
        for f in std::mem::take(&mut raw.dropped_files) {
            if let Some(p) = f.path().to_str().map(str::to_string).filter(|p| !p.is_empty()) {
                self.0.open_path(&p);
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.1.show(&mut self.0, ui);
    }
    fn on_exit(&mut self) {
        print::finish_pending_jobs();
        save_prefs(&self.0);
        // A ready update installs once the process has exited. The macOS exit below skips
        // dropping the app, so the helper is started here.
        self.0.start_pending_update();
        // macOS (#117): end the process here, before eframe tears the window down. AccessKit's
        // macOS adapter subclasses the window's content view and restores its class on teardown;
        // AppKit's Touch Bar support observes that view (KVO), so in the next display cycle
        // `-[_NSTouchBarFinderObservation invalidate]` throws "not registered" and the quit ends
        // in a crash report. eframe calls `on_exit` on every way of quitting (Quit/⌘Q, the Dock,
        // the close button, `app.quit`), and nothing after it is needed: the preferences are
        // saved and the log file is unbuffered. eframe does the same when it doesn't
        // run-and-return. `on_exit` only runs once the app exists, never on a failed start.
        #[cfg(target_os = "macos")]
        std::process::exit(0);
    }
}

fn config_dir() -> Option<std::path::PathBuf> {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support/GridCraft"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("GridCraft"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
            .map(|c| c.join("gridcraft"))
    }
}

/// Runs without preferences (`GRIDCRAFT_NO_PREFS`, agents' test runs) neither read nor write
/// them, and keep no log file.
fn prefs_enabled() -> bool {
    std::env::var_os("GRIDCRAFT_NO_PREFS").is_none()
}

/// Where the log files live: `logs` in the settings folder, next to `ui.json` (see `logging`).
fn log_dir() -> Option<std::path::PathBuf> {
    Some(config_dir()?.join("logs"))
}

fn load_prefs(app: &mut SheetApp) {
    if !prefs_enabled() {
        return;
    }
    // First run (or a ui.json without a usable language): follow the desktop's language.
    app.ui.language = gridcraft_ui_egui::i18n::Language::system();
    // ...and its light or dark appearance (#239), until a theme is chosen and saved in ui.json.
    app.ui.system_theme = true;
    let Some(dir) = config_dir() else { return };
    if let Ok(b) = std::fs::read(dir.join("ui.json"))
        && let Ok(ui) = serde_json::from_slice::<gridcraft_ui_egui::UiState>(&b)
    {
        let saved = serde_json::from_slice(&b).ok().and_then(|v| gridcraft_ui_egui::i18n::saved_language(&v));
        let language = saved.unwrap_or(app.ui.language);
        app.ui = ui.at_startup();
        app.ui.language = language;
    }
    if let Ok(b) = std::fs::read(dir.join("prefs.json"))
        && let Ok(p) = serde_json::from_slice(&b)
    {
        app.session.prefs = p;
    }
}

fn save_prefs(app: &SheetApp) {
    if !prefs_enabled() {
        return;
    }
    let Some(dir) = config_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(b) = serde_json::to_vec_pretty(&app.ui) {
        let _ = std::fs::write(dir.join("ui.json"), b);
    }
    if let Ok(b) = serde_json::to_vec_pretty(&app.session.prefs) {
        let _ = std::fs::write(dir.join("prefs.json"), b);
    }
}

/// Write HTML, plain text, and an optional PNG snapshot to the macOS general pasteboard in a
/// single `clearContents` session so apps like Zalo show a crisp table image preview while still
/// offering the "Paste as text" toggle (matching Microsoft Excel).
#[cfg(target_os = "macos")]
fn write_macos_clipboard(html: &str, text: &str, png: Option<&[u8]>) -> Result<(), String> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSData, NSString};

    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    if let Some(png_bytes) = png {
        let data = NSData::with_bytes(png_bytes);
        let ty = NSString::from_str("public.png");
        let _ = pb.setData_forType(Some(&data), &ty);
    }
    let html_str = NSString::from_str(html);
    let html_ty = NSString::from_str("public.html");
    let ok_html = pb.setString_forType(&html_str, &html_ty);
    let text_str = NSString::from_str(text);
    let text_ty = NSString::from_str("public.utf8-plain-text");
    let ok_text = pb.setString_forType(&text_str, &text_ty);
    if ok_html || ok_text {
        Ok(())
    } else {
        Err("NSPasteboard write failed".into())
    }
}

/// Format an HTML fragment using Microsoft's Windows `HTML Format` (`CF_HTML` v0.9) byte-offset envelope.
#[cfg(any(target_os = "windows", test))]
fn wrap_win_cf_html(ctn: &str) -> String {
    let h_version = "Version:0.9";
    let h_start_html = "\r\nStartHTML:";
    let h_end_html = "\r\nEndHTML:";
    let h_start_frag = "\r\nStartFragment:";
    let h_end_frag = "\r\nEndFragment:";
    let c_start_frag = "\r\n<html>\r\n<body>\r\n<!--StartFragment-->\r\n";
    let c_end_frag = "\r\n<!--EndFragment-->\r\n</body>\r\n</html>";
    let h_len = h_version.len()
        + h_start_html.len()
        + 10
        + h_end_html.len()
        + 10
        + h_start_frag.len()
        + 10
        + h_end_frag.len()
        + 10;
    let n_start_html = h_len + 2;
    let n_start_frag = h_len + c_start_frag.len();
    let n_end_frag = n_start_frag + ctn.len();
    let n_end_html = n_end_frag + c_end_frag.len();
    format!(
        "{h_version}{h_start_html}{n_start_html:010}{h_end_html}{n_end_html:010}{h_start_frag}{n_start_frag:010}{h_end_frag}{n_end_frag:010}{c_start_frag}{ctn}{c_end_frag}"
    )
}

/// Decode a PNG slice into a Windows `CF_DIBV5` payload (124-byte `BITMAPV5HEADER` + bottom-up BGRA pixels).
#[cfg(any(target_os = "windows", test))]
fn png_to_dibv5(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let rgba = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let width_i32 = i32::try_from(w).ok()?;
    let height_i32 = i32::try_from(h).ok()?;
    let pixel_bytes = w.checked_mul(h)?.checked_mul(4)?;
    let total_bytes = 124usize.checked_add(usize::try_from(pixel_bytes).ok()?)?;

    let mut out = Vec::with_capacity(total_bytes);
    out.extend_from_slice(&124u32.to_le_bytes()); // bV5Size
    out.extend_from_slice(&width_i32.to_le_bytes()); // bV5Width
    out.extend_from_slice(&height_i32.to_le_bytes()); // bV5Height (positive = bottom-up DIB)
    out.extend_from_slice(&1u16.to_le_bytes()); // bV5Planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bV5BitCount
    out.extend_from_slice(&3u32.to_le_bytes()); // bV5Compression = BI_BITFIELDS
    out.extend_from_slice(&pixel_bytes.to_le_bytes()); // bV5SizeImage
    out.extend_from_slice(&[0u8; 16]); // XPelsPerMeter, YPelsPerMeter, ClrUsed, ClrImportant
    out.extend_from_slice(&0x00FF_0000u32.to_le_bytes()); // bV5RedMask
    out.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // bV5GreenMask
    out.extend_from_slice(&0x0000_00FFu32.to_le_bytes()); // bV5BlueMask
    out.extend_from_slice(&0xFF00_0000u32.to_le_bytes()); // bV5AlphaMask
    out.extend_from_slice(&0x7352_4742u32.to_le_bytes()); // bV5CSType = LCS_sRGB
    out.extend_from_slice(&[0u8; 36]); // bV5Endpoints
    out.extend_from_slice(&[0u8; 12]); // bV5GammaRed/Green/Blue
    out.extend_from_slice(&4u32.to_le_bytes()); // bV5Intent = LCS_GM_IMAGES
    out.extend_from_slice(&[0u8; 12]); // bV5ProfileData, bV5ProfileSize, bV5Reserved
    for y in (0..h).rev() {
        for x in 0..w {
            let [r, g, b, a] = rgba.get_pixel(x, y).0;
            out.extend_from_slice(&[b, g, r, a]);
        }
    }
    Some(out)
}

/// Write `PNG`, `CF_DIBV5`, `HTML Format`, and `CF_UNICODETEXT` to the Win32 clipboard in one
/// `EmptyClipboard` session so Windows apps like Zalo PC show a table image preview while still
/// offering text paste (matching Microsoft Excel).
#[cfg(target_os = "windows")]
fn write_windows_clipboard(html: &str, text: &str, png: Option<&[u8]>) -> Result<(), String> {
    let _clip = clipboard_win::Clipboard::new_attempts(10).map_err(|e| e.to_string())?;
    clipboard_win::raw::empty().map_err(|e| e.to_string())?;

    if let Some(png_bytes) = png {
        if let Some(png_fmt) = clipboard_win::register_format("PNG") {
            let _ = clipboard_win::raw::set_without_clear(png_fmt.get(), png_bytes);
        }
        if let Some(dib) = png_to_dibv5(png_bytes) {
            let _ = clipboard_win::raw::set_without_clear(clipboard_win::formats::CF_DIBV5, &dib);
        }
    }
    let ok_html = clipboard_win::register_format("HTML Format").is_some_and(|fmt| {
        let cf_html = wrap_win_cf_html(html);
        clipboard_win::raw::set_without_clear(fmt.get(), cf_html.as_bytes()).is_ok()
    });
    let ok_text = clipboard_win::raw::set_string_with(text, clipboard_win::options::NoClear).is_ok();
    if ok_html || ok_text {
        Ok(())
    } else {
        Err("Win32 clipboard write failed".into())
    }
}

/// Read the system clipboard through the shared handle, creating it on first use.
fn read_clipboard<T>(
    handle: &RefCell<Option<arboard::Clipboard>>,
    read: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Option<T> {
    let mut clipboard = handle.try_borrow_mut().ok()?;
    if clipboard.is_none() {
        *clipboard = arboard::Clipboard::new().ok();
    }
    read(clipboard.as_mut()?).ok()
}

fn services() -> Services {
    // Keep the owner alive: X11/Wayland serve clipboard data from this handle.
    let clipboard: Rc<RefCell<Option<arboard::Clipboard>>> = Rc::default();
    let (rich_clipboard, html_clipboard, text_clipboard, image_clipboard) =
        (clipboard.clone(), clipboard.clone(), clipboard.clone(), clipboard.clone());
    Services {
        copy_rich: Some(Box::new(move |html, text, png| {
            #[cfg(target_os = "macos")]
            {
                let _ = &rich_clipboard;
                write_macos_clipboard(html, text, png)
            }
            #[cfg(target_os = "windows")]
            {
                let _ = &rich_clipboard;
                write_windows_clipboard(html, text, png)
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                let _ = png;
                let mut clipboard = rich_clipboard.borrow_mut();
                if clipboard.is_none() {
                    *clipboard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
                }
                clipboard.as_mut().ok_or("clipboard unavailable")?.set_html(html, Some(text)).map_err(|e| e.to_string())
            }
        })),
        copy_html: Some(Box::new(move |html, text| {
            let mut clipboard = clipboard.borrow_mut();
            if clipboard.is_none() {
                *clipboard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
            }
            clipboard.as_mut().ok_or("clipboard unavailable")?.set_html(html, Some(text)).map_err(|e| e.to_string())
        })),
        paste_html: Some(Box::new(move || read_clipboard(&html_clipboard, |clipboard| clipboard.get().html()))),
        paste_text: Some(Box::new(move || read_clipboard(&text_clipboard, |clipboard| clipboard.get_text()))),
        paste_image: Some(Box::new(move || {
            let image = read_clipboard(&image_clipboard, |clipboard| clipboard.get_image())?;
            image::RgbaImage::from_raw(u32::try_from(image.width).ok()?, u32::try_from(image.height).ok()?, image.bytes.into_owned())
        })),
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Spreadsheets", &["xlsx", "xlsm", "xls", "xlsb", "ods", "csv", "tsv", "txt", "slk", "json"])
                .add_filter("Excel Workbook", &["xlsx", "xlsm"])
                .add_filter("Excel 97-2003 Workbook (import)", &["xls"])
                .add_filter("Excel Binary Workbook (data import)", &["xlsb"])
                .add_filter("OpenDocument Spreadsheet (data import)", &["ods"])
                .add_filter("CSV", &["csv"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_picture: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Pictures (PNG, JPEG)", &["png", "jpg", "jpeg"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_save: Some(Box::new(|suggested: &str| {
            let path = std::path::Path::new(suggested);
            let stem = path.file_stem().and_then(std::ffi::OsStr::to_str).unwrap_or("Book1").to_string();
            // The suggested name's type (an export asks for .pdf, .csv or .html) comes first.
            const TYPES: [(&str, &str); 4] =
                [("Excel Workbook (.xlsx)", "xlsx"), ("CSV UTF-8 (.csv)", "csv"), ("Web Page (.html)", "html"), ("PDF (.pdf)", "pdf")];
            let ext = path.extension().and_then(std::ffi::OsStr::to_str).map(str::to_ascii_lowercase).filter(|e| TYPES.iter().any(|(_, x)| x == e));
            let ext = ext.unwrap_or_else(|| "xlsx".to_string());
            let mut dialog = rfd::FileDialog::new().set_file_name(format!("{stem}.{ext}"));
            for (label, x) in TYPES.iter().filter(|(_, x)| *x == ext).chain(TYPES.iter().filter(|(_, x)| *x != ext)) {
                dialog = dialog.add_filter(*label, &[*x]);
            }
            dialog.save_file().and_then(|p| p.to_str().map(str::to_string))
        })),
        open_url: Some(Box::new(|url: &str| open_with_system(url))),
        printers: Some(print::discover()),
        print_native: Some(print::submit()),
        ..Default::default()
    }
}

/// Opens a URL or file with the system's default application.
fn open_with_system(target: &str) {
    // Last line of defence behind `link.open` and the click prompt: the system only ever gets
    // http, https and mailto addresses.
    let gridcraft_engine::links::LinkKind::External(target) = gridcraft_engine::links::classify(target) else {
        log::warn!("refused to open {target:?}: only http, https and mailto links are opened");
        return;
    };
    let target = target.as_str();
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(cmd).arg(target).spawn() {
        log::warn!("{cmd} {target}: {e}");
    }
}

fn icon() -> Option<egui::IconData> {
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../../../assets/app-icon/gridcraft-macos-512.png");
    #[cfg(not(target_os = "macos"))]
    let bytes = include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/ai.storyteller.gridcraft.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() })
}

fn main() -> eframe::Result<()> {
    // A confined launcher run (`gridcraft_sandbox::launch_command`) comes before everything else:
    // it opens no window, writes no log, and becomes the program it confines.
    let raw: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if let Some(code) = gridcraft_sandbox::handle_args(&raw) {
        std::process::exit(code);
    }
    // First, so the panic hook and every start-up warning are recorded (`logging`).
    let logger = logging::install();
    // Commands catch their panics (`Session::execute`), but the default hook only prints to
    // standard error; this one sends every panic, caught or not, to the log file too, with a
    // backtrace when RUST_BACKTRACE is set.
    std::panic::set_hook(Box::new(|info| {
        let trace = std::backtrace::Backtrace::capture();
        let report = if trace.status() == std::backtrace::BacktraceStatus::Captured {
            format!("internal error: {info}\n{trace}")
        } else {
            format!("internal error: {info}")
        };
        // Standard error and the log file; standard error alone when RUST_LOG turned errors off.
        if log::log_enabled!(log::Level::Error) {
            log::error!("{report}");
        } else {
            // `eprintln!` panics on a broken stderr pipe, and a panic inside the panic hook aborts.
            let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("gridcraft: {report}\n"));
        }
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The restart helper a self-update spawns: it finishes the swap and relaunches, then exits.
    if let Some(done) = craft_update::self_update::finish(&gridcraft_ui_egui::UPDATE_APP, &std::env::args_os().skip(1).collect::<Vec<_>>()) {
        if let Err(e) = done {
            eprintln!("GridCraft update helper failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("GridCraft {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let mut control_port: Option<u16> = std::env::var("GRIDCRAFT_CONTROL_PORT").ok().and_then(|p| p.parse().ok());
    let mut control_token: Option<String> = std::env::var("GRIDCRAFT_CONTROL_TOKEN").ok();
    let mut sample: Option<String> = None;
    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--control" => {
                i += 1;
                control_port = args.get(i).and_then(|p| p.parse().ok());
            }
            "--control-token" => {
                i += 1;
                control_token = args.get(i).cloned();
            }
            "--sample" => {
                i += 1;
                sample = Some(args.get(i).cloned().unwrap_or_else(|| "sales".into()));
            }
            a if a.starts_with("-psn_") => {}
            a => files.push(a.to_string()),
        }
        i += 1;
    }
    // The log file lives in the settings folder, next to the preferences; opened after the
    // arguments, so `--version` leaves no file behind. Records logged until now are written to it
    // first. Runs without preferences (agents' test runs) log to standard error only, so they
    // don't rotate away the user's own logs.
    if let Some(logger) = logger {
        match log_dir().filter(|_| prefs_enabled()) {
            Some(dir) => match logger.attach_dir(&dir) {
                Ok(path) => log::info!("GridCraft {}, log file {}", env!("CARGO_PKG_VERSION"), path.display()),
                // Standard error only by now (`attach_dir` gave up on the file); unlike `eprintln!`, never panics.
                Err(e) => log::warn!("no log file: {e}"),
            },
            None => logger.no_file(),
        }
    }
    let mut session = Session::new();
    if let Some(s) = &sample
        && let Err(e) = session.execute("file.new", json!({"sample": s}))
    {
        log::warn!("sample {s}: {e}");
    }
    let mut app = SheetApp::new(session, services());
    app.watch_desktop_theme();
    app.start_update_check();
    let control_port = control_port;
    load_prefs(&mut app);
    // Stamp the startup workbook's author from the persisted user name.
    if app.session.documents().is_empty() {
        app.session.new_workbook();
    }
    app.after_engine(); // Show warnings from the sample.
    // Files named on the command line (a double-click on Windows and Linux) open as File › Open
    // opens them: a password-protected one asks for its password, and errors are shown, not
    // only logged. Each replaces the untouched blank Book1, as a file opened from Finder does.
    for f in &files {
        app.open_path(f);
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("GridCraft")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([640.0, 420.0])
        .with_app_id("ai.storyteller.gridcraft")
        .with_drag_and_drop(true);
    if cfg!(target_os = "macos") {
        viewport = viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false);
    }
    if let Some(i) = icon() {
        viewport = viewport.with_icon(i);
    }
    let window_builder = viewport.clone();
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(unix)]
    let mut options = options;
    // Intel Macs: Metal's compiler aborts on wgpu's indirect-validation shaders (see graphics.rs).
    #[cfg(target_os = "macos")]
    graphics::configure_indirect_validation(&mut options, graphics::skips_indirect_validation(true, cfg!(target_arch = "x86_64")));
    // winit has no file drag-and-drop on Wayland (only on X11), so dropping files from the file
    // manager showed a "no" cursor. Run through XWayland when it's there; GRIDCRAFT_WAYLAND=1
    // keeps the native Wayland backend.
    #[cfg(all(unix, not(target_os = "macos")))]
    if std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty()) && std::env::var_os("GRIDCRAFT_WAYLAND").is_none() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::x11::EventLoopBuilderExtX11;
            b.with_x11();
        }));
    }
    // Before eframe creates the wgpu instance: Windows starts on DirectX 12, or OpenGL when
    // DirectX 12 has no GPU driver, and never touches Vulkan unless asked (see graphics.rs).
    #[cfg(target_os = "windows")]
    let backend_override = eframe::wgpu::Backends::from_env();
    #[cfg(target_os = "windows")]
    let (options, first_backends) = {
        let mut options = options;
        let backends = graphics::configure(&mut options, backend_override);
        (options, backends)
    };
    #[cfg(target_os = "windows")]
    let retry_options = options.clone();
    // Registered before the event loop starts, so it catches the Finder event that launched us as
    // well as later ones. Lives until the event loop returns; the app creator only borrows it.
    #[cfg(target_os = "macos")]
    let apple_events = apple_events::AppleEvents::install();
    #[cfg(target_os = "macos")]
    let apple_events = &apple_events;
    // The app waits here until a start gets as far as creating it; a start that fails before
    // (no graphics adapter, device or surface) leaves it here for the next attempt.
    let pending = std::cell::Cell::new(Some(app));
    let run = |options: eframe::NativeOptions| {
        eframe::run_native(
            "GridCraft",
            options,
            Box::new(|cc| {
                let Some(mut app) = pending.take() else {
                    return Err("the app was already created".into());
                };
                SheetApp::setup_context_for_language(&cc.egui_ctx, app.ui.dark, app.ui.language);
                if let Some(port) = control_port {
                    match control_server::start(port, control_token.clone(), cc.egui_ctx.clone()) {
                        Ok(rx) => app.control_rx = Some(rx),
                        Err(e) => log::error!("control channel on port {port}: {e}"),
                    }
                }
                let windows = DocWindows::new(&mut app, window_builder.clone(), DocWindows::platform_quits_with_last_window());
                Ok(Box::new(App(
                    app,
                    windows,
                    #[cfg(target_os = "macos")]
                    None,
                    #[cfg(target_os = "macos")]
                    apple_events.connect(&cc.egui_ctx),
                )))
            }),
        )
    };
    let result = run(options);
    // A graphics error before the app existed: start once more with the other backend (eframe
    // reuses its event loop for the second run). Errors after that, or from winit, are returned.
    #[cfg(target_os = "windows")]
    if let Err(eframe::Error::Wgpu(e)) = &result
        && let Some(app) = pending.take()
        && let Some(backends) = graphics::fallback_backends(backend_override, first_backends)
    {
        log::error!("graphics: the renderer didn't start on {first_backends:?} ({e}); trying {backends:?}");
        pending.set(Some(app));
        let mut options = retry_options;
        graphics::configure_backends(&mut options, backends);
        return run(options);
    }
    result
}

#[cfg(test)]
mod packaging_tests {
    use gridcraft_engine::io::FileKind;

    /// The text between `start` and `end` that follows each `marker` in `text`, sorted (e.g. the
    /// `xlsb` in `Classes\.xlsb\OpenWithProgids`).
    fn extensions(text: &str, marker: &str, start: &str, end: char) -> Vec<String> {
        let mut v: Vec<String> = text.split(marker).skip(1).filter_map(|s| s.split_once(start)?.1.split(end).next()).map(str::to_string).collect();
        v.sort();
        v
    }

    /// macOS Open With (`CFBundleDocumentTypes`) offers GridCraft for the same files as Windows
    /// Open with, and GridCraft opens each of them. #321 added `.xlsb` and `.ods` on Windows and
    /// Linux only.
    #[test]
    fn macos_and_windows_offer_the_same_file_types() {
        let plist = include_str!("../../../packaging/macos/Info.plist.in");
        let wxs = include_str!("../../../packaging/windows/gridcraft.wxs");
        let mac = extensions(plist, "<key>CFBundleTypeExtensions</key>", "<string>", '<');
        let win = extensions(wxs, "Key=\"Software\\Classes\\.", "", '\\');
        assert!(mac.iter().any(|e| e == "xlsx"), "Info.plist.in extensions not found: {mac:?}");
        assert_eq!(mac, win);
        for ext in &mac {
            assert!(FileKind::from_path(&format!("book.{ext}")).is_some(), "GridCraft doesn't open .{ext}");
        }
    }

    #[test]
    fn cf_html_header_offsets_match_fragment_boundaries() -> Result<(), String> {
        let frag = "<table><tr><td>Xin chào Định Sơn</td></tr></table>";
        let wrapped = super::wrap_win_cf_html(frag);
        let bytes = wrapped.as_bytes();
        let parse_offset = |label: &str| -> Result<usize, String> {
            let line = wrapped
                .lines()
                .find(|l| l.starts_with(label))
                .ok_or_else(|| format!("missing header {label}"))?;
            line.trim_start_matches(label).parse::<usize>().map_err(|e| e.to_string())
        };
        let start_html = parse_offset("StartHTML:")?;
        let end_html = parse_offset("EndHTML:")?;
        let start_frag = parse_offset("StartFragment:")?;
        let end_frag = parse_offset("EndFragment:")?;
        assert!(bytes.get(start_html..start_html + 6) == Some(b"<html>"));
        assert!(bytes.get(end_html - 7..end_html) == Some(b"</html>"));
        assert!(bytes.get(start_frag..end_frag) == Some(frag.as_bytes()));
        Ok(())
    }

    #[test]
    fn png_to_dibv5_produces_valid_124_byte_header_and_bottom_up_bgra() -> Result<(), String> {
        let raw = vec![
            255, 0, 0, 255, 0, 255, 0, 128, // top row: red, semi-transparent green
            0, 0, 255, 255, 255, 255, 255, 255, // bottom row: blue, white
        ];
        let img = image::RgbaImage::from_raw(2, 2, raw).ok_or("failed to build RGBA image")?;
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        let dib = super::png_to_dibv5(&png.into_inner()).ok_or("png_to_dibv5 returned None")?;
        assert_eq!(dib.len(), 124 + 2 * 2 * 4);
        assert_eq!(&dib[0..4], &124u32.to_le_bytes());
        assert_eq!(&dib[4..8], &2i32.to_le_bytes());
        assert_eq!(&dib[8..12], &2i32.to_le_bytes());
        assert_eq!(&dib[14..16], &32u16.to_le_bytes());
        assert_eq!(&dib[16..20], &3u32.to_le_bytes());
        assert_eq!(&dib[124..140], &[255, 0, 0, 255, 255, 255, 255, 255, 0, 0, 255, 255, 0, 255, 0, 128]);
        Ok(())
    }
}
