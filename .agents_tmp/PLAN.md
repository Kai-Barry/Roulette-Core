# 1. OBJECTIVE

Rebuild the `roulette_core` Rust engine from scratch as a **data-driven, modular, headless game core** for *Roulette.OS* (Roulette of the Damned), replacing the current hard-coded prototype engine that is slow to extend and limited in scope.

The new engine must faithfully implement the mechanics specified in `docs/ROULETTE_OS_GAME_BLUEPRINT.md` — real 120 Hz deterministic wheel physics with prediction dry-runs, points-mode roulette battles with the full §10 damage formula stack, run/map progression, shop/forge/events/curses economy, EV-based enemy AI, and a data-driven card/wheel/enemy content system — while making every piece of content and every mechanic **addable without touching core logic** (external data files + effect DSL + hook pipeline). First deliverable is a CLI-playable, fully headless core plus a Monte Carlo balancing simulator; the web (Three.js) frontend is a later phase that consumes the same headless API over WASM.

# 2. CONTEXT SUMMARY

## 2.1 What exists today (the prototype engine being replaced)

| Existing file | Content | Verdict |
|---|---|---|
| `src/rng.rs` | Mulberry32 PRNG, djb2 string seeds, Fisher-Yates shuffle, deterministic and well-tested | **Keep** (migrate as-is) |
| `src/device.rs` | `GameDevice` trait; European/American wheels + dice; **instant uniform-random spin** (no physics); mutable-slot mutation model | Replace with declarative `WheelConfig` + physics sim |
| `src/bet.rs` | 9 bet types, hardcoded base multipliers | Replace with data-driven payout tables incl. dozen/column/special colors |
| `src/cards.rs` | **11 cards hard-coded in a Rust enum** (`CardEffect`), mode filter, `apply_to_device` | Replace with data-driven cards + effect DSL |
| `src/combat.rs` | Single `execute_turn` pipeline; HP/damage oriented; ignores points mode | Replace with full BattleState state machine (§3, §10) |
| `src/mode.rs` | `CombatMode` trait: 1v1 / Horde / PointRound | Absorb into declarative `MatchRules`; drop trait (YAGNI until a second real mode exists) |
| `src/target.rs` | `Target` trait; 1 enemy, 3 intents | Replace with data-driven enemies, 6 intent types, EV-based AI |
| `src/wheel.rs` | Legacy re-exports | Delete |
| `src/main.rs` | Interactive CLI demo | Rewrite against new API |
| `src/bin/simulate.rs` | Monte Carlo benchmark (instant spins) | Rewrite: full-battle sims + per-card EV balancer |
| `Cargo.toml` | Single crate, zero deps | Convert to Cargo workspace |

Problems to fix (why we are starting again): content hard-coded in source (each card = recompile), boolean-soup state (`BoardModifiers`), no run layer (map/shop/forge/events/curses missing), no physics (uniform random spin, so the prediction mechanic is impossible), oversimplified damage math, mixed damage/points semantics, no data validation, no balance tooling.

## 2.2 What the target game requires (from the blueprint)

- **§2 Run structure**: 7/11/15-floor branch map (3 lanes, node types combat/elite/shop/event/forge/boss), loadout store draft (10 PTS), reward rarity table (3/9/28/60 %).
- **§3 Combat**: points-mode battles, 30/30 chip pools, starting hand 2, draw ladder 0/3/5/7/9 (+2 thereafter), hand limit 8, chip denominations 1/5/10, bet merge/remove/clear/rebet, sacrifice (−10 PTS → +5⚡), round limits 3/5/8, sudden death on ties.
- **§4 Wheel**: 14 slot colors defined (7 used: red/black/green/gold/purple/cyan/crimson), bet payouts incl. gold 4×, purple 4×, cyan 4×, crimson 6×, dozen/column 3×; level-scaled special colors with 3 unlock abilities (Fever Ignition, Glacier Core, Synapse Link); 19 wheel templates; 18 board upgrades; ≥2-slot customizer rule.
- **§5 Physics**: 120 Hz fixed-step sim — WHEEL_DECAY 0.45, BALL_DECAY 2.4, 8 deflector pins (window 0.75–0.93), 37 divider pegs, gravity bands (9.0 / 4.5 / 3.0), settle criteria, slot index formula, multiball (cap 6–8), shotgun 1.5 s, cheat hooks (friction, ballMass, spinSpeed, tilt, targetZoneBias, nudge, bounceRandomness, splitPeg), seeded dry-run prediction with `predictionSize` sector.
- **§6 Cards**: 7 types, 4 rarities, 113 cards; effects expressed as physics/board/payout/utility/chaos/paint/money primitives; played cards persist face-up with temp durations (3–5 spins); points-mode description rewriter.
- **§7 Enemies**: 6 enemies (HP 40–220), 4-move looping intent patterns, action types attack/shield/physics_debuff/steal_chips/block_red/buff, EV-based betting AI with `difficulty` (0.25–1.0) and comeback risk tolerance.
- **§8 Curses**: 10 curses that corrupt tools (prediction, magnetism, economy, hand size, healing).
- **§9 Economy**: shop (6 cards + 2 wheels + heal), forge (3 free ops, reroll 5⚡, 6 op types), events (data-driven choices).
- **§10 Damage formula**: win detection on effective colors → base payout → zone multipliers → prediction/rarity cheat penalties → post-sum modifiers (streak ≤ 4.0, global, one-shots) → floor; special-color on-land effects; per-ball resolution.
- **§16 Schemas**: `RunState`, `WheelConfig`, `Card`, `BoardUpgrade`, `ForgeCard`, `StoreItem`, `Enemy`, `MapNode`, `BattleState`, `PhysicsModifiers`, `BoardModifiers` (§16.2–16.3).

## 2.3 Requirements & Constraints

