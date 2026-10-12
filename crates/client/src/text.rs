//! Everything the player reads comes from a text file per language (`crates/client/text/`),
//! looked up by key: `t("home.create")`, or `tf("lobby.wins", &[("name", "Sam")])` to fill in
//! `{name}`. Only English (`en-US`) exists so far; another language is another file with the
//! same keys (anything it lacks falls back to English).
//!
//! The test below checks the code and `en-US.txt` agree: every key used exists, and every key
//! in the file is used.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// The languages built in: (code, file contents). The first is the fallback.
const LANGUAGES: &[(&str, &str)] = &[("en-US", include_str!("../text/en-US.txt"))];

struct Table {
    /// Index into `LANGUAGES`.
    current: usize,
    maps: Vec<HashMap<&'static str, &'static str>>,
}

fn table() -> &'static RwLock<Table> {
    static TABLE: OnceLock<RwLock<Table>> = OnceLock::new();
    TABLE.get_or_init(|| RwLock::new(Table { current: 0, maps: LANGUAGES.iter().map(|(_, text)| parse(text)).collect() }))
}

/// `key = text` lines; `#` starts a comment line; blank lines are ignored.
fn parse(text: &'static str) -> HashMap<&'static str, &'static str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.trim(), v.trim())))
        .collect()
}

/// The language codes available, e.g. for a picker.
#[allow(dead_code)]
pub fn languages() -> impl Iterator<Item = &'static str> {
    LANGUAGES.iter().map(|(code, _)| *code)
}

/// Switches language (unknown codes keep the current one).
pub fn set_language(code: &str) {
    if let Some(i) = LANGUAGES.iter().position(|(c, _)| *c == code) {
        table().write().unwrap().current = i;
    }
}

/// The text for `key` in the current language, else English, else "???" (a missing text shows
/// up on screen rather than crashing; the tests make sure the code uses no missing keys).
pub fn t(key: &str) -> &'static str {
    let tab = table().read().unwrap();
    tab.maps[tab.current].get(key).or_else(|| tab.maps[0].get(key)).copied().unwrap_or("???")
}

/// `t(key)` with `{name}` placeholders filled in.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key).to_string();
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    /// Keys the code builds at run time from an id, rather than spelling out: `t(&format!(..))`.
    /// Each prefix's keys are checked against the ids they're built from.
    fn built_keys() -> BTreeSet<String> {
        use go_box_protocol::Refusal;
        use go_box_sim::character::Character;
        let mut keys = BTreeSet::new();
        for c in Character::ALL {
            keys.insert(format!("character.{}", c.id()));
            keys.insert(format!("about.{}", c.id()));
            if let Some(s) = c.special() {
                keys.insert(format!("special.{}", s.id()));
            }
        }
        for a in go_box_sim::sim::ArenaId::ALL {
            keys.insert(format!("arena.{}", a.id()));
        }
        for r in Refusal::ALL {
            keys.insert(format!("refusal.{}", r.id()));
        }
        for k in crate::settings::KEY_NAMES.iter().filter_map(|(_, name)| name.strip_prefix("key.")) {
            keys.insert(format!("key.{k}"));
        }
        for a in crate::settings::Act::ALL {
            keys.insert(format!("act.{}", a.id()));
        }
        for p in crate::settings::PAD_NAMES {
            keys.insert(format!("pad.{p}"));
        }
        keys
    }

    /// Every `t("...")` / `tf("...", ..)` key in the client's source.
    fn keys_in_code() -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("text.rs") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    for pat in ["t(\"", "tf(\""] {
                        for (i, _) in src.match_indices(pat) {
                            // Only whole calls: `t(` not preceded by a letter, digit or `_`.
                            let before = src[..i].chars().last();
                            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                                continue;
                            }
                            let rest = &src[i + pat.len()..];
                            keys.insert(rest[..rest.find('"').unwrap()].to_string());
                        }
                    }
                }
            }
        }
        keys
    }

    #[test]
    fn code_and_english_text_agree() {
        let english: BTreeSet<String> = parse(LANGUAGES[0].1).keys().map(|k| k.to_string()).collect();
        let used: BTreeSet<String> = keys_in_code().into_iter().chain(built_keys()).collect();
        let missing: Vec<_> = used.difference(&english).collect();
        let unused: Vec<_> = english.difference(&used).collect();
        assert!(missing.is_empty(), "keys used in code but missing from en-US.txt: {missing:?}");
        assert!(unused.is_empty(), "keys in en-US.txt that nothing uses: {unused:?}");
    }

    #[test]
    fn every_language_has_only_known_keys() {
        let english = parse(LANGUAGES[0].1);
        for (code, text) in LANGUAGES {
            let extra: Vec<_> = parse(text).into_keys().filter(|k| !english.contains_key(k)).collect();
            assert!(extra.is_empty(), "{code} has keys English doesn't: {extra:?}");
        }
    }

    #[test]
    fn placeholders_fill_in() {
        assert_eq!(tf("lobby.wins", &[("name", "Sam")]), "Sam wins!");
        assert_eq!(t("no.such.key"), "???");
    }
}
