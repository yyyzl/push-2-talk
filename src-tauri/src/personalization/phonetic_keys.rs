use pinyin::ToPinyin;
use rphonetic::DoubleMetaphone;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhoneticKeyBundle {
    pub normalized_text: String,
    pub en_phonetic_keys: Vec<String>,
    pub zh_pinyin_key: Option<String>,
    pub zh_pinyin_fuzzy_key: Option<String>,
    pub mixed_keys: Vec<String>,
    pub alias_keys: Vec<String>,
}

pub fn build_key_bundle(text: &str) -> PhoneticKeyBundle {
    let normalized_text = normalize_surface(text);
    let ascii_words = extract_ascii_words(text);
    let zh_pinyin_key = to_pinyin_key(text);
    let zh_pinyin_fuzzy_key = zh_pinyin_key
        .as_ref()
        .map(|key| to_fuzzy_pinyin(key))
        .filter(|key| !key.is_empty());
    let en_phonetic_keys = build_en_phonetic_keys(&ascii_words);
    let mixed_keys = build_mixed_keys(&zh_pinyin_fuzzy_key, &ascii_words, &en_phonetic_keys);
    let alias_keys = build_alias_keys(&zh_pinyin_fuzzy_key, &ascii_words);

    PhoneticKeyBundle {
        normalized_text,
        en_phonetic_keys,
        zh_pinyin_key,
        zh_pinyin_fuzzy_key,
        mixed_keys,
        alias_keys,
    }
}

pub fn normalize_surface(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

fn extract_ascii_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            current.push(ch);
        } else if !current.is_empty() {
            push_alpha_word(&mut words, &current);
            current.clear();
        }
    }

    if !current.is_empty() {
        push_alpha_word(&mut words, &current);
    }

    words
}

fn push_alpha_word(words: &mut Vec<String>, raw: &str) {
    let alpha: String = raw.chars().filter(|ch| ch.is_ascii_alphabetic()).collect();
    if alpha.len() >= 2 {
        words.push(alpha.to_lowercase());
    }
}

fn to_pinyin_key(text: &str) -> Option<String> {
    let mut result = String::new();
    for ch in text.chars() {
        if let Some(pinyin) = ch.to_pinyin() {
            result.push_str(pinyin.plain());
        }
    }

    (!result.is_empty()).then_some(result)
}

pub fn to_fuzzy_pinyin(input: &str) -> String {
    let mut s = input.to_lowercase();
    for (from, to) in [
        ("zh", "z"),
        ("ch", "c"),
        ("sh", "s"),
        ("ang", "an"),
        ("eng", "en"),
        ("ing", "in"),
    ] {
        s = s.replace(from, to);
    }
    s
}

fn build_en_phonetic_keys(words: &[String]) -> Vec<String> {
    if words.is_empty() {
        return Vec::new();
    }

    let mut keys = Vec::new();
    append_phonetic_keys(&mut keys, words);

    let singularized_words = singularize_ascii_words(words);
    if singularized_words != words {
        append_phonetic_keys(&mut keys, &singularized_words);
    }

    keys
}

fn append_phonetic_keys(keys: &mut Vec<String>, words: &[String]) {
    let encoder = DoubleMetaphone::default();
    let primary = compute_phonetic_key(&encoder, words, false);
    let alternate = compute_phonetic_key(&encoder, words, true);

    if !primary.is_empty() {
        push_unique_key(keys, primary.clone());
    }
    if !alternate.is_empty() && alternate != primary {
        push_unique_key(keys, alternate);
    }
}

fn singularize_ascii_words(words: &[String]) -> Vec<String> {
    words
        .iter()
        .map(|word| singularize_ascii_word(word))
        .collect()
}

fn singularize_ascii_word(word: &str) -> String {
    if word.len() <= 3 || word.ends_with("ss") {
        return word.to_string();
    }

    if let Some(stem) = word.strip_suffix("ies") {
        if stem.len() >= 2 {
            return format!("{stem}y");
        }
    }

    if let Some(stem) = word.strip_suffix('s') {
        return stem.to_string();
    }

    word.to_string()
}

fn push_unique_key(keys: &mut Vec<String>, key: String) {
    if !key.is_empty() && !keys.iter().any(|existing| existing == &key) {
        keys.push(key);
    }
}

