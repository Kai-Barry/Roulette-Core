//! Points-mode text rewriting (TASK-034, §6.5).
//!
//! `format_description(text, mode)` rewrites combat language into casino
//! language at query/display time so card texts stay mode-agnostic. The
//! ordered passes below follow §6.5 verbatim; multipliers like "3x" survive
//! because only the trailing "damage" word is rewritten. Case shape is
//! preserved (flat / Title / UPPER).

use crate::battle::state::CombatMode;

/// Rewrites a card description for the given combat mode. Damage mode is the
/// verbatim text; points mode applies the §6.5 ordered rewrite list.
pub fn format_description(text: &str, mode: CombatMode) -> String {
    match mode {
        CombatMode::Damage => text.to_string(),
        CombatMode::Points => rewrite_points(text),
    }
}

fn rewrite_points(text: &str) -> String {
    let mut t = text.to_string();
    // 1. "incoming enemy damage" → "incoming opponent points".
    t = replace_phrase_ci(
        &t,
        &["incoming", "enemy", "damage"],
        &["incoming", "opponent", "points"],
    );
    // 2. "enemy damage" / "opponent damage" → "opponent points".
    t = replace_phrase_ci(&t, &["enemy", "damage"], &["opponent", "points"]);
    t = replace_phrase_ci(&t, &["opponent", "damage"], &["opponent", "points"]);
    // 3. "flat damage" → "flat PTS".
    t = replace_phrase_ci(&t, &["flat", "damage"], &["flat", "PTS"]);
    // 4. "deal/deals/dealt damage" → "score/scores/scored PTS" — the verb and
    //    trailing word are rewritten independently, so multipliers like
    //    "deals 3x damage" keep "3x" intact.
    t = replace_phrase_ci(&t, &["deal", "damage"], &["score", "PTS"]);
    t = replace_phrase_ci(&t, &["deals", "damage"], &["scores", "PTS"]);
    t = replace_phrase_ci(&t, &["dealt", "damage"], &["scored", "PTS"]);
    // 5. "damage dealt" → "points scored"; "damage taken" → "points taken".
    t = replace_phrase_ci(&t, &["damage", "dealt"], &["points", "scored"]);
    t = replace_phrase_ci(&t, &["damage", "taken"], &["points", "taken"]);
    // 6. Any remaining damage/dmg → PTS.
    t = replace_word_ci(&t, "damage", "PTS");
    t = replace_word_ci(&t, "dmg", "PTS");
    // 7. Any remaining deal/deals/dealt → score/scores/scored.
    t = replace_word_ci(&t, "dealt", "scored");
    t = replace_word_ci(&t, "deals", "scores");
    t = replace_word_ci(&t, "deal", "score");
    t
}

/// Recase: lowercase → lowercase, Title → Title, ALL-CAPS → ALL-CAPS.
/// The acronym `PTS` is invariant — it always renders uppercase (§6.5).
fn recase(word: &str, template: &str) -> String {
    if word == "PTS" {
        return word.to_string();
    }
    let alpha: Vec<char> = template.chars().filter(|c| c.is_alphabetic()).collect();
    if !alpha.is_empty() && alpha.iter().all(|c| c.is_uppercase()) {
        word.to_uppercase()
    } else if alpha.first().is_some_and(|c| c.is_uppercase()) {
        let mut cs = word.chars();
        match cs.next() {
            Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
            None => String::new(),
        }
    } else {
        word.to_lowercase()
    }
}

/// Case-insensitive whole-word replace; the replacement is recased to the
/// matched word's shape.
fn replace_word_ci(text: &str, word: &str, replacement: &str) -> String {
    let lower = text.to_lowercase();
    let needle = word.to_lowercase();
    if !lower.contains(&needle) {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if lower[i..].starts_with(&needle) && word_boundary(bytes, i, needle.len()) {
            out.push_str(&recase(replacement, &text[i..i + needle.len()]));
            i += needle.len();
        } else {
            let ch_len = utf8_len(bytes[i]);
            out.push_str(&text[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

/// Case-insensitive phrase replace; each replacement word is recased against
/// the matched word at the same position.
fn replace_phrase_ci(text: &str, phrase: &[&str], replacements: &[&str]) -> String {
    let lower = text.to_lowercase();
    let needle = phrase.join(" ").to_lowercase();
    if !lower.contains(&needle) {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if lower[i..].starts_with(&needle) {
            let mut cursor = i;
            for (j, part) in phrase.iter().enumerate() {
                if j > 0 {
                    while cursor < bytes.len() && (bytes[cursor] as char).is_whitespace() {
                        out.push_str(&text[cursor..cursor + utf8_len(bytes[cursor])]);
                        cursor += utf8_len(bytes[cursor]);
                    }
                }
                out.push_str(&recase(replacements[j], &text[cursor..cursor + part.len()]));
                cursor += part.len();
            }
            i = cursor;
        } else {
            let ch_len = utf8_len(bytes[i]);
            out.push_str(&text[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

fn word_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before_ok = start == 0 || !(bytes[start - 1] as char).is_alphanumeric();
    let after = start + len;
    let after_ok = after >= bytes.len() || !(bytes[after] as char).is_alphanumeric();
    before_ok && after_ok
}

fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}
