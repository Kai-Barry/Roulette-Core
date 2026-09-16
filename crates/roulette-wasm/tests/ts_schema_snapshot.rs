//! TEST-007 schema drift gate (TASK-007, RISK-004 fallback): extracts the
//! `Command` and `EngineEvent` variant names straight from the Rust source
//! and asserts each — and only each — appears in the handwritten TS contract
//! `web/src/engine/schema.ts`. Fails the build when serde-visible types
//! change without regenerating the TS side.

use std::fmt::Write as _;

const COMMAND_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../roulette-core/src/api/mod.rs");
const EVENT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../roulette-core/src/api/events.rs");
const SCHEMA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/src/engine/schema.ts");

/// Extracts variant names of the enum whose declaration starts at
/// `pub enum <Name> {`, by scanning the balanced body for variant
/// declarations: an identifier in variant position (start of line, after
/// whitespace) followed by `{`, `(`, `,` or end-of-line. Doc comments and
/// attributes are skipped naturally since they don't start with an ident.
fn enum_variants(source: &str, enum_name: &str) -> Vec<String> {
    let decl = format!("pub enum {enum_name} {{");
    let start = source.find(&decl).unwrap_or_else(|| panic!("enum {enum_name} not found"));
    let body_start = start + decl.len();
    let mut depth = 1usize;
    let mut body_end = body_start;
    for (i, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = body_start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &source[body_start..body_end];
    let mut variants = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#[") {
            continue;
        }
        let ident: String = trimmed.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if ident.is_empty() {
            continue;
        }
        let rest = trimmed[ident.len()..].trim_start();
        if rest.starts_with('{') || rest.starts_with('(') || rest.starts_with(',') || rest.is_empty()
        {
            variants.push(ident);
        }
    }
    assert!(!variants.is_empty(), "no variants parsed from {enum_name}");
    variants
}

/// PascalCase → snake_case, matching serde's rename_all = "snake_case".
fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            for l in ch.to_lowercase() {
                out.push(l);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Pulls the string literals out of `export const <NAME> = [ ... ] as const;`.
fn ts_tag_list(schema_ts: &str, const_name: &str) -> Vec<String> {
    let decl = format!("export const {const_name} = [");
    let start = schema_ts
        .find(&decl)
        .unwrap_or_else(|| panic!("missing {const_name} in schema.ts"));
    let body_start = start + decl.len();
    let end_rel = schema_ts[body_start..]
        .find("] as const;")
        .unwrap_or_else(|| panic!("unterminated {const_name} in schema.ts"));
    let body = &schema_ts[body_start..body_start + end_rel];
    body.split(',')
        .map(|s| s.trim().trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn check(name: &str, variants: &[String], ts_tags: &[String]) {
    let expected: Vec<String> = variants.iter().map(|v| to_snake_case(v)).collect();
    let mut msg = String::new();
    for tag in &expected {
        if !ts_tags.contains(tag) {
            let _ = writeln!(msg, "  missing in schema.ts {name}: {tag}");
        }
    }
    for tag in ts_tags {
        if !expected.contains(tag) {
            let _ = writeln!(msg, "  unknown in schema.ts {name}: {tag} (not a Rust variant)");
        }
    }
    if !msg.is_empty() {
        panic!("schema drift detected between Rust `{name}` and web/src/engine/schema.ts:\n{msg}Update COMMAND_TAGS/EVENT_TAGS in web/src/engine/schema.ts.");
    }
}

#[test]
fn ts_schema_matches_rust_variants() {
    let command_src = std::fs::read_to_string(COMMAND_PATH).expect("read Command source");
    let event_src = std::fs::read_to_string(EVENT_PATH).expect("read EngineEvent source");
    let schema_ts = std::fs::read_to_string(SCHEMA_PATH).expect("read schema.ts");

    let commands = enum_variants(&command_src, "Command");
    let events = enum_variants(&event_src, "EngineEvent");
    assert!(commands.len() >= 20, "plausible Command count, got {}", commands.len());
    assert!(events.len() >= 35, "plausible EngineEvent count, got {}", events.len());

    check("Command", &commands, &ts_tag_list(&schema_ts, "COMMAND_TAGS"));
    check("EngineEvent", &events, &ts_tag_list(&schema_ts, "EVENT_TAGS"));
}