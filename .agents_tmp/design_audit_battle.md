# Roulette.OS — Design-Lens Audit II: Battle System (empirical, post-Phase-7)

Scope: the implemented roulette-core battle loop as shipped (commits up to
`02a754d`/`7a51581`), audited **empirically** through the headless WASM harness
(REQ-004: `?headless=1` contract, `web/test/lib.mjs` + `uiCandidates`), not from the
blueprint text. Companion to the pre-implementation audit `.agents_tmp/design_audit.md`
(that report reviewed the *spec*; this one reviews the *built system*). Method: Schell
lenses, cited as #N. Evidence: 30-seed sweeps per policy, 40-spin round-level traces,
full-run playthroughs, all via legal Commands only (REQ-005), fixed seeds (REQ-006).

**Headline: the design is mechanically coherent but currently unwinnable as a run.
Every policy tested — 12 bet policies — wins ≤ 30% of battles, and 0/30 full runs
reached Victory. The enemy's pool compounds while the player's optimal play is flat;
the retry loop converts every lost battle into 6–9 HP, and HP runs out before a run
completes.**

---

## 1. Measured baseline (the numbers this report stands on)

Single battle, floor-0 normal enemy (30 seeds each, 3–8 rounds, legal commands only):

| Policy (all legal) | Battle victories | Notes |
|---|---|---|
| red flat 10 | **9/30** | best policy found |
| red flat 5 / 15 / 20 / 30 | 2 / 5 / 6 / 4 | worse at every other stake |
| odd flat 10 | 1/30 | parity skew (see B3) |
| even flat 10 | 3/30 | |
| dozen-2 stake 5 / 10 / 15 | 2 / 3 / 2 | pool balloons on miss (B6) |
| near-all-in red 25 / 28 / 29 | 8 / 9 / 9 | no better than flat |
| martingale (double on loss) | 4/30 | worse |
| red+black hedge (all-in both) | 3/30 | worse |
| fractional 25% / 33% / 50% | 3 / 4 / 2 | **compounding loses to flat** |

Round-level (40 spins): enemy lands a payout 15/40 (37.5%), avg 11.7 pts/round;
landed distribution skews **BLACK 55% / RED 38%** and **EVEN 57% / ODD 35%**.
Player HP loss occurs **only** in rounds where the enemy payout > 0: 6–9 HP per
failed battle. Full-run sweep (30 runs, flat-10-red + walk-away retries):
**0 victories**, 45–49 battles each, all ended `hp 0, game_over`.
Single-battle victory verified reachable (P42E30 → P54E30 → victory; rewards `low_sweep`).

---

## 2. Gameplay-blocking findings

### 🔴 B1 — The run is unwinnable under every policy tested (Lens #33 triangularity, #40 punishment)

The retry loop (score defeat with HP>0 → node stays open, per `complete_battle`)
converts chip losses into HP losses: each failed battle re-arms enemy intents whose
execution on winning enemy spins costs 6–9 HP. At a ~30% best-case battle win rate the
expected failed battles per run (~8 nodes + retries) ≈ 2 per success → **~100+ HP of
bleed against an 80 HP pool**. Measured: every one of 30 full runs died at hp 0 after
45–49 battles. This is a death spiral, not a difficulty curve: losing doesn't teach or
reroll — it just drains the one resource that matters while chips reset to the 30
floor for free.

**Fix direction:** one or more of (a) raise flat-policy battle win rate into a
40–60% target band (enemy difficulty/commit curve), (b) make intents cost the enemy
something (intent executes only on *winning* enemy spin is already the rule — but the
enemy wins 37.5% of rounds with a ~11.7-pt avg payout; either rate or magnitude must
come down), or (c) give score-defeat a HP-neutral retry cost (e.g. chips floor drop)
so HP isn't the sole failure currency.

### 🔴 B2 — Enemy compounding vs. flat-player asymmetry (Lens #29 feedback loops)

