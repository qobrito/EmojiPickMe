mod config;
mod locale;
mod search;
mod settings;
mod ui;

use gtk::prelude::*;
use gtk::{gio, glib};
use gtk4 as gtk;

const APP_ID: &str = "dev.poppy.emoji-picker";

fn main() -> glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(ui::build_ui);
    app.run()
}
