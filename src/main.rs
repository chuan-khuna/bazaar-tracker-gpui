//! Bazaar Tracker: a desktop companion for The Bazaar. Opens the window; the tracking lives in the
//! `bazaar-tracker` crate.

mod overlays;
mod shell;
mod theme;
mod tracking;
mod ui;

use gpui::{App, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

use gpui_kit_assets::AllAssets as Assets;
use shell::instance::{self, Launch};
use shell::settings::{Load, SettingsStore};
use tracking::service::Service;
use ui::{TrackerApp, frame::title_bar};

/// The window size when it opens and after "Reset layout".
pub const DEFAULT_WINDOW_SIZE: (f32, f32) = (1000., 640.);
/// The smallest the window can be made.
pub const MIN_WINDOW_SIZE: (f32, f32) = (820., 520.);

fn main() {
    let data_dir = match shell::data_dir::locate() {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("bazaar-tracker: cannot create the data folder: {error}");
            std::process::exit(1);
        }
    };

    // Only one copy runs per data folder: a second launch brings the first one's window forward.
    let guard = match instance::acquire(&data_dir) {
        Launch::First(guard) => guard,
        Launch::Second => return,
    };

    let (feed, log_rows) = tracking::log::feed::channel();
    if let Err(error) = shell::logging::init(&data_dir, feed) {
        eprintln!("bazaar-tracker: cannot write the log: {error}");
    }
    tracing::info!("data folder: {}", data_dir.display());

    let (settings, load) = SettingsStore::load(&data_dir);
    let startup_error = match load {
        Load::Unusable(problem) => {
            tracing::error!("{problem}; using the default settings");
            Some(format!("{problem}. Using the defaults."))
        }
        Load::Read | Load::Created => None,
    };

    // Follows the game on background threads until the app quits (dropping it stops them).
    let service = Service::start(&data_dir, &settings.get().log_path);
    let startup_error = [startup_error, service.error.clone()].into_iter().flatten().reduce(|first, second| format!("{first} {second}"));
    let link = service.link();

    gpui_platform::application().with_assets(Assets).run(move |cx: &mut App| {
        gpui_component::init(cx);
        ui::bind_keys(cx);

        let (width, height) = DEFAULT_WINDOW_SIZE;
        let (min_width, min_height) = MIN_WINDOW_SIZE;
        let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(min_width), px(min_height))),
                    titlebar: Some(title_bar::options()),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let app = TrackerApp::new(settings, startup_error, link, log_rows, cx);
                        window.focus(&app.focus_handle, cx);
                        TrackerApp::follow_main_window(window, cx);
                        app
                    })
                },
            )
            .expect("the window opens");

        // Closing the main window quits, the pop-out with it; closing the pop-out does not.
        let main_window = window.window_id();
        cx.on_window_closed(move |cx, closed| {
            if closed == main_window {
                cx.quit();
            }
        })
        .detach();

        // Another copy was launched: show and focus this window.
        let requests = guard.show_requests();
        cx.spawn(async move |cx| {
            // Keeps the single-instance lock for as long as the app runs.
            let _guard = guard;
            while requests.recv().await.is_ok() {
                window.update(cx, |_, window, _| window.activate_window()).ok();
            }
        })
        .detach();

        cx.activate(true);
    });

    drop(service);
}
