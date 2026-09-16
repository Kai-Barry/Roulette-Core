# Roulette-Core — repository memory

## Project
Rust rebuild of the `roulette_core` engine for *Roulette.OS* per
`docs/ROULETTE_OS_GAME_BLUEPRINT.md` (normative for all constants) and
`.agents_tmp/PLAN.md` (phased build plan, 4-crate workspace:
roulette-content / roulette-core / roulette-cli / roulette-sim).
Principle: *content is data (RON), mechanics are code, seam is a typed EffectKind DSL.*

## Environment
- Rust stable via `rust-toolchain.toml`; `source ~/.cargo/env` first in shells.
- Gate: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` (also `check.sh`).
- RON gotchas learned the hard way: maps use `{}`, enum-tuple variants `name(args)`,
  struct variants `name(field: val)`, `[T; N]` arrays deserialize from RON tuples `(...)`
  not lists, floats spell `NaN` not `nan`.

## Status (2026-09-14)
- Phase 1 (GOAL-002) complete: content crate + 7 RON files (107 cards — all match
  blueprint cost/rarity, verified programmatically; 8 wheels; 18 upgrades; 10 curses;
  6 enemies; events; forge ops). 15 content tests green, clippy clean.
- Design-lens audit done: `.agents_tmp/design_audit.md` (findings F1–F5).
- Phase 2 (GOAL-003) complete: `roulette-core/src/phys/` — WheelLayout,
  PhysicsModifiers (incl. `prediction_accuracy`), 120 Hz Simulator with
  LAUNCH/RIM/PIN/SETTLE phases, all §5.3 cheat hooks, `run_to_completion()`,
  `predict()`/`predict_from_master()`, `SimEvent` stream, `golden_spins` debug-vs-
  release determinism tool. 17 physics tests green; workspace total 43, clippy clean.

## Decisions
- **DEC-001 (user-approved, refined during Phase 2):** prediction is probabilistic,
  NEVER certain. Pure RNG-stream decoupling fails (chaotic divergence → hit-rate ≈
  chance ≈ 2.7%); the implemented model is: honest dry-run on the live `derive("physics")`
  stream + an **oracle-honesty roll** on `derive("prediction")` — with probability
  `PhysicsModifiers::prediction_accuracy` (default 0.90) the band centers on the true
  landing, else on a decoy dry-run. Hit rate is a direct tunable knob, always < 100%.
  See `docs/DECISIONS.md`. Test: `prediction_never_matches_with_certainty`.
- Elite `dealers_claw` difficulty fixed 0.6 → 0.85 (blueprint §14.2).
- Validator rejects non-finite payouts (RON parses `NaN`; `x <= 0.0` is false for NaN).
- Open: F3 (score pool semantics — pin before Phase 5), F4 (enemy max_hp
  vestigial in points mode). F2 fixed in Phase 3 (color dominance via
  effective-color resolution). F3 resolved for Phase 4: in points mode the
  score IS the chip pool (§10.4 payouts return to the pool).

## Phase 2 (GOAL-003) notes
- Constants live verbatim in blueprint §5.1/§5.2; do not restate — cite sections.
- Physics must use `deterministic_floats::sin_approx/cos_approx` (REQ-007, no libm).
- RNG: mulberry32 with `derive(label)` substreams (GUD-004); physics and prediction
  substreams must stay independent (DEC-001).
- Wheel layouts come from `roulette-content` `WheelDef`; sim is headless with
  `run_to_completion()` (step cap 7200) and emits `SimEvent`s.
## Phase 4 (GOAL-005) notes
- Pipeline contract: `resolve_spin(SpinInput)` is the single §10.1 entry; board
  base = battle-owned `player_board` clone, then `player_stack.fold_into(&mut
  board)` (converts after paints). Persist the mutated board back post-spin.
- Streak multiplier applies on the current spin from prior count; HEAT COMBO /
  GLACIER SHIELD arm when count reaches ≥3 (§10.3).
- Spin side-effect PTS (GOLD/PURPLE/CYAN/CRIMSON consolation, zone triggers)
  fold into spin payout; score == pool in points mode.
- LUCKY_CHARM is a policy query (`should_use_lucky_charm`/`consume_lucky_charm`);
  the orchestrator re-spins + re-resolves. Insurance refunds full stake on
  all-lost spin (§10.5).
- Enemy: `enemy_choose_bets` EV enumeration (optimal with p=difficulty, else
  random valid bet), ~50/30/rest split, comeback risk scaling; intents execute
  only on a winning enemy spin (§7.3); stuns skip turns.
- Round end: `end_round()` ticks Round-scope modifiers, applies Curse of Blood,
  compares pools at the limit; tie → sudden death extra rounds.
- Reward rolls (§2.5) deferred to run layer (Phase 6).

## Phase 5 (GOAL-006) notes — card effect execution via typed DSL
- `cards/` module: `format.rs` (§6.5 points-mode rewrite; **`PTS` is an invariant acronym — never recased to lowercase**), `effects.rs` (typed DSL dispatch), `mod.rs` (hooks).
- Hook seams: `on_play_card` (per effect, ordered), `after_spin_resolve` (file played cards to discard, temp→exile, clear spin-scoped stack entries, tick Spins, expire board arms + reset physics), `on_round_end` (CapitalVenture banks ONLY on round win, then clears).
- `resolve_spin` is snapshot-based: `merged_board(side)` folds the stack into a local board; **write-back must carry ONLY battle-owned fields** (streak counts, block_red, stun_strike, heavy_nudge, insurance, double_next_payout). Writing the folded board back double-applies stack arms on the next spin (36-vs-12 bug class).
- HEAT COMBO heat multiplier writes `player_board.payout_multipliers` directly (battle-owned) and reverts on streak reset.
- Bet arms: Red/Black/Odd/Even/Dozen/Column **and Green** fold `payout_multiplier_for`; `effective_color` applies `swap_red_black` LAST (red↔black only, after all converts).
- `ModifierStack` scoping (§6.3): `Spins(n)` ticks **per spin** via `tick_after_spin` ("next N spins"), `Round` clears via `tick_at_round_end`, `Spin`/`NextWin` via `clear_spin_scoped`. Converts land in the paint layer unless fight-scope (fight-scope ones also into `converts`).
- `AddGreenSlot` ignores numbers not on the wheel (no phantom slots). `Rng::range_usize(min, max)` is INCLUSIVE of max.
- Landed side effects (§10.2): gold = +15 PTS + gold adjacent + heist; 12 is canonical red (CANONICAL_RED table in wheel/mod.rs) — bet math in tests must use it.
- Card costs charge before effects (`pay_card_cost` → `on_play_card`); percent gains resolve on the post-cost pool. Played cards sit on `active_played` (the felt) until spin end — never assert discard membership at play time.
- Status: **142 tests / 0 failures / 0 clippy warnings** across workspace; commit 0664360 on main. Next: Phase 6 (run layer: reward rolls §2.5, progression).

## Phase 6 (GOAL-007) notes — run layer
- `run/` module: `map.rs`, `state.rs` (RunState + flow API), `rewards.rs`, `shop.rs`, `forge.rs`, `events.rs`.
- Map gen (§2.3): floor 0 = exactly 3 combat nodes (lanes 0/1/2); middle floors 2–3 nodes, lanes = sorted distinct picks from {0,1,2}; type roll ORDER matters — `floor%4==3` elite, `floor%4==1 && floor>1` shop, else roll 0.2 shop / 0.4 event / 0.55 forge / else combat; last floor = single boss lane 1. Connections: closest-lane next floor ALWAYS + 40% second link when ≤1 lane away; orphan-fix adds a donor edge from the closest-lane prev-floor node (reachability proof = BFS test over 20 seeds).
- §4.5 level cost is "15 + 5(L−1) (15, 20, 25, …)" with **L = the level being LEFT** — i.e. reaching `target` costs `15 + 5(target−2)`. First upgrade (to 2) = 15. (Blueprint wording is ambiguous; the "(15, 20, 25…)" sequence disambiguates.)
- RunState per §16.2: `relics: Vec<String>` exists as a placeholder ("field exists; unused in v1" per §16.2 relic note). Chips ⚡ = combat pool AND shop currency (§2.5 shared pool) — `complete_battle` folds `chips_pool` (+ Capital Venture bank) back into `run.chips`; a bankroll above the 30 floor rides IN via `enter_battle` (battle starts at max(30, carried)).
- Battle fold-back: score defeat with HP>0 → node stays OPEN (retry), `state=Map`; HP==0 → `GameState::GameOver`; boss beaten → `GameState::Victory`. PLAY AGAIN = engine-level `start_new_run` (headless: UI returns to Menu).
- Shop (§9.1): offer cached per node (`run.shop_offer`), cleared on `complete_node` (RETURN TO PATHS clears cache). Fragile → `RunError::HealingBlocked` for Blood Infusion; heal caps at max_hp (tests must set headroom).
- Forge (§9.2): `apply_op(wheel, op, green_level, rng)` — destroy_random needs the rng param (≥2-slot rule checked per iteration). Optional payout-multiplier fields bump with defaults gold/purple 4.0, cyan/crimson 6.0.
- Rewards (§2.5): each pick rolls its own rarity; bands fall down the table when dry; picks dedupe across the set.
- serde_json added as workspace dep (REQ-009 snapshot/restore on RunState).
- Nested `pub enum` inside `impl` blocks is illegal Rust — keep such enums at module level (BattleResult pattern).
- Status: **169 tests / 0 failures / 0 clippy warnings** across workspace; commit 350c34b on main. Next: Phase 7 per PLAN.md (physics integration pass — 120Hz, §13).

## Phase 0 (3D frontend plan) notes — AI harness + wasm
- `crates/roulette-wasm`: EngineHandle binding — `new_engine(id)`, `apply_command(cmd_json) -> events_json | Err(error_json)`, `get_state_json()` → `{seed,game_state,run,battle,undo_depth}`, `undo()`, `undo_depth()`. Build: `wasm-pack build --target nodejs --out-dir pkg --release` from the crate dir (~35s); Node loads `pkg/roulette_wasm.js`. wasm panics POISON the instance ("recursive use of an object") — the JS harness then throws on every call; fix panics at the Rust side (serde expectations), never rely on catching them.
- **serde trap (fixed)**: `EngineEvent` is internally tagged (`#[serde(tag="event")]`) — newtype variants holding plain strings (CardGained(String) etc.) panic at serialize time; they are struct variants now (`card_gained {id}` etc.). Keep it that way.
- Node harness (`web/test/`): `lib.mjs` = EngineSession + `candidatesFor(state)` (state→legal-command candidates) + `mapCandidates` (mirrors engine `pickable_nodes`); `matchers.mjs` (TASK-002 expectEvents/expectEvent tail-subset matchers); `invariants.mjs` (TASK-004 chips/hp/deck/hand/bets/undo-depth/event-ordering); `node-wasm.mjs` boots and smoke-verifies all three layers (TASK-001). Run: `node web/test/node-wasm.mjs`.
- Bots (`tools/ai-play.mjs`): `--seed --policy random|greedy|scripted FILE --max-commands --json --difficulty --campaign`. Exit 0 iff terminated + invariants held. Determinism verified: repeated greedy runs byte-identical (REQ-006).
- **Map node-screen exit contract**: there is NO Leave command — `pick_node` with a connected node id exits shop/forge/event screens (CLI `pick` does the same). `pickable_nodes` = floor 0 when no current node, else current node connections + (un-completed current node itself ONLY when `state==Map`, i.e. retry after score-loss walk-away; re-picking from inside a node screen would regenerate offers forever — guard is in `run/state.rs::pickable_nodes`).
- Betting symmetry trap: with mirrored wheels, both sides betting the same type every round can tie into an infinite sudden-death loop; lib.mjs alternates red/black by `battle.round` parity to break it.
- Battle snapshot keys: `phase,turn,bets,enemy_bets,chips_pool,enemy_chips_pool,hand,active_played,draw_pile,discard_pile,player_board,enemy_board,player_wheel,enemy_wheel,player_hp,enemy_hp,enemy_intent,curses,physics,is_sudden_death,round,max_rounds`. Run keys: `hp,chips,deck,curses,difficulty,map,current_floor,current_node,shop_offer,forge_offer,loadout_offer,color_levels,color_unlocks,combat_mode`. Map nodes: `{id:"f<l><lane>",floor,lane,node_type,connections,completed}`.
- Status: workspace cargo suite green (incl. stun regression); Node harness green (TASK-001/002/004); greedy t1=VICTORY 134 cmds hp24; random t1 terminates naturally. Commit 02a754d on feat/roulette-cli-front-end. Next: remaining 3D frontend phases per plan/feature-3d-frontend-1.md.

