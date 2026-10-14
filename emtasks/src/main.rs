mod app;
mod csvio;
mod datepicker;
mod history;
mod model;
mod platform;
mod storage;
mod theme;
mod timeutil;
mod ui;

/// A released app has no visible stderr, so record crashes next to the data
/// (File → Show data in Finder) where a user can find and send them.
fn install_crash_log() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        use std::io::Write;
        let dir = storage::Storage::default_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(storage::CRASH_LOG)) {
            let _ = writeln!(
                f,
                "── {} · emTasks {} ──\n{info}\n{}\n",
                timeutil::fmt_stamp(timeutil::now()),
                env!("CARGO_PKG_VERSION"),
                std::backtrace::Backtrace::force_capture()
            );
        }
    }));
}

fn main() -> eframe::Result {
    install_crash_log();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("emTasks")
            .with_inner_size([1200.0, 780.0])
            .with_min_inner_size([820.0, 520.0])
            // eframe swaps in its own default Dock icon unless we hand it ours.
            .with_icon(std::sync::Arc::new(
                eframe::icon_data::from_png_bytes(include_bytes!("../packaging/icon-1024.png")).unwrap_or_default(),
            )),
        ..Default::default()
    };
    eframe::run_native(
        "emTasks",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
