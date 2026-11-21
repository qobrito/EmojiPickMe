use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use gtk4::glib;

use crate::config::{valid_code, write_file};

pub const CLDR_URL: &str = "https://raw.githubusercontent.com/unicode-org/cldr/main/common";

// языки, которые всегда показываются в настройках; остальные — по коду CLDR
pub const KNOWN_LOCALES: &[(&str, &str)] = &[
    ("ru", "Русский"),
    ("uk", "Українська"),
    ("be", "Беларуская"),
    ("kk", "Қазақ тілі"),
    ("pl", "Polski"),
    ("cs", "Čeština"),
    ("de", "Deutsch"),
    ("fr", "Français"),
    ("es", "Español"),
    ("it", "Italiano"),
    ("pt", "Português"),
    ("nl", "Nederlands"),
    ("sv", "Svenska"),
    ("fi", "Suomi"),
    ("tr", "Türkçe"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("zh", "中文"),
];

pub struct Loc {
    pub name: String,
    // название и ключевые слова, нормализованные для поиска
    pub words: Vec<String>,
}

pub type Locale = HashMap<String, Loc>;

pub fn locales_dir() -> PathBuf {
    glib::user_data_dir().join("emoji-picker").join("locales")
}

pub fn locale_file(code: &str) -> PathBuf {
    locales_dir().join(format!("{code}.tsv"))
}

pub fn normalize(s: &str) -> String {
    s.to_lowercase().replace('ё', "е")
}

pub fn emoji_key(s: &str) -> String {
    s.replace('\u{FE0F}', "")
}

pub fn installed_locales() -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(locales_dir())
        .map(|dir| {
            dir.filter_map(|e| {
                let name = e.ok()?.file_name().into_string().ok()?;
                let code = name.strip_suffix(".tsv")?;
                valid_code(code).then(|| code.to_string())
            })
            .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

// файл: emoji \t название \t ключевые|слова
pub fn load_locale(code: &str) -> Option<Locale> {
    let text = fs::read_to_string(locale_file(code)).ok()?;
    let map: Locale = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let cp = parts.next()?;
            let name = parts.next()?;
            let mut words = vec![normalize(name)];
            words.extend(
                parts
                    .next()
                    .unwrap_or("")
                    .split('|')
                    .filter(|k| !k.is_empty())
                    .map(normalize),
            );
            Some((
                cp.to_string(),
                Loc {
                    name: name.to_string(),
                    words,
                },
            ))
        })
        .collect();
    (!map.is_empty()).then_some(map)
}

pub fn fetch(url: &str) -> Result<String, String> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "60", url])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "не найден curl".to_string())?;
    if !out.status.success() {
        return Err(format!("не удалось скачать {url}"));
    }
    String::from_utf8(out.stdout).map_err(|_| "ответ не в UTF-8".to_string())
}

pub fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

// строки вида <annotation cp="👍" type="tts">название</annotation>
// и <annotation cp="👍">слово | слово</annotation>
pub fn parse_annotations(
    xml: &str,
    names: &mut HashMap<String, String>,
    words: &mut HashMap<String, String>,
) {
    for line in xml.lines() {
        let Some(rest) = line.trim().strip_prefix("<annotation cp=\"") else {
            continue;
        };
        let Some((cp, rest)) = rest.split_once('"') else {
            continue;
        };
        let Some((attrs, rest)) = rest.split_once('>') else {
            continue;
        };
        let Some(text) = rest.strip_suffix("</annotation>") else {
            continue;
        };
        let cp = emoji_key(&xml_unescape(cp));
        // варианты с тоном кожи в списке не показываются
        let skin = |c: char| ('\u{1F3FB}'..='\u{1F3FF}').contains(&c);
        if cp.chars().count() > 1 && cp.chars().any(skin) {
            continue;
        }
        let text = xml_unescape(text.trim());
        // ↑↑↑ — «унаследовать от родительской локали», данных нет
        if text.is_empty() || text == "↑↑↑" {
            continue;
        }
        let target = if attrs.contains("type=\"tts\"") {
            &mut *names
        } else {
            &mut *words
        };
        target.entry(cp).or_insert(text);
    }
}

pub fn download_locale(code: &str) -> Result<(), String> {
    let main = fetch(&format!("{CLDR_URL}/annotations/{code}.xml"))
        .map_err(|_| format!("язык «{code}» не найден в CLDR или нет сети"))?;
    // производных аннотаций (флаги, профессии и т.п.) у части языков нет
    let derived = fetch(&format!("{CLDR_URL}/annotationsDerived/{code}.xml")).unwrap_or_default();

    let mut names = HashMap::new();
    let mut words = HashMap::new();
    parse_annotations(&main, &mut names, &mut words);
    parse_annotations(&derived, &mut names, &mut words);
    if names.is_empty() {
        return Err(format!("в CLDR нет названий emoji для «{code}»"));
    }

    let mut out = String::new();
    for (cp, name) in &names {
        let kw: Vec<&str> = words
            .get(cp)
            .map(|w| {
                w.split('|')
                    .map(str::trim)
                    .filter(|k| !k.is_empty() && k != name)
                    .collect()
            })
            .unwrap_or_default();
        out.push_str(&format!("{cp}\t{name}\t{}\n", kw.join("|")));
    }
    write_file(locale_file(code), out).map_err(|e| format!("не удалось сохранить: {e}"))
}

pub fn loc<'a>(locale: Option<&'a Locale>, e: &emojis::Emoji) -> Option<&'a Loc> {
    locale?.get(&emoji_key(e.as_str()))
}