- **REQ-001**: All game content (cards, wheels, board upgrades, enemies, curses, events) is defined in external data files and loaded at runtime; adding a card requires **zero** Rust code changes.
- **REQ-002**: Wheel outcomes come from the deterministic 120 Hz physics simulation per §5 (constants verbatim), supporting both live stepping (for a future renderer) and headless `run_to_completion`.
- **REQ-003**: Prediction is a real dry-run of the same simulation with the same seed, returning a `predictionSize` sector; penalty ladder 0.3/0.5/1.0 applies per §10.1 step 4.
- **REQ-004**: Combat implements points mode with the complete §10 damage pipeline and §3 turn structure; legacy `damage` mode remains available via match rules.
- **REQ-005**: Full run layer: map generation (§2.3), rewards (§2.5), shop (§9.1), forge + wheel customizer (§9.2, §4.8), events (§9.4), curses (§8).
- **REQ-006**: Enemy AI: EV-based bet enumeration with `difficulty` probability of the optimal line and comeback risk tolerance (§7.4).
- **REQ-007**: Cross-platform bit-exact determinism: identical seed ⇒ identical outcomes on native and wasm; physics may not use platform-dependent transcendentals (custom fixed polynomial sin/cos).
- **REQ-008**: Engine is headless (no IO, no rendering); all mutations flow through a public `Engine` API returning `Result<_, EngineError>` and emitting an `EngineEvent` stream a future web frontend can replay/render.
- **REQ-009**: Snapshot/undo: cheap `Clone`-based snapshots before each player action; `undo()` restores.
- **REQ-010**: Content is validated at load (unique ids, known effect kinds, wheels ≥ 2 slots, unique slot numbers per wheel, positive payouts) and fails fast with precise errors.
- **REQ-011**: Monte Carlo tooling: full-battle simulation (win rates, avg rounds, throughput) and per-card EV evaluation (§15) with suggested-cost report and `--apply` rewrite of data files.
- **CON-001**: Rust (edition 2021), stable toolchain; workspace of small crates; keep compile times low.
- **CON-002**: Allowed dependencies (deliberate, minimal): `serde`, `ron`, `thiserror`; `proptest` as dev-dependency only. No async, no IO in core.
- **CON-003**: Engine-first scope: CLI + simulator this time; the Three.js/DOM frontend, audio, and PS1 shader are explicitly out of scope (later phase consuming the same headless API over wasm-bindgen).
- **CON-004**: Content fidelity: systems are built against a representative subset (≈55 cards, 6 wheels) first; the full 113-card/19-wheel table is a later content-port wave once systems are stable.
- **CON-005**: Existing `src/*` prototype code is replaced, not preserved behind compat shims; `wheel.rs` legacy aliases are deleted.
- **GUD-001**: Errors via `thiserror`; no `panic!`/`unwrap` in library paths on user/content input.
- **GUD-002**: Exhaustive `match` everywhere (compiler-enforced extension points); new mechanic ⇒ new enum variant + one handler arm.
- **GUD-003**: `cargo fmt` + `clippy -D warnings` clean; unit tests colocated, integration tests per crate, golden determinism tests for the sim.
- **GUD-004**: Named RNG substreams (e.g. `derive("physics")`, `derive("ai")`, `derive("loot")`) from one master seed so adding a random call in one system never shifts another system's stream.
- **GUD-005**: Data-first: any state gameplay mutates (physics modifiers, board modifiers, streaks) lives in plain serde-friendly structs — inspectable, cloneable, snapshot-able.
- **PAT-001**: **Effect DSL** — cards/enemies/curses/upgrades/events declare `Vec<EffectKind>` data; a dispatch module executes kinds at defined hook points.
- **PAT-002**: **Modifier stack** — timed/temporary effects are entries in `Vec<Modifier>` with `remaining_spins`, replacing boolean soup; expiry ticks at round end.
- **PAT-003**: **Pipeline resolution** — spin resolution is an ordered pure-function pipeline (win detect → base payout → zone mults → cheat penalties → post-sum → floor) matching §10.1 exactly.
- **PAT-004**: **Event-sourced facade** — public API methods return event logs; state is queryable via serde-serializable snapshots for tooling and future UI.

# 3. APPROACH OVERVIEW

Reorganize the repository into a **Cargo workspace** of four crates around one principle: *content is data, mechanics are code, and the seam between them is a typed effect DSL*.

| Crate | Kind | Responsibility |
|---|---|---|
| `crates/roulette-content` | lib | Content schema (serde/RON), loader + validator, embedded default content; definitions for cards, wheels, upgrades, enemies, curses, events |
| `crates/roulette-core` | lib | Headless engine: rng, physics, wheel/bets, board modifiers, battle state machine + §10 damage pipeline, enemy AI, run/map/economy, public `Engine` facade |
| `crates/roulette-cli` | bin | Terminal game driving the public API (play the whole game headless) |
| `crates/roulette-sim` | bin | `simulate battles` + `simulate cards` Monte Carlo / EV balancing tool (§15) |

Rationale: the prototype's limitations were structural (content in code, monolithic turn function, missing layers). A workspace with a strict content/engine seam (a) makes new content a data-file edit, (b) makes new mechanics a new enum variant handled at explicit hook points, (c) keeps the core free of IO so the web frontend is a thin binding later, and (d) lets the balancing tool rewrite content in place — closing the prototype's balance loop. Alternatives are listed below; the main rejections: staying in TypeScript (loses the deterministic high-speed simulation layer that motivates this repo) and keeping everything in one crate (cheap now, re-splits later).

## 3.1 Alternatives considered

