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