fn compute_phonetic_key(
    encoder: &DoubleMetaphone,
    words: &[String],
    use_alternate: bool,
) -> String {
    let mut codes = Vec::with_capacity(words.len());

    for word in words {
        let dm = encoder.double_metaphone(word);
        let mut code = if use_alternate {
            dm.alternate()
        } else {
            dm.primary()
        };

        if use_alternate && code.is_empty() {
            code = dm.primary();
        }
        if code.is_empty() {
            return String::new();
        }

        codes.push(code);
    }

    codes.join("|")
}

fn build_mixed_keys(
    zh_pinyin_fuzzy_key: &Option<String>,
    ascii_words: &[String],
    en_phonetic_keys: &[String],
) -> Vec<String> {
    let Some(zh_key) = zh_pinyin_fuzzy_key else {
        return Vec::new();
    };

    let mut keys = Vec::new();
    if !ascii_words.is_empty() {
        keys.push(format!("{}|{}", zh_key, ascii_words.join("|")));
    }
    for key in en_phonetic_keys {
        keys.push(format!("{}|{}", zh_key, key));
    }
    keys.sort();
    keys.dedup();
    keys
}

fn build_alias_keys(zh_pinyin_fuzzy_key: &Option<String>, ascii_words: &[String]) -> Vec<String> {
    let mut keys = Vec::new();

    if let Some(zh_key) = zh_pinyin_fuzzy_key {
        if ascii_words.is_empty() {
            keys.push(zh_key.clone());
        } else {
            keys.push(format!("{}|{}", zh_key, ascii_words.join("|")));
            if let Some(last_word) = ascii_words.last() {
                keys.push(format!("{}|{}", zh_key, last_word));
            }
        }
    }

    keys.extend(build_seeded_product_alias_keys(ascii_words));
    keys.sort();
    keys.dedup();
    keys
}

fn build_seeded_product_alias_keys(ascii_words: &[String]) -> Vec<String> {
    let mut keys = Vec::new();

    for (index, word) in ascii_words.iter().enumerate() {
        for alias in product_pinyin_aliases(word) {
            let tail = &ascii_words[index + 1..];
            if tail.is_empty() {
                keys.push(alias.to_string());
                continue;
            }

            keys.push(format!("{}|{}", alias, tail.join("|")));
            for phonetic_tail in build_en_phonetic_keys(tail) {
                keys.push(format!("{}|{}", alias, phonetic_tail));
            }
        }
    }

    keys
}

fn product_pinyin_aliases(word: &str) -> &'static [&'static str] {
    match word {
        "claude" => &["kelaode"],
        "openai" => &["oupenai", "oupen|ai"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_near_misses_share_double_metaphone_key() {
        let cloud_code = build_key_bundle("cloud code");
        let claud_code = build_key_bundle("claud code");
        let cloud_coat = build_key_bundle("cloud coat");

        assert!(!cloud_code.en_phonetic_keys.is_empty());
        assert_eq!(cloud_code.en_phonetic_keys, claud_code.en_phonetic_keys);
        assert_eq!(cloud_code.en_phonetic_keys, cloud_coat.en_phonetic_keys);
    }

    #[test]
    fn english_plural_near_misses_include_singular_phonetic_key() {
        let type_script = build_key_bundle("type script");
        let types_script = build_key_bundle("types script");
        let types_scripts = build_key_bundle("types scripts");

        assert_shared_key(
            &type_script.en_phonetic_keys,
            &types_script.en_phonetic_keys,
        );
        assert_shared_key(
            &type_script.en_phonetic_keys,
            &types_scripts.en_phonetic_keys,
        );
    }

    #[test]
    fn mixed_chinese_alias_key_keeps_ascii_tail() {
        let keys = build_key_bundle("克劳德 code");

        assert!(keys.alias_keys.iter().any(|key| key == "kelaode|code"));
        assert!(keys.mixed_keys.iter().any(|key| key == "kelaode|code"));
    }

    #[test]
    fn corrected_ascii_product_generates_cross_language_aliases() {
        let keys = build_key_bundle("Claude Code");

        assert!(keys.alias_keys.iter().any(|key| key == "kelaode|code"));
        assert!(keys.alias_keys.iter().any(|key| key == "kelaode|KT"));
    }

    #[test]
    fn corrected_openai_generates_cross_language_aliases() {
        let keys = build_key_bundle("OpenAI");

        assert!(keys.alias_keys.iter().any(|key| key == "oupenai"));
        assert!(keys.alias_keys.iter().any(|key| key == "oupen|ai"));
    }

    fn assert_shared_key(left: &[String], right: &[String]) {
        assert!(
            left.iter().any(|left_key| right.contains(left_key)),
            "expected shared key between {:?} and {:?}",
            left,
            right
        );
    }
}