- **ALT-001**: Rebuild engine in TypeScript (as the shipped prototype). Rejected: the prototype was the slow/limited baseline; a Rust core gives deterministic high-speed simulation for balance tooling and clean WASM embedding; the blueprint's Monte Carlo studio depends on it.
- **ALT-002**: Keep cards as compiled Rust data tables (like today). Rejected: violates REQ-001; the balancer cannot rewrite content without recompiles; data files + a validator give faster iteration.
- **ALT-003**: Port all 113 cards / 19 wheels before stabilizing systems. Rejected: high data-entry cost before the effect vocabulary is frozen; subset-first (CON-004) lets real cards drive the DSL design, then the remaining table ports mechanically.
- **ALT-004**: Single crate with feature flags instead of a workspace. Rejected: crate boundaries are cheap to create now and expensive later; cohesive modules stay inside `roulette-core`.

# 4. IMPLEMENTATION STEPS

Phases are ordered by dependency. Phase 0 establishes the workspace; each later phase lands with green tests. Tasks inside a phase are parallelizable unless a `TASK-xxx` dependency is noted.

## Phase 0 — Workspace scaffolding & legacy migration

- **GOAL-001**: Convert the repo to the 4-crate workspace with the deterministic RNG layer and CI-grade hygiene, keeping `cargo test` green at every commit.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-001 | Create workspace `Cargo.toml` (`resolver = "2"`, members = the four crates); add `rust-toolchain.toml` pinning stable; add `serde`, `ron`, `thiserror` (+ dev `proptest`); move current `src/{lib,rng}.rs` into `crates/roulette-core/src/` (rng module unchanged); delete `src/wheel.rs` legacy aliases | | |
| TASK-002 | Create `crates/roulette-content` (lib), `crates/roulette-cli` (bin `roulette`), `crates/roulette-sim` (bins `simulate`); temporarily port old `main.rs`/`simulate.rs` behavior into the new bins against the moved core so the workspace compiles | | |
| TASK-003 | Add `Rng::derive(label) -> Rng` child-stream constructor (djb2(seed_str + label) reseed) plus `Rng::range_usize` and `Rng::pick`; unit tests for stream independence (GUD-004) | | |
| TASK-004 | Add `deterministic_floats` module in core: polynomial `sin_approx`/`cos_approx` (documented max error vs std ≤ 1e-9 on [−2π, 2π]) + tests pinning exact bit patterns (REQ-007) | | |
| TASK-005 | Add `rustfmt.toml`, clippy policy (`-D warnings`), and a task runner entry: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` | | |

## Phase 1 — Content crate: schema, loader, validator, wave-1 data

- **GOAL-002**: Type-safe content definitions loaded from RON files with fail-fast validation, and wave-1 content (≈55 cards, 6 wheels, all upgrades/curses/enemies/events) authored.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-006 | Define content schema mirroring §16.2: `CardDef { id, name, description, cost: u8 (0..=5), card_type: {Physics, Board, Payout, Utility, Chaos, Paint, Money}, rarity: {Common, Uncommon, Rare, Legendary}, effects: Vec<EffectKind> }`; `WheelDef { id, name, description, numbers: Vec<u32> (physical order), green_numbers: Vec<u32>, slot_colors: BTreeMap<u32, SlotColor>, payout_multipliers: PayoutTable, upgrades: Vec<String>, rarity, points_cost }`; `BoardUpgradeDef`; `EnemyDef { id, name, max_hp, tier, wheel_id, intent_pattern: [EnemyAction; 4], difficulty, sprite }`; `CurseDef`; `EventDef { id, title, choices: Vec<EventChoice { label, effects: Vec<EffectKind> }> }`; `ForgeOpDef` (6 op types + params, §9.2); all with `serde::{Serialize, Deserialize}` and `#[serde(deny_unknown_fields)]` | | |
| TASK-007 | Define shared value enums: `SlotColor { Red, Black, Green, Gold, Purple, Cyan, Crimson }`, `EnemyAction { Attack, Shield, PhysicsDebuff, StealChips, BlockRed, Buff }`, `NumberSet` helper (range / list / predicate: odd, even, prime, single-digit, high, low) for slot-targeting effects | | |
| TASK-008 | Define the `EffectKind` DSL enum covering every effect in §6.2/§6.4: `Physics { friction, ball_mass, spin_speed, wheel_tilt, target_zone_bias, bias_red_only, bias_black_only, nudge_distance, bounce_randomness, multiball, split_peg, shotgun, prediction_size }` (all `Option<_>`), `PayoutMult { target: PayoutTarget, value, duration: DurationKind (Fight | Spins(u8) | NextWin) }`, `CustomNumberMult { numbers, mult }`, `ConvertSlots { to: SlotColor, from: Option<SlotColor>, numbers: NumberSet, scope: Round | Spin | Fight }`, `AddGreenSlot { number }`, `ZoneMark { kind: {Lucky, Cursed, ChipMine, LifeFountain, DangerZone, GoldFoil, CopperPlate, Mirror}, slots }`, `GlobalMult { mult, spins }`, `Draw { count }`, `DiscardHand`, `GrantChips { amount }`, `StunEnemy { turns, condition: OnPlay | OnDamageGE(u16) }`, one-shot armers: `DoubleNextPayout, Insurance, LuckyCharm, EmeraldForest, GoldenHeist, RiskCapital, CapitalVenture { vent_count }`, `HandOp { kind: {CopyRandomHandCard, CopyLastPlayed, RecycleFromDiscard, Retain, ExileSelf} }`, `TempEssenceChips { count, destination: {Hand, DrawPile, DiscardPile} }`, `SetCustomPayout { bet_color, mult }` | | |
| TASK-009 | Implement `Content` root with `schema_version: u32`; `Content::load_dir(path)` and embedded fallback `Content::embedded()` (`include_str!`); validator implementing REQ-010 with `ContentError` (thiserror) reporting file + id + reason | | |
| TASK-010 | Author `content/cards.ron` wave-1 (≈55 cards spanning every §6.4 hook): payout (crimson_double, dark_fury, green_greed, lucky_seven, golden_zero, green_ripple, high_roller, low_sweep, double_payout, first/second/third_dozen, single_out, prime_target, column_wave), physics (friction_oil, lead_ball, table_tilt, magnetic_force, predictive_sight, nudge_cheat, sand_trap, eagle_eye, slow_spin, turbo_spin, omniscience, micro_nudge), chaos (triple_threat, peg_splitter, shotgun_blast), board (crimson_flood, abyssal_darkness, emerald_rift, crimson_deluge, jade_path, zero_eclipse, mirror_slot, gold_foil, copper_plate, danger_zone, red_sea), paint (paint_red, paint_green, paint_gold, paint_inverse), utility (double_down, quick_draw, heavy_draw, deck_shuffle, calculated_risk, golden_mirror, copy_paste, insurance_policy, stun_strike, adrenaline_rush, lucky_charm), money (essence_chip, chip_maker, tax_refund, capital_venture, golden_heist) — all with verbatim §6.3 numbers | | |
| TASK-011 | Author `content/wheels.ron` wave-1 (classic, dozen, double_zero, phantom, chaos_engine, deaths_roulette — exact §4.6 tables incl. the European order array), `content/board_upgrades.ron` (all 18, §4.7), `content/curses.ron` (all 10, §8), `content/enemies.ron` (all 6, §7.1 with 4-move patterns + difficulty), `content/events.ron` (Hooded Spectre, §9.4) | | |
| TASK-012 | Tests: RON round-trip idempotence; validator rejects (duplicate id, unknown slot color, wheel < 2 slots, duplicate wheel numbers, non-positive payout, unknown upgrade/wheel ref); all embedded content loads | | |