The enemy commits **10–50% of its own pool** per round (risk-scaled,
`enemy_choose_bets`, ~0.1+0.4·risk of pool) on positive-EV 3× dozen/column lines, so
its expected earn **grows with its pool**: compounding. The player's empirical optimum
is a **flat** 10-chip red bet — every compounding-style player policy (fractional
Kelly 25/33/50%, martingale, all-in, hedge) measured *worse* than flat (2–4/30 vs
9/30), because one miss at a large stake craters the pool into the walk-away cycle
while a flat 10 never does. Result: the two sides run different math — enemy EV/round
rises with its pool (victories observed only when the enemy stayed poor: enemy pts
48–50 at player victory), player EV/round is constant. The "beat the table by cheating
harder" fantasy (audit-I §1) has no mechanical channel here yet: cards/upgrades exist
but the *betting* layer offers no compounding path that survives a miss.

**Fix direction:** mirror the risk-curve: let the player scale stakes with their own
pool without catastrophic downside (e.g. stakes refund on loss up to a band, streak
multipliers already in §10.3 HEAT COMBO made meaningful), or anti-snowball the enemy
(commit fraction shrinks as its pool grows). Currently the enemy snowballs and the
player cannot.

### 🟠 B3 — Landed-distribution skew makes bets secretly unequal (Lens #28, #59 readable-odds)

40-spin landed distribution: BLACK 55% vs RED 38%, EVEN 57% vs ODD 35% (odd bets
1/30 in battle sweeps vs red 9/30 — consistent with the skew). Absent physics cheats
the wheel should land ~uniform; whatever the source (wheel physicals, seed family, or
settle-phase bias), the player has **no way to observe it** — odds *look* symmetric
(§ pillar 5 readable odds with corruption). If skew is intentional corruption, it must
be surfaced (prediction band, heat-map); if not, it's a physics tuning bug. Either
way, "bet red" being secretly 3× better than "bet odd" violates the game's own
readable-odds pillar and makes the strategy layer a guessing game.

### 🟠 B3b — Sudden-death same-type tie can loop forever (Lens #21, REQ-005)

`end_round` tie → sudden death extra round (round_end.rs:93), with no cap. With
mirrored same-type betting both sides can tie indefinitely → **non-terminating battle
under a fixed legal policy**. AGENTS.md already documents the harness workaround
("lib.mjs alternates red/black by round parity to break it"); this session one flat-red
sweep hit a suspected loop seed (1/30 anomaly + a harness hang at 120-battle cap).
An engine that can fail to terminate under legal play breaks the AI-playability
contract (REQ-005) and any fixed-policy balancer run. Fix: cap sudden death (e.g.
3 extra rounds → tiebreak by curse count / coin), or break ties by stake diversity.

---

## 3. Observability gaps (REQ-004/005/006 — AI & replay sufficiency)

The engine event stream (`run_started → node_picked → battle_started → bet_placed →
spin_started → ball_landed → spin_resolved → intent_executed → round_ended →
battle_ended`) is well-shaped, but three payloads are insufficient for an AI (or a
spectator/replay UI) to reconstruct *why* things happened:

### 🟠 B4 — Enemy bets are invisible
`BetPlaced` fires only for the player's `Command::PlaceBet` (api/mod.rs:439); enemy
bets are chosen internally and **cleared at resolve without any event**
(resolve.rs:283; "none logged" across 6 traced battles). Consequence: on every enemy
payout the observer sees pools jump (E30→E72, E30→E83…) with no way to attribute it.
This also blocks any player-side "what is the house betting?" read — the single most
thematic piece of information in a gambling duel.

### 🟠 B5 — `IntentExecuted` carries no payload
The event is a bare tag; the actual intent (`EnemyIntent { action, value,
description, pattern_index }` — state.rs:63) is *in state* but not in the event. The
HP loss lands with zero explanation. Fix: emit the intent fields
(`intent_executed {action, value, description}`) — trivial, and it directly serves
REQ-005 (AI needs the HP-cost attribution) and the 3D frontend's intent display.