## 3D frontend phases (plan/feature-3d-frontend-1.md) progress
- Phase 2 commit 3dd7de6 (GameClient core), Phase 3 commit 6e926e4 (vscreen + screens), Phase 4 commit 0bf041b (RenderManager/WheelVisual/dressing/fx/picking).
- Phase 5 commit 7a51581 (audio, TASK-028..030 + TEST-013). Key facts:
  - Timed sim events: `simulator.rs::run_to_completion_with_telemetry_timed` returns `Vec<(u32, SimEvent)>` (frame index = telemetry frame idx, uniform fast path → empty Vec); engine stores both spins' tracks under `telemetry` feature; JSON accessor `spin_sim_events_json` (api/mod.rs, next to `spin_telemetry_bytes`); wasm export `spin_sim_events(side)` (0=player, 1=enemy). Client: `GameClient.spinSimEvents(side) -> TimedSimEvent[]` ({frame, event}).
  - Sound sync contract: entry sets `rm.onPlaybackFrame = f => sound.tickPlayback(f)`; SoundManager.queueSimEvents(sorted-by-frame) fires each event exactly once as the playback cursor crosses its frame (TEST-013i asserts exactly-once counts).
  - SoundManager (web/src/audio/SoundManager.ts) is headless-safe: no AudioContext in Node → recipes only `record()` into `played`; `diagnostics()` = {ctx, buses, recipes, lastPlayed, simQueue, music}. Buses music/drone/sfx read from persisted SettingsStore (defaults .55/.15/.8 §12.9); `syncVolumes()` pushes store → gains.
  - EncounterMusic (web/src/audio/music.ts): lookahead sequencer (25ms tick/100ms ahead), `TIER_TEMPO=[96,108,126,144,160]`, boss = ×1.1 push + drums layer; layers bass always, arp tier≥2; tier derives from engine `battle_started` event (normal/elite/boss → 0/2/4) — entry tracks `lastBattleTier`, never invents audio state (REQ-002 analogue).
  - RoundOutcome mapping trap: engine outcome is 'player_victory'|'player_defeat'|'sudden_death'|'in_progress' — NOT win/lose; map in SoundManager.onEvents.
  - Tests: `node web/test/audio.test.mjs` (13 gates TEST-013a..m). pkg/pkg-web built BEFORE the unused-`mut` warning fix — rebuild only needed if Rust surface changes again.
  - Untracked leftovers (do not commit): `.agents_tmp/design_audit_ui.md`, `package-lock.json` (node_modules/dist/target gitignored).