## Phase 2 — Deterministic physics simulation (the game's soul)

- **GOAL-003**: `roulette_core::phys` implements §5 verbatim: 120 Hz fixed-step sim, pin/peg chaos, settle detection, cheat hooks, multiball, seeded prediction dry-run, `SimEvent` stream.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-013 | Implement `WheelLayout` (from `WheelDef`): slot angular positions, slot width, color map; `PhysicsModifiers` struct per §16.3 with defaults (spinSpeed 1.0, ballMass 1.0, friction 1.0, bounceRandomness 0.1, wheelTilt 0, targetZoneBias 0, predictionSize 0, multiball 0, splitPeg off, shotgunTime 0) | | |
| TASK-014 | Implement `Simulator::new(layout, modifiers, rng)` and `step()` (1/120 s): LAUNCH (wheel ω 2.0–3.5 × spinSpeed; ball ω −10…−15/√mass counter-rotating), RIM TRACK (centrifugal ω²r vs gravity bands 9.0, reduced 3.0 above r 0.92, 4.5 above 0.78), PIN/PEG CHAOS (8 deflector pins in radius window 0.75–0.93; bounce reverses ω × 0.45 + jitter × bounceRandomness × 18; divider pegs restitution 0.4/√mass), SETTLE (r ≤ 0.69, h ≤ 0.021, rel speed < 0.8, \|v_r\| < 0.15 → slot `floor((angleInWheel + halfSlot)/slotWidth)`); emit `SimEvent::{PegHit, DividerTick, BallSettled, SettleTimeout}` | | |
| TASK-015 | Implement cheat hooks (§5.3): friction scales both decays; ballMass into ω and restitution; wheelTilt directional gravity `sin(angleDiff) × tilt × 6.5` (deterministic sin); targetZoneBias angular pull `ω × 25 × bias × dt` + inward gravity ×8×bias toward highest-value bet zone (biasRedOnly/biasBlackOnly target color-filtered slots); nudge post-settle ±n toward nearest player bet; bounceRandomness jitter; multiball; splitPeg duplication (cap 6–8); shotgun 1.5 s → +4 balls | | |
| TASK-016 | Implement `run_to_completion()` (headless, step cap 7200 = 60 s) and `predict(rng_seed, modifiers) -> Prediction { sector_start, sector_size }` dry-run with **decoupled RNG substream (DEC-001/F1)**: prediction draws from `derive("prediction")`, live spin from `derive("physics")` — same spin parameters but independent launch jitter ⇒ sector is a *probabilistic band* (~85–95% hit-rate target), never certain knowledge; `predictionSize` width (first ball only) (REQ-003) | | |
| TASK-017 | Tests: golden seeds (same seed ⇒ identical settle slot + event-log hash); settle always yields a valid slot; bias 0.2/0.9/1.0 landing distributions; friction 0.3 vs 5.0 duration extremes; nudge ≤ ±n slots; multiball/splitPeg caps; determinism debug-vs-release; **prediction hit-rate < 100% at every width (DEC-001/F1)** | | |

## Phase 3 — Wheels, bets & board modifiers

