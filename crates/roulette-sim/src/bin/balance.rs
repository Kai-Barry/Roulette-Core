//! `balance` — per-card EV audit + suggested-cost rewriter (§15/TASK-046/047):
//! evaluates every card against `defaultBets=[red/10]` on the classic wheel,
//! rates against the §14.3 rarity bands, and (with `--apply`) rewrites
//! `cost:` fields in `content/cards.ron` for OP/UP cards.

use std::sync::Arc;

use roulette_content::Content;
use roulette_sim::ev::{apply_costs, evaluate_card, EvConfig};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("usage: balance [--spins N] [--physics] [--seed-prefix s] [--apply]");
        return;
    }
    let spins = args
        .iter()
        .position(|a| a == "--spins")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let physics = args.iter().any(|a| a == "--physics");
    let seed_prefix = args
        .iter()
        .position(|a| a == "--seed-prefix")
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| "ev".into());
    let apply = args.iter().any(|a| a == "--apply");
    let cfg = EvConfig { spins, physics, seed_prefix };
    let _ = apply;

    // Audit against the on-disk wave content (falls back to embedded).
    let path = std::path::Path::new("content");
    let (content, content_dir) = match Content::load_dir(path) {
        Ok(c) => (Arc::new(c), Some(path.to_path_buf())),
        Err(_) => (Arc::new(Content::embedded().expect("embedded content validates")), None),
    };
    let wheel_def = content
        .wheels
        .iter()
        .find(|w| w.id == roulette_sim::ev::SCENARIO_WHEEL_ID)
        .expect("classic wheel in content");
    let wheel = roulette_core::wheel::WheelConfig::from_def(wheel_def);

    let mut reports: Vec<_> =
        content.cards.iter().map(|def| evaluate_card(&content, def, &cfg, &wheel)).collect();
    reports.sort_by(|a, b| {
        b.efficiency.partial_cmp(&a.efficiency).unwrap_or(std::cmp::Ordering::Equal)
    });

    println!(
        "card EV audit — {} cards, {} spins/card, mode={} (§14.3 bands)",
        reports.len(),
        spins,
        if physics { "physics" } else { "uniform" }
    );
    println!(
        "{:<22} {:<10} {:<4} {:>8} {:>8} {:>8} {:>9} {:<10} {:>3}",
        "id", "type", "⚡", "base", "mod", "ΔEV", "ΔEV/⚡", "rating", "→cost"
    );
    for r in &reports {
        println!(
            "{:<22} {:<10} {:<4} {:>8.2} {:>8.2} {:>+8.2} {:>9.2} {:<10} {:>3}",
            r.id,
            format!("{:?}", r.card_type),
            r.cost,
            r.baseline,
            r.modified,
            r.delta_ev,
            r.efficiency,
            r.rating,
            if r.suggested_cost != r.cost { r.suggested_cost.to_string() } else { "-".into() },
        );
    }
    let mut counts = std::collections::BTreeMap::new();
    for r in &reports {
        *counts.entry(r.rating.clone()).or_insert(0u32) += 1;
    }
    println!("ratings: {:?}", counts);

    let updates: Vec<(String, u8)> = reports
        .iter()
        .filter(|r| r.suggested_cost != r.cost)
        .map(|r| (r.id.clone(), r.suggested_cost))
        .collect();
    if updates.is_empty() {
        println!("no cost rewrites suggested");
        return;
    }
    println!(
        "suggested rewrites: {} — {}",
        updates.len(),
        updates.iter().map(|(id, c)| format!("{id}→{c}")).collect::<Vec<_>>().join(", ")
    );

    if !args.iter().any(|a| a == "--apply") {
        println!("dry run (pass --apply to rewrite content/cards.ron)");
        return;
    }
    let Some(dir) = content_dir else {
        eprintln!("--apply needs content/ on disk (embedded fallback is read-only)");
        std::process::exit(1);
    };
    let cards_path = dir.join("cards.ron");
    let text = std::fs::read_to_string(&cards_path).expect("content/cards.ron readable");
    let (new_text, rewritten) = apply_costs(&text, &updates);
    std::fs::write(&cards_path, &new_text).expect("write cards.ron");
    // Validate: the rewritten file must parse and only cost fields differ.
    let reloaded = Content::load_dir(dir).expect("rewritten cards.ron parses");
    for (id, cost) in &updates {
        let def = reloaded.cards.iter().find(|c| &c.id == id).expect("card still present");
        assert_eq!(&def.cost, cost, "cost rewrite verified for {id}");
    }
    println!("applied {rewritten} cost rewrites to {}", cards_path.display());
}
