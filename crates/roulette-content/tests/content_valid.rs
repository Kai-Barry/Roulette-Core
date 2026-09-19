//! TASK-012: RON round-trip idempotence, validator rejections, embedded load.

use roulette_content::*;

#[test]
fn embedded_content_loads_and_validates() {
    let content = Content::embedded().expect("embedded content must load");
    assert!(content.cards.len() >= 55, "card count: {}", content.cards.len());
    assert!(content.wheels.len() >= 6);
    assert_eq!(content.board_upgrades.len(), 18, "all 18 board upgrades");
    assert_eq!(content.curses.len(), 10, "all 10 curses");
    assert_eq!(content.enemies.len(), 6, "all 6 enemies");
    assert!(!content.events.is_empty());
    assert!(!content.forge_ops.is_empty());

    // Classic wheel: 37 unique slots, European order, gold 7 (§4.6).
    let classic = content.wheels.iter().find(|w| w.id == "classic").expect("classic wheel");
    assert_eq!(classic.numbers.len(), 37);
    assert_eq!(classic.numbers[0], 0);
    assert_eq!(classic.slot_colors.get(&7), Some(&SlotColor::Gold));
    assert_eq!(classic.payout_multipliers.number, 36.0);

    // Enemy roster shape (§7.1).
    let house = content.enemies.iter().find(|e| e.id == "the_house").expect("boss");
    assert_eq!(house.max_hp, 220);
    assert_eq!(house.tier, EnemyTier::Boss);
    assert_eq!(house.difficulty, 1.0);
}

#[test]
fn load_dir_matches_embedded() {
    let dir = Content::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content"))
        .expect("content dir loads");
    let embedded = Content::embedded().expect("embedded loads");
    assert_eq!(dir, embedded, "dir content must equal embedded content");
}

#[test]
fn ron_round_trip_is_idempotent() {
    let content = Content::embedded().expect("embedded loads");
    let ser1 = ron::to_string(&content).expect("serialize");
    let back1: Content = ron::from_str(&ser1).expect("deserialize");
    let ser2 = ron::to_string(&back1).expect("re-serialize");
    assert_eq!(ser1, ser2, "round trip must be idempotent");
    assert_eq!(back1, content);
}

#[test]
fn parse_rejects_duplicate_card_id() {
    let ron = r#"
(
    schema_version: 1,
    cards: [
        (id: "dup", name: "A", description: "x", cost: 1, card_type: payout, rarity: common, effects: [insurance]),
        (id: "dup", name: "B", description: "x", cost: 1, card_type: payout, rarity: common, effects: [insurance]),
    ],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("duplicate id"), "{err}");
}

#[test]
fn parse_rejects_wheel_below_two_slots() {
    let ron = r#"
(
    schema_version: 1,
    wheels: [(
        id: "tiny", name: "Tiny", description: "x",
        numbers: [0], green_numbers: [0], red_numbers: [], black_numbers: [],
        payout_multipliers: (red: 2.0, black: 2.0, green: 14.0, number: 36.0, odd: 2.0, even: 2.0),
        rarity: common,
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("at least 2 slots"), "{err}");
}

#[test]
fn parse_rejects_duplicate_wheel_numbers() {
    let ron = r#"
(
    schema_version: 1,
    wheels: [(
        id: "doubled", name: "Doubled", description: "x",
        numbers: [0, 7, 7], green_numbers: [0], red_numbers: [7], black_numbers: [],
        payout_multipliers: (red: 2.0, black: 2.0, green: 14.0, number: 36.0, odd: 2.0, even: 2.0),
        rarity: common,
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("duplicate slot number"), "{err}");
}

#[test]
fn parse_rejects_uncolored_slot() {
    let ron = r#"
(
    schema_version: 1,
    wheels: [(
        id: "gap", name: "Gap", description: "x",
        numbers: [0, 1], green_numbers: [0], red_numbers: [], black_numbers: [],
        payout_multipliers: (red: 2.0, black: 2.0, green: 14.0, number: 36.0, odd: 2.0, even: 2.0),
        rarity: common,
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("uncolored"), "{err}");
}

#[test]
fn parse_rejects_non_positive_payout() {
    let ron = r#"
(
    schema_version: 1,
    wheels: [(
        id: "free", name: "Free", description: "x",
        numbers: [0, 1], green_numbers: [0], red_numbers: [1], black_numbers: [],
        payout_multipliers: (red: 0.0, black: 2.0, green: 14.0, number: 36.0, odd: 2.0, even: 2.0),
        rarity: common,
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("must be positive"), "{err}");
}

#[test]
fn parse_rejects_nan_payout() {
    let ron = r#"
(
    schema_version: 1,
    wheels: [(
        id: "nanwheel", name: "Nan", description: "x",
        numbers: [0, 1], green_numbers: [0], red_numbers: [1], black_numbers: [],
        payout_multipliers: (red: 2.0, black: 2.0, green: 14.0, number: 36.0, odd: 2.0, even: NaN),
        rarity: common,
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("non-finite"), "{err}");
}

#[test]
fn parse_rejects_unknown_upgrade_ref() {
    let ron = r#"
(
    schema_version: 1,
    wheels: [(
        id: "ref", name: "Ref", description: "x",
        numbers: [0, 1], green_numbers: [0], red_numbers: [1], black_numbers: [],
        payout_multipliers: (red: 2.0, black: 2.0, green: 14.0, number: 36.0, odd: 2.0, even: 2.0),
        upgrades: ["not_a_real_upgrade"],
        rarity: common,
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("unknown upgrade"), "{err}");
}

#[test]
fn parse_rejects_unknown_enemy_wheel() {
    let ron = r#"
(
    schema_version: 1,
    enemies: [(
        id: "ghost", name: "Ghost", max_hp: 40, tier: normal, wheel_id: "missing_wheel",
        intent_pattern: (
            (action: attack, value: 5, description: "a"),
            (action: attack, value: 5, description: "b"),
            (action: attack, value: 5, description: "c"),
            (action: attack, value: 5, description: "d"),
        ),
        difficulty: 0.25, sprite_name: "x",
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("unknown wheel"), "{err}");
}

#[test]
fn parse_rejects_bad_schema_version() {
    let ron = "(schema_version: 999)";
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("schema version"), "{err}");
}

#[test]
fn parse_rejects_unknown_fields() {
    let ron = r#"
(
    schema_version: 1,
    cards: [(
        id: "x", name: "X", description: "x", cost: 1, card_type: payout, rarity: common,
        effects: [], bogus_field: 1,
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("bogus_field") || err.to_string().contains("parse"), "{err}");
}

#[test]
fn parse_rejects_event_granting_unknown_card() {
    let ron = r#"
(
    schema_version: 1,
    events: [(
        id: "bad_event", title: "Bad",
        choices: [(id: "c1", label: "Take it", effects: [grant_card(card_id: "nope")])],
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("unknown card"), "{err}");
}

#[test]
fn parse_rejects_card_over_cost_cap() {
    let ron = r#"
(
    schema_version: 1,
    cards: [(
        id: "wallet_drain", name: "Wallet Drain", description: "x", cost: 9,
        card_type: payout, rarity: legendary, effects: [insurance],
    )],
)
"#;
    let err = Content::parse(ron).unwrap_err();
    assert!(err.to_string().contains("outside 0..=5"), "{err}");
}
