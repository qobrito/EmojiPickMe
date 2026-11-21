use std::cell::{Cell, RefCell};
use std::process::{Command, Stdio};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use gtk4 as gtk;
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};

use crate::config::{load_config, load_recents, save_recents, Config};
use crate::locale::{load_locale, loc, Locale};
use crate::search::search;
use crate::settings;

pub struct State {
    pub config: Config,
    pub recents: Vec<String>,
    pub results: Vec<&'static emojis::Emoji>,
    pub locale: Option<Locale>,
}

pub fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "
        .search { font-size: 20px; min-height: 40px; margin: 10px 10px 4px 10px; }
        .emoji  { font-size: 28px; }
        .name   { font-size: 16px; }
        .code   { font-size: 13px; opacity: 0.55; }
        .hint   { font-size: 12px; opacity: 0.6; margin: 2px 10px 8px 10px; }
        .title  { font-size: 20px; font-weight: bold; }
        .subtitle { font-size: 16px; font-weight: bold; }
        list row { border-radius: 8px; margin: 1px 6px; }
        ",
    );
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn make_row(e: &emojis::Emoji, locale: Option<&Locale>) -> gtk::ListBoxRow {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    b.set_margin_top(4);
    b.set_margin_bottom(4);
    b.set_margin_start(8);
    b.set_margin_end(8);

    let em = gtk::Label::new(Some(e.as_str()));
    em.add_css_class("emoji");
    em.set_width_chars(3);

    let names = gtk::Box::new(gtk::Orientation::Vertical, 0);
    names.set_hexpand(true);
    names.set_valign(gtk::Align::Center);

    let l = loc(locale, e);
    let name = gtk::Label::new(Some(l.map_or(e.name(), |l| l.name.as_str())));
    name.add_css_class("name");
    name.set_xalign(0.0);
    names.append(&name);

    if l.is_some() {
        let en = gtk::Label::new(Some(e.name()));
        en.add_css_class("code");
        en.set_xalign(0.0);
        names.append(&en);
    }

    b.append(&em);
    b.append(&names);

    if let Some(code) = e.shortcode() {
        let c = gtk::Label::new(Some(&format!(":{code}:")));
        c.add_css_class("code");
        b.append(&c);
    }

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&b));
    row
}

pub fn copy_to_clipboard(text: &str) {
    // wl-copy сам форкается в фон и держит буфер, пока его не заменят
    let ok = Command::new("wl-copy")
        .arg("--")
        .arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !ok {
        if let Some(display) = gdk::Display::default() {
            display.clipboard().set_text(text);
        }
    }
}

pub fn pick(window: &gtk::ApplicationWindow, state: &Rc<RefCell<State>>, list: &gtk::ListBox) {
    let Some(row) = list.selected_row() else {
        return;
    };
    let idx = row.index();
    if idx < 0 {
        return;
    }
    let emoji = match state.borrow().results.get(idx as usize) {
        Some(e) => e.as_str().to_string(),
        None => return,
    };

    copy_to_clipboard(&emoji);

    {
        let mut st = state.borrow_mut();
        if st.config.recent_enabled {
            st.recents.retain(|r| r != &emoji);
            st.recents.insert(0, emoji);
            let max = st.config.recent_max as usize;
            st.recents.truncate(max);
            save_recents(&st.recents);
        }
    }

    window.close();
}

pub fn move_selection(list: &gtk::ListBox, delta: i32) {
    let cur = list.selected_row().map(|r| r.index()).unwrap_or(-1);
    let target = if cur < 0 { 0 } else { (cur + delta).max(0) };
    if let Some(row) = list.row_at_index(target).or_else(|| list.selected_row()) {
        list.select_row(Some(&row));
        row.grab_focus();
    }
}

pub fn list_has_focus(window: &gtk::ApplicationWindow, list: &gtk::ListBox) -> bool {
    match gtk::prelude::GtkWindowExt::focus(window) {
        Some(w) => w.is_ancestor(list) || w == *list.upcast_ref::<gtk::Widget>(),
        None => false,
    }
}

pub fn focus_entry(entry: &gtk::Entry) {
    entry.grab_focus();
    entry.set_position(-1);
}

