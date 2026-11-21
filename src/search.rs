use crate::locale::{loc, normalize, Locale};

pub const MAX_RESULTS: usize = 300;

pub fn is_base(e: &emojis::Emoji) -> bool {
    matches!(e.skin_tone(), None | Some(emojis::SkinTone::Default))
}

pub fn token_score(t: &str, fields: &[&str]) -> Option<u32> {
    let is_word = |s: &str| {
        s.split(|c: char| !c.is_alphanumeric())
            .any(|w| w.starts_with(t))
    };
    if fields.contains(&t) {
        return Some(0);
    }
    if fields.iter().any(|f| is_word(f)) {
        return Some(1);
    }
    if fields.iter().any(|f| f.contains(t)) {
        return Some(2);
    }
    None
}

pub fn search(
    query: &str,
    recents: &[String],
    show_recent: bool,
    locale: Option<&Locale>,
) -> Vec<&'static emojis::Emoji> {
    let q = normalize(query.trim());

    if q.is_empty() {
        let mut out: Vec<&'static emojis::Emoji> = Vec::new();
        if show_recent {
            out.extend(recents.iter().filter_map(|r| emojis::get(r.as_str())));
        }
        for e in emojis::iter().filter(|e| is_base(e)) {
            if !out.iter().any(|x| x.as_str() == e.as_str()) {
                out.push(e);
            }
        }
        out.truncate(MAX_RESULTS);
        return out;
    }

    let tokens: Vec<&str> = q.split_whitespace().collect();
    let mut scored: Vec<(u32, usize, usize, &'static emojis::Emoji)> = Vec::new();

    for e in emojis::iter().filter(|e| is_base(e)) {
        let name = e.name().to_lowercase();
        let codes: Vec<String> = e.shortcodes().map(|s| s.to_lowercase()).collect();
        let mut fields: Vec<&str> = vec![name.as_str()];
        fields.extend(codes.iter().map(String::as_str));
        if let Some(l) = loc(locale, e) {
            fields.extend(l.words.iter().map(String::as_str));
        }

        let mut total = 0u32;
        let mut matched = true;
        for t in &tokens {
            match token_score(t, &fields) {
                Some(s) => total += s,
                None => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            let recent_pos = if show_recent {
                recents
                    .iter()
                    .position(|r| r == e.as_str())
                    .unwrap_or(usize::MAX)
            } else {
                usize::MAX
            };
            scored.push((total, recent_pos, name.len(), e));
        }
    }

    scored.sort_by_key(|(s, r, l, _)| (*s, *r, *l));
    scored.into_iter().take(MAX_RESULTS).map(|x| x.3).collect()
}