- **GOAL-004**: Declarative runtime `WheelConfig` with effective-color resolution, the full bet vocabulary with data-driven payouts, and the modifier/board system (PAT-002).

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-018 | Implement `WheelConfig` runtime struct (cloneable, serde) from `WheelDef`: numbers, colors, `payout_multipliers { red, black, green, number, odd, even, gold?, purple?, cyan?, crimson? }`, upgrades; `effective_color(number, board)` honoring converts/paint/custom colors (§10.1 step 1) | | ✅ |
| TASK-019 | Implement `Bet { bet_type, amount }` with `BetType { Red, Black, Green, Number(u32), Odd, Even, Dozen(u8), Column(u8), Gold, Purple, Cyan, Crimson }` and win-check vs effective color/number incl. mirror-slot and extra-green ladder (32 @ ≥1; 11,22 @ ≥2; 5,17,29 @ ≥4, §4.4); payout base table from wheel config (2/2/14/36 defaults; 3× dozen/column; 4/4/4/6 specials, §4.2) | | ✅ |
| TASK-020 | ✅ `BoardModifiers` (serde struct, §16.3): customNumberMultipliers, convert lists, zone marks, globalMultiplier, streak counters/flags, insurance/riskCapital/goldenHeist/emeraldForest flags, doubleNextPayout — populated by `ModifierStack` entries, not loose booleans | | ✅ |
| TASK-021 | ✅ `ModifierStack`: `push(entry { source_card, kind, remaining_spins })`, `tick_at_round_end()`, typed `query()` accessors consumed by the damage pipeline; paint cards keep `marked_slots` on card instances (§6.3 note) | | ✅ |
| TASK-022 | Implement wheel customizer ops (§4.8): cycle slot color, add/remove slot (≥ 2 enforced), set unique number, save/cancel; board-upgrade application (§4.7: multiplier boosts, add green, converts, lucky_seven heal, light_ball/heavy_friction physics mods, ability unlocks) | | ✅ |
| TASK-023 | Tests: effective color precedence (convert > paint > base); green ladder; bet win matrix vs §4.2; customizer invariants (≥ 2 slots, unique numbers) | | ✅ |

## Phase 4 — Battle engine & the §10 damage pipeline

- **GOAL-005**: Points-mode battle state machine with verbatim §3 turn flow, draw ladder, bet accounting, and the ordered §10 resolution pipeline as pure functions.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-024 | Implement `BattleState` (§16.3): both sides' chip pools (player 30 / 15 under Greed) and scores, hand/draw/discard piles, active played cards (face-up, lock-at-spin flag), bets per side, phase `betting → spinning → resolved`, active wheel owner, round limit (3/5/8), sudden-death flag, drawsThisTurn, curse set, stun counters, freeCards flags | ✅ | 2026-09-14 |
| TASK-025 | Implement player actions: `play_card`, `draw_cards` (ladder 0/3/5/7/9 then +2; Avarice flat 3; hand limit 8 / 5 under Choked), `place_bet` (merge same cell+type), `remove_bet` (proportional refund), `clear_bets` (+ backup), `rebet`, `sacrifice` (−10 PTS → +5 ⚡), `spin` (empty bets = pass turn with confirmation flag) | ✅ | 2026-09-14 |
| TASK-026 | Implement `battle::resolve` pipeline (§10.1) as a pure function over `SpinInput`: (1) win detection on effective colors incl. green ladder / Emerald Forest (prime slots, ×2 green mult) / Green Ripple (+5 × green slots) / Eclipse void; (2) base payout `amount × mult`; (3) zone multipliers in order (low/high/dozen/column/primes, luckyZone ×1.5, goldFoil ×10, copperPlate ×1.5); (4) cheat penalties (prediction 0.3/0.5/1.0 by sector width 9–7/5–3/1; low-rarity physics ×0.3); (5) post-sum (doubleNextPayout, streak `min(4.0, 1 + 0.5×count)`, globalMultiplier, TURBO ×1.5, omniscience ×3); (6) `floor`; per-ball summation for multiball | ✅ | 2026-09-14 |
| TASK-027 | Implement resolve-time side effects (§10.2/§10.4/§10.5): GOLD converts landed + adjacent slot, +PTS/⚡ `15 + 5(L−1)`, +30 with Golden Heist; PURPLE +PTS and stun `1 + floor((L−1)/2)`; CYAN +PTS, pool refill to turn-start backup, draws `2 + (L−1)`; CRIMSON ×12 losing / ×6 tied-winning consolation + bonus PTS; streak tracking (§10.3) + HEAT COMBO (`3.5 + 0.5(L−1)`) / GLACIER SHIELD drain `10 + 3(L−1)`; zone triggers chipMine/lifeFountain/dangerZone/cursedZone; LUCKY_CHARM one-shot reroll policy; insurance full refund on all-zero; STUN_STRIKE ≥ 5 dmg → +2 stun; HEAVY_NUDGE all-zero → +15 ⚡ | ✅ | 2026-09-14 |
| TASK-028 | Implement enemy turn (§7): AI bets — EV enumeration over its wheel, optimal line with probability `difficulty`, up to 3 bets split ~50/30/rest, comeback risk tolerance scaling toward 1.0 when behind; enemy spin resolves into `enemyChipsPool`; on its win, execute queued intent (attack → player HP; steal_chips → drain player pool; physics_debuff → player physics modifiers; shield; block_red; buff); stuns skip turns | ✅ | 2026-09-14 |
| TASK-029 | Round end: tick modifier durations, Curse of Blood −2 HP, round limit → compare pools/scores, tie → sudden death extra round; emit `BattleOutcome { InProgress, PlayerVictory, PlayerDefeat, SuddenDeath }`; rewards roll per §2.5 (3/9/28/60 %) on node completion | ✅ (rewards roll deferred to Phase 6 run module) | 2026-09-14 |
| TASK-030 | Tests: seeded golden battle replays (full log hash); table-driven formula tests per §10.1 step; ladder/hand-limit edges; sudden-death loop; AI determinism; all 10 curse behaviors (Faraday zero bias, Fog no prediction, Rust friction ×2, Greed 15, Avarice flat 3, Fragile no heal, Eclipse green void, Blood −2/round, Lead cap 5, Choked hand 5) | ✅ 34 battle tests (97 total incl. Phase 0–3); golden replay + AI determinism + ladder/sudden-death + Greed/Avarice/Blood/Lead/Choked curse coverage; Faraday/Fog/Rust/Fragile/Eclipse behaviors are physics-layer concerns (Phase 7 hooks) | 2026-09-14 |

**Phase 4 gate**: `cargo fmt` clean, `cargo clippy -D warnings` clean, 97/97 tests pass (63 prior + 34 new battle tests).