pub fn build_ui(app: &gtk::Application) {
    load_css();

    let config = load_config();
    let locale = if config.locale.is_empty() {
        None
    } else {
        load_locale(&config.locale)
    };
    let state = Rc::new(RefCell::new(State {
        config,
        recents: load_recents(),
        results: Vec::new(),
        locale,
    }));
    settings::apply_theme(state.borrow().config.dark);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Emoji Picker")
        .default_width(640)
        .default_height(480)
        .decorated(false)
        .build();

    // оверлей поверх всех окон, как dmenu/fuzzel: без якорей — по центру экрана
    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_namespace("emoji-picker");
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
    }

    // ── главная страница ──
    let entry = gtk::Entry::builder()
        .placeholder_text("Поиск emoji…")
        .build();
    entry.add_css_class("search");

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .build();
    let scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    list.set_adjustment(Some(&scrolled.vadjustment()));

    let hint = gtk::Label::builder()
        .label("Enter — копировать · ↑↓ — выбор · Tab — список ⇄ ввод · Ctrl+, — настройки · Esc — выход")
        .wrap(true)
        .xalign(0.0)
        .build();
    hint.add_css_class("hint");

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    main_box.append(&entry);
    main_box.append(&scrolled);
    main_box.append(&hint);

    let stack = gtk::Stack::new();
    stack.add_named(&main_box, Some("main"));
    stack.set_visible_child_name("main");
    window.set_child(Some(&stack));

    // ── перерисовка списка ──
    let refresh: Rc<dyn Fn()> = {
        let state = state.clone();
        let entry = entry.clone();
        let list = list.clone();
        let scrolled = scrolled.clone();
        Rc::new(move || {
            let q = entry.text().to_string();

            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let results = {
                let st = state.borrow();
                let n = st.recents.len().min(st.config.recent_max as usize);
                let results = search(
                    &q,
                    &st.recents[..n],
                    st.config.recent_enabled,
                    st.locale.as_ref(),
                );
                for e in &results {
                    list.append(&make_row(e, st.locale.as_ref()));
                }
                results
            };
            state.borrow_mut().results = results;

            if let Some(r) = list.row_at_index(0) {
                list.select_row(Some(&r));
            }
            scrolled.vadjustment().set_value(0.0);
        })
    };

    {
        let refresh = refresh.clone();
        entry.connect_changed(move |_| refresh());
    }

    // клик мышкой по строке
    {
        let window = window.clone();
        let state = state.clone();
        list.connect_row_activated(move |l, row| {
            l.select_row(Some(row));
            pick(&window, &state, l);
        });
    }

    let settings_page = settings::build(&state, refresh.clone());
    stack.add_named(&settings_page.root, Some("settings"));

    let toggle_settings: Rc<dyn Fn()> = {
        let stack = stack.clone();
        let entry = entry.clone();
        let first = settings_page.first.clone();
        Rc::new(move || {
            if stack.visible_child_name().as_deref() == Some("settings") {
                stack.set_visible_child_name("main");
                focus_entry(&entry);
            } else {
                stack.set_visible_child_name("settings");
                first.grab_focus();
            }
        })
    };

    // ── клавиатура ──
    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let window = window.clone();
        let stack = stack.clone();
        let entry = entry.clone();
        let list = list.clone();
        let state = state.clone();
        let toggle_settings = toggle_settings.clone();

        key.connect_key_pressed(move |_, keyval, _, mods| {
            let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
            let alt = mods.contains(gdk::ModifierType::ALT_MASK);
            let in_settings = stack.visible_child_name().as_deref() == Some("settings");

            if ctrl && keyval == gdk::Key::comma {
                toggle_settings();
                return glib::Propagation::Stop;
            }

            if in_settings {
                if keyval == gdk::Key::Escape {
                    toggle_settings();
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }

            let list_mode = list_has_focus(&window, &list);

            if keyval == gdk::Key::Escape {
                window.close();
                return glib::Propagation::Stop;
            }
            if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
                pick(&window, &state, &list);
                return glib::Propagation::Stop;
            }
            if keyval == gdk::Key::Down || (ctrl && keyval == gdk::Key::n) {
                move_selection(&list, 1);
                return glib::Propagation::Stop;
            }
            if keyval == gdk::Key::Up || (ctrl && keyval == gdk::Key::p) {
                move_selection(&list, -1);
                return glib::Propagation::Stop;
            }
            if keyval == gdk::Key::Tab || keyval == gdk::Key::ISO_Left_Tab {
                if list_mode {
                    focus_entry(&entry);
                } else if let Some(r) = list.selected_row().or_else(|| list.row_at_index(0)) {
                    list.select_row(Some(&r));
                    r.grab_focus();
                }
                return glib::Propagation::Stop;
            }

            if list_mode {
                // в режиме списка Left/Right не должны уводить фокус
                if keyval == gdk::Key::Left || keyval == gdk::Key::Right {
                    return glib::Propagation::Stop;
                }
                // печать/Backspace в режиме списка возвращают в поле ввода
                if !ctrl && !alt {
                    if keyval == gdk::Key::BackSpace {
                        focus_entry(&entry);
                        let n = entry.text().chars().count() as i32;
                        if n > 0 {
                            entry.delete_text(n - 1, n);
                        }
                        return glib::Propagation::Stop;
                    }
                    if let Some(c) = keyval.to_unicode() {
                        if !c.is_control() {
                            focus_entry(&entry);
                            let mut pos = entry.text().chars().count() as i32;
                            entry.insert_text(&c.to_string(), &mut pos);
                            entry.set_position(-1);
                            return glib::Propagation::Stop;
                        }
                    }
                }
            }

            glib::Propagation::Proceed
        });
    }
    window.add_controller(key);

    // закрываться при потере фокуса (как dmenu)
    {
        let had_focus = Rc::new(Cell::new(false));
        window.connect_is_active_notify(move |w| {
            if w.is_active() {
                had_focus.set(true);
            } else if had_focus.get() {
                w.close();
            }
        });
    }

    refresh();
    window.present();
    entry.grab_focus();
}
