//! RON loader (TASK-009): `Content::load_dir` for the data directory and
//! `Content::embedded` for the built-in defaults (REQ-001).

use crate::schema::Content;
use crate::validate::{validate, ContentError};
use std::path::Path;

/// The RON files that make up a content bundle.
const FILES: [(&str, &str); 7] = [
    ("cards", "cards.ron"),
    ("wheels", "wheels.ron"),
    ("board_upgrades", "board_upgrades.ron"),
    ("curses", "curses.ron"),
    ("enemies", "enemies.ron"),
    ("events", "events.ron"),
    ("forge_ops", "forge_ops.ron"),
];

/// Embedded default content, compiled into the binary so the engine and the
/// simulator work without an on-disk data directory.
pub mod embedded {
    /// Embedded `cards.ron`.
    pub const CARDS: &str = include_str!("../../../content/cards.ron");
    /// Embedded `wheels.ron`.
    pub const WHEELS: &str = include_str!("../../../content/wheels.ron");
    /// Embedded `board_upgrades.ron`.
    pub const BOARD_UPGRADES: &str = include_str!("../../../content/board_upgrades.ron");
    /// Embedded `curses.ron`.
    pub const CURSES: &str = include_str!("../../../content/curses.ron");
    /// Embedded `enemies.ron`.
    pub const ENEMIES: &str = include_str!("../../../content/enemies.ron");
    /// Embedded `events.ron`.
    pub const EVENTS: &str = include_str!("../../../content/events.ron");
    /// Embedded `forge_ops.ron`.
    pub const FORGE_OPS: &str = include_str!("../../../content/forge_ops.ron");
}

/// ron parser options: IMPLICIT_SOME lets content authors write bare values
/// for `Option` fields (`friction: 0.65` instead of `Some(0.65)`).
fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

fn parse_part(file: &str, text: &str) -> Result<Content, ContentError> {
    ron_options()
        .from_str::<Content>(text)
        .map_err(|source| ContentError::Parse { file: file.to_string(), source })
}

fn merge_parts(parts: [Result<Content, ContentError>; 7]) -> Result<Content, ContentError> {
    let mut merged =
        Content { schema_version: crate::schema::SCHEMA_VERSION, ..Content::default() };
    for part in parts {
        let part = part?;
        merged.cards.extend(part.cards);
        merged.wheels.extend(part.wheels);
        merged.board_upgrades.extend(part.board_upgrades);
        merged.curses.extend(part.curses);
        merged.enemies.extend(part.enemies);
        merged.events.extend(part.events);
        merged.forge_ops.extend(part.forge_ops);
    }
    Ok(merged)
}

impl Content {
    /// Loads and validates a content directory holding the seven RON files.
    pub fn load_dir(path: impl AsRef<Path>) -> Result<Content, ContentError> {
        let base = path.as_ref();
        let mut parts: Vec<Result<Content, ContentError>> = Vec::with_capacity(FILES.len());
        for (_, name) in FILES {
            let file_path = base.join(name);
            let text = std::fs::read_to_string(&file_path).map_err(|source| ContentError::Io {
                file: file_path.display().to_string(),
                source,
            })?;
            parts.push(parse_part(name, &text));
        }
        let [a, b, c, d, e, f, g] = parts.try_into().expect("exactly 7 content files");
        let merged = merge_parts([a, b, c, d, e, f, g])?;
        validate(&merged)?;
        Ok(merged)
    }

    /// Loads the embedded default content (see [`embedded`]).
    pub fn embedded() -> Result<Content, ContentError> {
        let texts = [
            embedded::CARDS,
            embedded::WHEELS,
            embedded::BOARD_UPGRADES,
            embedded::CURSES,
            embedded::ENEMIES,
            embedded::EVENTS,
            embedded::FORGE_OPS,
        ];
        let mut parts: Vec<Result<Content, ContentError>> = Vec::with_capacity(7);
        for (i, text) in texts.iter().enumerate() {
            let (_, name) = FILES[i];
            parts.push(parse_part(name, text));
        }
        let [a, b, c, d, e, f, g] = parts.try_into().expect("exactly 7 content files");
        let merged = merge_parts([a, b, c, d, e, f, g])?;
        validate(&merged)?;
        Ok(merged)
    }

    /// Parses and validates a single ad-hoc RON document (tests, tools).
    pub fn parse(text: &str) -> Result<Content, ContentError> {
        let content = parse_part("ad-hoc", text)?;
        validate(&content)?;
        Ok(content)
    }
}
