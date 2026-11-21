use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::sync::OnceLock;

use gtk::prelude::*;
use gtk::{gio, glib};
use gtk4 as gtk;

use crate::config::{save_config, valid_code};
use crate::locale::{download_locale, installed_locales, load_locale, locale_file, KNOWN_LOCALES};
use crate::ui::State;

pub struct Page {
    pub root: gtk::Box,
    // куда ставить фокус при открытии настроек
    pub first: gtk::Widget,
}

// при выключении возвращается то, что было в системных настройках GTK
pub fn apply_theme(dark: bool) {
    static SYSTEM_DARK: OnceLock<bool> = OnceLock::new();
    if let Some(s) = gtk::Settings::default() {
        let system = *SYSTEM_DARK.get_or_init(|| s.is_gtk_application_prefer_dark_theme());
        s.set_gtk_application_prefer_dark_theme(dark || system);
    }
}

pub fn build(state: &Rc<RefCell<State>>, refresh: Rc<dyn Fn()>) -> Page {
    // ── страница настроек ──
    let settings_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    settings_box.set_margin_top(20);
    settings_box.set_margin_bottom(12);
    settings_box.set_margin_start(20);
    settings_box.set_margin_end(20);

    let title = gtk::Label::builder().label("Настройки").xalign(0.0).build();
    title.add_css_class("title");

    let dark_switch = gtk::Switch::new();
    dark_switch.set_valign(gtk::Align::Center);
    dark_switch.set_active(state.borrow().config.dark);

    let row_dark = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row_dark.append(
        &gtk::Label::builder()
            .label("Тёмная тема")
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    row_dark.append(&dark_switch);

    let switch = gtk::Switch::new();
    switch.set_valign(gtk::Align::Center);
    switch.set_active(state.borrow().config.recent_enabled);

    let spin = gtk::SpinButton::with_range(1.0, 200.0, 1.0);
    spin.set_value(state.borrow().config.recent_max as f64);
    spin.set_sensitive(switch.is_active());

    let row1 = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row1.append(
        &gtk::Label::builder()
            .label("Показывать недавние emoji")
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    row1.append(&switch);

    let row2 = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row2.append(
        &gtk::Label::builder()
            .label("Максимум недавних")
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    row2.append(&spin);

    let langs_title = gtk::Label::builder()
        .label("Локализации названий")
        .xalign(0.0)
        .build();
    langs_title.add_css_class("subtitle");

    let langs = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    let langs_scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&langs)
        .build();

    let custom_entry = gtk::Entry::builder()
        .placeholder_text("другой код языка CLDR, например sr_Latn")
        .hexpand(true)
        .build();
    let custom_button = gtk::Button::with_label("Скачать");
    let row3 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row3.append(&custom_entry);
    row3.append(&custom_button);

    let lang_status = gtk::Label::builder().xalign(0.0).wrap(true).build();
    lang_status.add_css_class("code");

    let settings_hint = gtk::Label::builder()
        .label("Esc или Ctrl+, — назад")
        .xalign(0.0)
        .build();
    settings_hint.add_css_class("hint");

    settings_box.append(&title);
    settings_box.append(&row_dark);
    settings_box.append(&row1);
    settings_box.append(&row2);
    settings_box.append(&langs_title);
    settings_box.append(&langs_scrolled);
    settings_box.append(&row3);
    settings_box.append(&lang_status);
    settings_box.append(&settings_hint);

    // ── обработчики ──
    {
        let state = state.clone();
        dark_switch.connect_active_notify(move |sw| {
            let mut st = state.borrow_mut();
            st.config.dark = sw.is_active();
            save_config(&st.config);
            apply_theme(st.config.dark);
        });
    }
    {
        let state = state.clone();
        let refresh = refresh.clone();
        let spin = spin.clone();
        switch.connect_active_notify(move |sw| {
            let on = sw.is_active();
            spin.set_sensitive(on);
            {
                let mut st = state.borrow_mut();
                st.config.recent_enabled = on;
                save_config(&st.config);
            }
            refresh();
        });
    }
    {
        let state = state.clone();
        let refresh = refresh.clone();
        spin.connect_value_changed(move |s| {
            {
                let mut st = state.borrow_mut();
                st.config.recent_max = s.value() as u32;
                save_config(&st.config);
            }
            refresh();
        });
    }

    let lang_ui = Rc::new(LangUi {
        state: state.clone(),
        list: langs.clone(),
        status: lang_status.clone(),
        refresh: refresh.clone(),
    });
    fill_langs(&lang_ui);
    {
        let download_custom: Rc<dyn Fn()> = {
            let lang_ui = lang_ui.clone();
            let custom_entry = custom_entry.clone();
            Rc::new(move || {
                let code = custom_entry.text().trim().to_string();
                custom_entry.set_text("");
                start_download(&lang_ui, code);
            })
        };
        let d = download_custom.clone();
        custom_button.connect_clicked(move |_| d());
        custom_entry.connect_activate(move |_| download_custom());
    }

    Page {
        root: settings_box,
        first: dark_switch.upcast(),
    }
}

// ── локализации ──

pub struct LangUi {
    state: Rc<RefCell<State>>,
    list: gtk::ListBox,
    status: gtk::Label,
    refresh: Rc<dyn Fn()>,
}

pub fn fill_langs(ui: &Rc<LangUi>) {
    while let Some(child) = ui.list.first_child() {
        ui.list.remove(&child);
    }

    let installed = installed_locales();
    let active = ui.state.borrow().config.locale.clone();

    let mut codes: Vec<String> = KNOWN_LOCALES.iter().map(|(c, _)| c.to_string()).collect();
    for c in &installed {
        if !codes.contains(c) {
            codes.push(c.clone());
        }
    }

    for code in codes {
        let title = KNOWN_LOCALES
            .iter()
            .find(|(c, _)| *c == code)
            .map_or(code.clone(), |(_, n)| format!("{n} ({code})"));
        let is_installed = installed.contains(&code);
        let is_active = active == code;

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(4);
        row.set_margin_bottom(4);
        row.set_margin_start(8);
        row.set_margin_end(8);
        row.append(
            &gtk::Label::builder()
                .label(&title)
                .xalign(0.0)
                .hexpand(true)
                .build(),
        );

        let note = match (is_installed, is_active) {
            (_, true) => "применён",
            (true, false) => "скачан",
            _ => "",
        };
        let note_label = gtk::Label::new(Some(note));
        note_label.add_css_class("code");
        row.append(&note_label);

        let button = |label: &str, action: Box<dyn Fn()>| {
            let b = gtk::Button::with_label(label);
            b.connect_clicked(move |_| action());
            row.append(&b);
        };

        if !is_installed {
            let (ui, code) = (ui.clone(), code.clone());
            button(
                "Скачать",
                Box::new(move || start_download(&ui, code.clone())),
            );
        } else {
            if is_active {
                let ui = ui.clone();
                button("Снять", Box::new(move || apply_locale(&ui, "")));
            } else {
                let (ui, code) = (ui.clone(), code.clone());
                button("Применить", Box::new(move || apply_locale(&ui, &code)));
            }
            let (ui, code) = (ui.clone(), code.clone());
            button("Удалить", Box::new(move || delete_locale(&ui, &code)));
        }

        let lrow = gtk::ListBoxRow::new();
        lrow.set_activatable(false);
        lrow.set_child(Some(&row));
        ui.list.append(&lrow);
    }
}

pub fn apply_locale(ui: &Rc<LangUi>, code: &str) {
    let ok = {
        let mut st = ui.state.borrow_mut();
        st.locale = if code.is_empty() {
            None
        } else {
            load_locale(code)
        };
        st.config.locale = if st.locale.is_some() {
            code.to_string()
        } else {
            String::new()
        };
        save_config(&st.config);
        code.is_empty() || st.locale.is_some()
    };
    ui.status.set_text(if ok {
        ""
    } else {
        "файл локализации повреждён, скачай заново"
    });
    (ui.refresh)();
    fill_langs(ui);
}

pub fn delete_locale(ui: &Rc<LangUi>, code: &str) {
    let _ = fs::remove_file(locale_file(code));
    if ui.state.borrow().config.locale == code {
        apply_locale(ui, "");
    } else {
        fill_langs(ui);
    }
}

pub fn start_download(ui: &Rc<LangUi>, code: String) {
    if !valid_code(&code) {
        ui.status
            .set_text("код языка: латиница, цифры и «_», например sr_Latn");
        return;
    }
    ui.status.set_text(&format!("скачивание «{code}»…"));
    ui.list.set_sensitive(false);

    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let c = code.clone();
        let res = gio::spawn_blocking(move || download_locale(&c)).await;
        ui.list.set_sensitive(true);
        match res {
            Ok(Ok(())) => ui.status.set_text(&format!("«{code}» скачан")),
            Ok(Err(e)) => ui.status.set_text(&e),
            Err(_) => ui.status.set_text("ошибка при скачивании"),
        }
        fill_langs(&ui);
    });
}
