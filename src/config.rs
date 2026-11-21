use std::fs;
use std::path::PathBuf;

use gtk4::glib;

#[derive(Clone)]
pub struct Config {
    pub recent_enabled: bool,
    pub recent_max: u32,
    // код применённой локализации, пусто — только английский
    pub locale: String,
    pub dark: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            recent_enabled: true,
            recent_max: 24,
            locale: String::new(),
            dark: false,
        }
    }
}

pub fn config_file() -> PathBuf {
    glib::user_config_dir().join("emoji-picker").join("config")
}

pub fn recent_file() -> PathBuf {
    glib::user_data_dir().join("emoji-picker").join("recent")
}

pub fn write_file(path: PathBuf, content: String) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, content)
}

pub fn valid_code(code: &str) -> bool {
    !code.is_empty() && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn load_config() -> Config {
    let mut cfg = Config::default();
    if let Ok(text) = fs::read_to_string(config_file()) {
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k.trim() {
                "recent_enabled" => {
                    if let Ok(b) = v.trim().parse::<bool>() {
                        cfg.recent_enabled = b;
                    }
                }
                "recent_max" => {
                    if let Ok(n) = v.trim().parse::<u32>() {
                        cfg.recent_max = n.clamp(1, 200);
                    }
                }
                "dark" => {
                    if let Ok(b) = v.trim().parse::<bool>() {
                        cfg.dark = b;
                    }
                }
                "locale" if valid_code(v.trim()) => cfg.locale = v.trim().to_string(),
                _ => {}
            }
        }
    }
    cfg
}

pub fn save_config(cfg: &Config) {
    let _ = write_file(
        config_file(),
        format!(
            "recent_enabled={}\nrecent_max={}\nlocale={}\ndark={}\n",
            cfg.recent_enabled, cfg.recent_max, cfg.locale, cfg.dark
        ),
    );
}

pub fn load_recents() -> Vec<String> {
    fs::read_to_string(recent_file())
        .map(|t| {
            t.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

pub fn save_recents(recents: &[String]) {
    let _ = write_file(recent_file(), recents.join("\n") + "\n");
}