Phase 4 notes / deferrals:
- CYAN pool refill uses `turn_start_pool` backup taken at `begin_betting`; draws granted 2+(L−1) on the ladder after the refill.
- Player card modifiers reach the pipeline via `ModifierStack::fold_into(&mut battle_board)` (battle-owned state is the base; card arms fold on top; converts after paints).
- §10.1 step 5 streak multiplier applies on the *current* spin from the count built by previous spins; HEAT COMBO/Glacier arm when the count reaches ≥3.
- Spin-side-effect points (GOLD/PURPLE/CYAN/CRIMSON consolation, zone triggers) are added to the spin payout (score = pool in points mode).
- LUCKY_CHARM exposed as a policy query (`should_use_lucky_charm` + `consume_lucky_charm`); the orchestrator re-spins and re-resolves.
- §2.5 reward rolling is a run-layer concern (needs the run's node state); scheduled with Phase 6.

## Phase 5 — Card effect execution (DSL dispatch)

- **GOAL-006**: All wave-1 cards playable through `EffectKind` dispatch at the §6.4 hook points, including hand surgery, the money sub-engine, and points-mode text rewriting.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-031 | Implement `cards::effects` dispatch: `EffectKind` × hook point (`OnPlayCard`, `BeforeSpin`, `OnBallLand`, `AfterSpinResolve`, `OnRoundEnd`) mutating `BattleState`/`BoardModifiers`/`PhysicsModifiers`; returns `Result<bool>` where `false` = play rejected (e.g. Identity Shift with no hand card) | | |
| TASK-032 | Implement payout/temp arming via `ModifierStack` (no per-card code): GREEN_GREED ×50 for 3 spins, dozen ×3.5 / column ×4, HIGH/LOW/EVEN/ODD ×2.5 for 3 spins, SINGLE_OUT ×40, prime ×3.5, fight-long crimson/dark surges, GOLDEN_ZERO customNumberMultipliers[0]=300, LUCKY_SEVEN 7→200, jackpot/devil trios ×250, DOUBLE_PAYOUT one-shot, GREEN_RIPPLE resolved at spin | | |
| TASK-033 | Implement hand/pile ops (QUICK_DRAW, HEAVY_DRAW, DECK_SHUFFLE, CALCULATED_RISK, GOLDEN_MIRROR 2×0-cost clones, COPY_PASTE, RETAIN_VISION, identity_shift) and money cards (essence_chip exile, chip_maker temp cards into draw pile, tax_refund discard scan, capital_venture deferred +25 on win, golden_heist +30 on gold landing) | | |
| TASK-034 | Implement `format_description(text, mode)` points-mode rewriter per §6.5 (deal damage → score PTS, etc.), applied at query/display time | | |
| TASK-035 | Tests: one golden replay per wave-1 card asserting exact state deltas; rejection paths; lock-at-spin behavior (temp card cannot be removed mid-effect) | | |

## Phase 6 — Run layer: map, economy, curses

- **GOAL-007**: Full run progression — map generation (§2.3), node resolution, shop/forge/events, curses, difficulty options, run-level state per §16.2 `RunState`.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-036 | Implement `RunState { hp/max_hp 80, chips 20, store_points 10, deck, player_wheel, color_levels, color_unlocks, curses, current_floor, map, current_node, combat_mode }` with serde snapshots | | |
| TASK-037 | Implement `map::generate(floors, rng)` per §2.3 exactly: floor 0 = 3 combat nodes (one per lane); middle floors 2–3 nodes with type roll order (floor%4==3 → elite; floor%4==1 && floor>1 → shop; else rand bands < 0.2 shop, < 0.4 event, < 0.55 forge, else combat); final floor = 1 boss; connections: closest-lane node next floor + 40 % second connection if ≤ 1 lane away; orphan fix; path rule (floor 0 any node pickable) | | |
| TASK-038 | Implement loadout store (10 PTS: 5 unique common cards + 1 common wheel), shop (§9.1: 6 cards + 2 wheels + Blood Infusion +25 HP/12 ⚡; price bands 8–13/14–21/25–35/45–60 ⚡ by rarity; offer cached per node), forge (§9.2: 3 free ops, reroll 5⚡, bronze/silver/gold 50/35/15, 6 op types, ≥2-slot rule, ⚙ CUSTOMIZE), event dispatch (choices → effects), curse application (elite 2, boss 2–3) | | |
| TASK-039 | Implement color levels & unlocks (§4.5): level cost `15 + 5(L−1)` ⚡ (max 10); payout scaling +0.2× red/black, +2.0× green, +0.5× specials; unlock effects Fever Ignition / Glacier Core / Synapse Link wired as modifiers | | |
| TASK-040 | Implement run flow API: `start_new_run(seed, difficulty)`, `pick_node`, node completion → floor advance → MAP / VICTORY / GAME_OVER; undo via snapshots (REQ-009) | | |
| TASK-041 | Tests: map shape properties (reachability, lane adjacency, node counts); shop price bands; forge economy; full-run golden test (seeded menu → victory with a fixed policy) | | |

## Phase 7 — Headless API hardening, CLI game, engine events

- **GOAL-008**: Stable documented public facade (`Engine`) where every mutation returns `EngineEvent`s and `Result`; rebuild the CLI on it, proving the engine is front-end-ready.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-042 | Finalize `roulette_core::Engine` facade: full command set (start/pick/play/draw/bet/clear/rebet/sacrifice/spin/purchase/forge/customize/choose_event/undo) + read-only snapshot getters; `EngineError` taxonomy (invalid state, insufficient chips, card rejected, content error); `EngineEvent` log per call (CardPlayed, BetPlaced, SpinStarted, BallLanded, DamageResolved, IntentExecuted, RoundEnded, NodeCompleted, ChipsChanged, …) (PAT-004) | | |
| TASK-043 | Rewrite `crates/roulette-cli` on the facade: points/damage mode select, map navigation, hand/bet/spin loop, draw ladder UI, spin report, shop/forge/event screens; `--seed`, `--script <file>` scripted input for CI smoke runs, `--non-interactive` demo | | |
| TASK-044 | Determinism contract test: same seed + same scripted input ⇒ byte-identical `EngineEvent` log (REQ-007 at API level) | | |

## Phase 8 — Monte Carlo simulator & balancer (§15)

- **GOAL-009**: Balance tooling replacing `tools/balance-cards.ts`: full-battle simulation and per-card EV studio able to audit and `--apply` cost changes into `content/cards.ron`.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-045 | Implement `simulate battles`: N seeded runs with configurable policy (bet strategy, card-play policy); report win rate, avg rounds, damage/⚡ distribution, throughput; two spin modes: `physics` (real sim) and `sample_mode = uniform` fast path (statistically equivalent uniform slot sampling, zero sim steps) for bulk runs | | |
| TASK-046 | Implement EV engine per §15: scenario presets (EUROPEAN_WHEEL, DEFAULT_BOARD_MODIFIERS), `evaluate_card(card, default_bets=[red/10], wheel, board)`, ΔEV vs baseline, efficiency (dmg/⚡), rating bands per §14.3 (common 2.5–10 / uncommon 6–22 / rare 12–45 / legendary 25–120) with ratings OP/BALANCED/UP/DEAD/PHYSICS-DEPENDENT, suggested cost | | |
| TASK-047 | CLI: `simulate battles --runs N --seed-prefix s`, `simulate cards [--apply]` printing the audit table (current → suggested cost, ΔEV, rating); `--apply` rewrites `content/cards.ron` costs; physics-type cards excluded from repricing (§15.2) | | |
| TASK-048 | Throughput benchmark (uniform mode) + physics-mode battle benchmark as CI smoke test (no strict gate); document achieved throughput in README | | |

## Phase 9 — Content wave 2, docs & polish

- **GOAL-010**: Port the remaining content, keep tooling in sync, and rewrite repo docs for the new architecture.

| Task | Description | Completed | Date |
|---|---|---|---|
| TASK-049 | Port the remaining ≈58 cards (full §6.3 table) and remaining 13 wheels (§4.6) into RON; run validator + EV audit; add `EffectKind` variants only for genuinely new mechanics discovered during the port | | |
| TASK-050 | Rewrite `README.md` and `ARCHITECTURE.md` for the workspace: crate map, determinism contract, content authoring guide (how to add a card/wheel/curse in RON), effect-DSL reference, CLI and simulator usage; replace stale WASM/Unity FFI section with a short "future wasm-bindgen binding" note; delete legacy `src/` tree remnants | | |
| TASK-051 | Final hygiene: fmt/clippy clean, `cargo test --workspace` green, content validation in the test suite, README migration note (prototype → v2 engine) | | |

## 4.1 Dependencies

- **DEP-001**: `serde` (derive) — content + snapshot serialization.
- **DEP-002**: `ron` — content data file format (Rust-friendly, commentable, diffable).
- **DEP-003**: `thiserror` — error types across crates.
- **DEP-004**: `proptest` (dev-dependency, `roulette-core` only) — property tests for physics/bets.
- **DEP-005**: `docs/ROULETTE_OS_GAME_BLUEPRINT.md` is the normative source for all constants (§2–§10, §16.2–16.3); tasks cite sections rather than restating every number.
- **DEP-006**: Future (out of scope, interface prepared): a `wasm-bindgen` binding crate consuming the `Engine` facade + `EngineEvent` stream.

## 4.2 Affected files

- **FILE-001**: `Cargo.toml` — workspace root (members, shared lints/profiles); `rust-toolchain.toml`, `rustfmt.toml`, task runner file.
- **FILE-002**: `crates/roulette-content/src/{lib,schema,validate,loader}.rs` — content crate.
- **FILE-003**: `content/{cards,wheels,board_upgrades,curses,enemies,events}.ron` — data files.
- **FILE-004**: `crates/roulette-core/src/lib.rs` — crate root listing all modules.
- **FILE-005**: `crates/roulette-core/src/rng.rs` — migrated RNG + `derive` substreams (replaces `src/rng.rs`).
- **FILE-006**: `crates/roulette-core/src/deterministic_floats.rs` — sin/cos approximations.
- **FILE-007**: `crates/roulette-core/src/phys/{mod,simulator,layout,events}.rs` — physics (replaces `device.rs` spin model).
- **FILE-008**: `crates/roulette-core/src/wheel/{mod,config,bets,customizer}.rs` (replaces `bet.rs`, `wheel.rs`).
- **FILE-009**: `crates/roulette-core/src/board/{mod,modifiers,zones}.rs`.
- **FILE-010**: `crates/roulette-core/src/battle/{mod,state,actions,resolve,draw_ladder,special_colors}.rs` (replaces `combat.rs`).
- **FILE-011**: `crates/roulette-core/src/cards/{mod,effects,hand}.rs` (replaces `cards.rs`).
- **FILE-012**: `crates/roulette-core/src/enemy/{mod,intents,ai}.rs` (replaces `target.rs`).
- **FILE-013**: `crates/roulette-core/src/run/{mod,state,map,shop,forge,events,curses,loadout}.rs`.
- **FILE-014**: `crates/roulette-core/src/api/{mod,engine,events,error}.rs` — facade.
- **FILE-015**: `crates/roulette-core/src/match_rules.rs` (replaces `mode.rs`).
- **FILE-016**: `crates/roulette-cli/src/{main.rs,screens/*}` — terminal game.
- **FILE-017**: `crates/roulette-sim/src/bin/{simulate,balance}.rs` + `ev.rs` (replaces `src/bin/simulate.rs`).
- **FILE-018**: `crates/roulette-core/tests/{golden_battle,golden_run}.rs`, `crates/roulette-content/tests/content_valid.rs`.
- **FILE-019**: `README.md`, `ARCHITECTURE.md` — rewritten; legacy `src/` tree removed at end of Phase 0 migration.

## 4.3 Implementation order & rationale

1. Workspace + RNG + determinism utilities first — everything depends on them.
2. Content schema before battle code: the `EffectKind`/`WheelDef` vocabulary freezes the seam all later phases code against.
3. Physics before battle resolution: §10 depends on per-ball landing; prediction depends on the dry-run.
4. Battle + damage pipeline before the full card port: cards are data once the DSL exists.
5. Run layer after battle: it composes everything; economy tuning needs a correct battle.
6. CLI/simulator last: tooling builds only on the stable public facade.

# 5. TESTING AND VALIDATION

## 5.1 Test matrix

- **TEST-001**: Determinism: golden-seed physics sims produce identical settle slots and event-log hashes; API-level replay test (TASK-044) is byte-identical across runs.
- **TEST-002**: §5 physics invariants: settle always lands a valid slot; launch constants (wheel ω 2.0–3.5 × spinSpeed; ball ω −10…−15/√mass) verified; pin window 0.75–0.93 respected; settle thresholds enforced; multiball caps 6–8.
- **TEST-003**: Cheat hooks: friction 0.3/5.0 extremes, tilt pull direction, bias 0.2/0.9/1.0 distribution shifts toward bet zones (statistical assertion over ≥ 5k spins), nudge ≤ ±n, splitPeg cap, shotgun +4 balls after 1.5 s.
- **TEST-004**: §10.1 pipeline table tests: each multiplier layer asserted independently, then composed end-to-end; prediction penalty ladder 0.3/0.5/1.0; low-rarity physics ×0.3; streak cap 4.0; crimson ×12/×6; floor rounding.
- **TEST-005**: Special colors per §10.2 with level scaling (gold convert + adjacent, purple stun formula, cyan refill + draws, crimson consolation).
- **TEST-006**: Draw ladder & hand limits: free 1st draw, 3/5/7/9 ladder, +2 continuation, hand 8 (5 under Choked), Avarice flat 3, Greed 15-pool start.
- **TEST-007**: Bet accounting: merge on same cell+type, proportional refund on remove, full refund on clear, REBET from backup, Lead cap 5 chips.
- **TEST-008**: Map generator properties (§2.3): node counts per floor, reachability (no orphans), connection probabilities, boss-only final floor.
- **TEST-009**: Economy: shop price bands per rarity (§9.1), forge 3 free ops + 5⚡ reroll + bronze/silver/gold 50/35/15, ≥2-slot invariant, event effects.
- **TEST-010**: Enemy AI: seeded intent pattern loops; EV-optimal bet chosen with probability = difficulty; stun skipping; intent execution effects (attack/steal/debuff/shield).
- **TEST-011**: Card golden replays: one per wave-1 card, extended to all 113 in Phase 9, asserting exact state deltas vs blueprint text.
- **TEST-012**: Content validation: rejects duplicates/unknown refs/underspecified wheels; all embedded content loads; RON round-trip idempotent.
- **TEST-013**: Fidelity checklist (§16.5) as an integration test: base scores 30/30, starting hand 2, no free auto-draw, sudden-death ties, payout-as-chips scoring.
- **TEST-014**: Simulator smoke: `simulate battles --runs 2000` completes with stable output for the fixed policy; `simulate cards` audits every card; `--apply` produces only cost-field diffs in `content/cards.ron`.
- **TEST-015**: Hygiene gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all green; content validator runs inside the test suite.

## 5.2 Definition of done

- A player can complete a full seeded run (menu → loadout → map → combat/elite/shop/event/forge → boss → victory or game-over) through the CLI, with correct points-mode scoring, curses on elite/boss floors, and reward rolls — deterministic per seed.
- All §16.5 fidelity items demonstrably hold in tests.
- Adding a new card = adding a RON entry (+ validator pass) with zero engine edits; adding a genuinely new mechanic = one `EffectKind` variant + one dispatch arm + tests.
- The simulator reproduces §15 outputs (audit table with ΔEV, efficiency, rating, suggested cost) and can rewrite card costs in place.
- Docs describe the new architecture; legacy prototype modules are gone.

## 5.3 Risks & assumptions

- **RISK-001**: Physics tuning risk — verbatim §5 constants may behave differently in a new implementation; mitigated by keeping all constants in one parameterized module, golden tests, and easy adjustment.
- **RISK-002**: `EffectKind` vocabulary may prove incomplete for some of the 113 cards; mitigated by the wave-1 subset covering every §6.4 hook before wave-2 porting (TASK-047 may add variants).
- **RISK-003**: f64 physics determinism across wasm/native — mitigated by custom sin/cos (REQ-007); transcendental-free design avoids residual libm divergence.
- **RISK-004**: Data-entry errors in the 113-card table; mitigated by RON schema validation + `simulate cards` audit + review against §6.3.
- **RISK-005**: Scope creep toward the web frontend; enforced out by CON-003 (the facade/events make the later binding thin, but no rendering work happens in this plan).
- **ASSUMPTION-001**: Blueprint constants (§2–§10) are the authoritative balance target; where the prototype Rust engine differs (e.g. payout multipliers), the blueprint wins.
- **ASSUMPTION-002**: Single-player headless usage; networking and persistence are out of scope (snapshots are serde-serializable to enable save/load later).
- **ASSUMPTION-003**: `docs/ROULETTE_OS_GAME_BLUEPRINT.md` remains the living in-repo spec; code and tests cite it by section.