### 🟡 B6 — Pool ballooning on dozen misses is untracked
Dozen misses with 15-chip stakes produced player pools of 156→390 in trace seeds;
`RoundEnded` carries only `{round, player_pts, enemy_pts, outcome}`. That's *enough*
to audit ex post, but with B4 (no enemy bets) the *cause* of a swing is
unreconstructable from events alone. Also note the trace anomaly: pools read
`undefined` when read after `battle_ended` (state folded/cleared) — harness scripts
must read pools from `RoundEnded` events, not the (null) battle snapshot.

---

## 4. What works (keep)

- **Single-battle economics are sound in isolation:** player EV on a fair 37-wheel
  (+0.457/chip red) vs enemy per-chip EV (+0.297 dozen) means the *player* has the
  better raw line — the problem is purely stake-shape (B2), not payout tables. Payout
  constants (§10.1), color levels (0.2 steps, MAX 10), and card multipliers verified
  correct end-to-end: `dark_fury` ×2.5 measured exactly (10 → 25/27 on black hits,
  stack folds fight-scoped per Phase-5 note).
- **Event ordering contract** (TASK-042) holds across all sweeps; determinism
  (REQ-006) holds — repeated seeds byte-identical.
- **Enemy AI shape** (EV enumeration, difficulty-gated optimality, comeback risk
  scaling, 4-move intent patterns) is genuinely good design — readable, fair-ish, and
  thematically on-point. The *stake curve*, not the AI, is the problem.
- Map gen (§2.3) verified: floor 0 = 3 combat lanes; elite/shop/forge roll order as
  specced; Short 7 / Medium 11 / Long 15 floors.

---

## 5. Prioritized recommendations

| # | Action | Where | Cost |
|---|---|---|---|
| 1 | B1: retune enemy hit-rate or intent magnitude so flat-policy battle win rate lands 40–60%; make win-rate-by-floor a balancer-reported metric (audit-I §5) | balance pass | medium |
| 2 | B2: anti-snowball enemy stake curve OR player compounding channel (HEAT COMBO as the vehicle) | enemy.rs + §10.3 | medium |
| 3 | B3b: sudden-death cap (3 rounds → deterministic tiebreak) | round_end.rs | small |
| 4 | B4/B5: `intent_executed {action,value,description}` + enemy bet exposure in `spin_resolved`/`RoundEnded` (or a dedicated `enemy_bets` event pre-clear) | events.rs | small |
| 5 | B3: audit landed-distribution skew; surface per-color/base-rate bias in UI or fix physics | physics | medium |
| 6 | Retry-cost clarity: surface HP bleed per failed battle in `RoundEnded` (e.g. `hp_delta`) so the real resource is legible (Lens #40) | events.rs | small |
| 7 | Harness note: guard `chips_pool` reads after `battle_ended` (null battle) in test scripts; reconcile the 1/30 bet-10-red outlier (suspected B3b loop seed) vs the stable 9/30 | test infra | trivial |

---

## Verdict

Theme and mechanics remain audit-I's strengths, and the enemy AI is the best-designed
system in the codebase. But the **battle layer currently fails its own essential
experience**: the player cannot compound, the house can, punishment flows through a
resource (HP) the surface never explains, and the run is empirically unwinnable
(0/30). None of this is structural to the design — it's a balance pass plus an
event-payload pass, both small and both prerequisite to the 3D frontend being able to
*tell the story* of a round (REQ-002's "never invent" rule needs the engine to emit
the why, not just the what).

Fix order: B3b (termination) → B4/B5 (observability) → B1/B2 (balance curve) → B3
(distribution audit), then re-run the 30-seed full-run sweep as the acceptance gate
(target: ≥3/30 full-run victories under flat play, 40–60% battle win rate).