# Full Playthrough Notes — Roulette.OS (2026-09-14)

Playthrough performed against the real wasm engine (`roulette-wasm` pkg, Node harness
`web/test/lib.mjs`), using **only legal `Command`s** (REQ-005). Drivers written for this
session: `/tmp/playthrough.mjs` (v1), `/tmp/playthrough2.mjs` (v2, fixed). Lenses cited
from `.agents/skills/game-design-lenses/`.

## How it was played

| Run | Seed | Policy | Result |
|---|---|---|---|
| Greedy baseline | notes_greedy_1 | greedy (repo bot) | victory, 84 cmds, 45/80 HP |
| Deliberate "cheater" | notes_v2_1 | prediction-guided band bets + full economy | **victory**, 164 cmds, 27/80 HP, all 7 floors |
| Pass-only "doomer" ×10 | doom_1..10 | spin with no bets, no cards, no draws | **7/10 VICTORIES** |
| Cheater ×5 | cheat_1..5 | deliberate | 4/5 victories, two runs at ≤ 27 HP |
| Repo gate | — | `tools/ai-check.mjs` | **FAILS** (greedy seed 3, command cap) |

Deliberate-run arc: loadout draft (5 cards + wheel, 10 PTS) → floor-0 combat **defeat →
retry** (−6 HP) → THE HOODED SPECTRE event (inject_syringe: −8 HP +14⚡) → elite (5-round,
Avarice + Curse of Blood) → shop → boss (8 rounds, 220 HP). Determinism verified: the same
seed replays byte-identical event streams.

## Findings (ordered by severity)

### F1 — Pass-only is a dominant strategy (Lens #38 challenge, #12 problem, §3.5)
Spinning with no bets every round won **7/10 seeds** (SHORT). Mechanism: the enemy's bets
are negative-EV against its own wheel (e.g. repeated `green` bets at 2/38 on the
double-zero wheel) so its 30-chip pool bleeds to ~15 by the round-limit comparison, while
the player's untouched 30 "wins the table". HP drains only from attack intents (6–9/round),
which the 80-HP budget absorbs across a whole run. The core pillar "you must cheat to win"
is currently inverted: *not playing at all* is the safest strategy.
**Fix levers (blueprint-compatible):** a per-round **table minimum** (real-casino framing:
passing still costs chips); enemy bets chosen with positive EV on its wheel ("the house
cheats too" — on-theme); or intent escalation when the player passively passes.

### F2 — `essence_recycle` ("Identity Shift") is an infinite no-op loop (engine bug, breaks TEST-005/REQ-005)
Card text: "Discard a card from your hand to draw 1 card for 0 cost."
Observed: played **3,000+ times consecutively** — hand stays `["essence_recycle"]`, pool
unchanged, no discard, no draw, `card_played(cost:1)` event spams each time. Root cause:
`HandOp::DiscardFromHandDrawOne` (cards/effects.rs) returns `Ok(false)` when the hand holds
only the played card; the play is then refunded and the card returned to hand → identical
state, forever. This is why `ai:check` **fails today** (greedy seed 3 hits the 5,000-command
cap at floor 1).
**Fix:** treat a no-op effect play as an illegal command (verbatim `EngineError`, no
event), and/or exclude the played card from the draw-refill path; add a regression test:
"no command may be applied twice in a row with zero state delta" (extends TASK-033's
softlock sweep).

**FIXED (this session)** — implemented as resolve-not-reject, not the illegal-command
route (an engine error would make a *legally-present* card unplayable, and the client
has no rejection UX yet). Changes:
- `cards/effects.rs`: `DiscardFromHandDrawOne` with the played copy as the only hand card
  now resolves — the played copy `temp`s (exiles itself, skipping both felt→discard
  cleanups) and `draw_free(1)` cycles the deck. Solo play = hand shrinks by the played
  copy, deck cycles by one. Normal 2+ card case unchanged.
- `api/events.rs`: new `card_play_rejected { card_id, cost }` event, emitted by
  `Command::PlayCard` on `Ok(false)` — rejections are now observable (design-audit B2:
  no silent no-op plays can loop invisibly again; Recycle Bin etc. still legitimately
  reject).
- TS contract `web/src/engine/schema.ts` updated (EVENT_TAGS + EngineEvent union) —
  TEST-007 snapshot gate passes.
- Regression test rewritten in `tests/cards.rs` (`retain_vision_and_recycle_bin_and_identity_shift`):
  solo play must resolve (Ok), leave hand, not re-enter discard, pool charged once (49/50).
- Verification: full workspace `cargo test` green; `npm run ai:check` **PASS** (6/6 runs;
  greedy s3 went from 5,001-command stall → victory in 116 commands). Harness bots still
  retry rejected plays (their loop is policy-level, tolerable within the 5,000 cap); a
  harness-level rejected-play cooldown is optional polish, not a gate.

### F3 — The flagship "cheat" verb (`predict`) is not wired into the UI (Lens #33, GATE #56; TASK-018 gap)
The engine exposes `predict`, `GameClient.predict()` exists, and `prediction_shown` renders
when it appears — but **no screen/candidate dispatches `predict`** (`web/src/ui/screens.ts`
renders the band; nothing issues the command). Also, base wheels have **no prediction**
(the engine answers `prediction_unavailable: wheel has no prediction upgrade`), and nothing
in the UI communicates that availability or its cost. Human players currently cannot use
the core pillar-2 mechanic without playing a physics card.
**Fix:** add `predict` to combat-screen candidates (with availability state), surface the
§10 penalty ("landing inside your own band pays ×0.3–0.5") in the prediction UI.

### F4 — `spin_resolved` gives no breakdown; gate #20 ("why did I gain/lose PTS") is unverifiable (Lens #20, TASK-018)
The event carries only `{side, pts, insurance_refund, all_lost}`; state exposes no per-bet
breakdown at resolve time. Verified cases that *need* explanation: a 9-number band bet
paying 10 PTS (36 × 0.3 prediction penalty — correct per §10, but unexplainable from the
event stream); a 5-number band bet paying 18 (36 × 0.5); in-band landings paying **0**
(all_lost despite landing in the band — band ≠ bet coverage on slot-order rotation).
**Fix:** extend `spin_resolved` (additive) with per-bet results + multiplier stack, or
publish a `spin_breakdown` side record per REQ-008's side-channel pattern.

### F5 — Prediction honesty is real (DEC-001 verified) but invisible
Band stats on the deliberate run: 2/2 player landings in-band; payouts 10 and 18 PTS
confirm the ×0.3/×0.5 penalties. Accuracy is probabilistic and < 100% as designed. But
nothing in the event log or state names the penalty — players see "36× payout bet paid 10"
with no reason. Pairs with F4.

### F6 — `round_ended.hp_delta` semantics are undocumented (Lens #56)
In the elite fight the field read `−2` every round while actual HP dropped 6–7 (the intent
damage is applied at `intent_executed` time and excluded from `hp_delta`; the `−2` is the
elite's Curse of Blood). Once known it's coherent — but neither doc nor schema says so.
**Fix:** one line in schema.ts docs, or include intent damage in `hp_delta`.

### F7 — Enemy AI is static (Lens #38 flow)
4-move intent loop, bets always on its own wheel with no adaptation. This is the root
enabler of F1. Elites/boss differentiate via curses/stats only.

### F8 — Small gaps
- `eventChoiceIds` in `web/src/ui/legality.ts` hardcodes only THE HOODED SPECTRE's choices;
  other events fall back to `['decline']` (legality-audit exposure, TEST-015).
- Shop items in raw state are `{card:{def:{…}}, price}` — fine for the UI (which projects
  `{index,name,price,kind}`), but AI-tooling should consume projections, not raw state.

## What works well (verified)
- **Determinism** (REQ-006): identical event logs on replay; seed-bound.
- **Illegal commands** rejected verbatim with serde messages (SEC-002), e.g.
  `bad command json: invalid value: integer 99999, expected u16`.
- **Sacrifice** (−10 PTS → +5⚡), **undo** (full state restore incl. card stack),
  **clear/rebet** (exact layout restore) — all match §3.2/§3.4.
- **Curses on elites are legible in data**: Avarice (draws cost flat 3) + Curse of Blood
  (−2 HP/round) exactly matched observed deltas.
- **Negative-EV roulette math**: straight number 35:1 pays 36× minus house edge
  implementation is faithful; special colors (green 12×, gold/purple/cyan/crimson ladders)
  and the double-zero wheel's `00`-as-37 encoding are coherent.
- **Sudden-death termination** (house edge after capped extra rounds) — no infinite ties.
- **Defeat → retry flow** works and costs HP only through intents/curses (§3.3).
- 43 engine tests + UI (19) + client-core (18) suites green; only `ai:check`'s TEST-005
  leg fails (F2).

## Priority recommendations
1. **Fix F2** (engine) + regression test; restores the failing `ai:check` gate.
2. **Fix F1** (design): table-minimum or EV-positive house betting; re-run the 10-seed
   pass-only campaign as the acceptance metric (target: ≤ 1/10 victories).
3. **Wire F3**: expose `predict` in combat UI + prediction availability/cost display.
4. **Enrich F4**: per-bet `spin_resolved` breakdown + penalty line (TASK-018 gate #20).
5. Re-run the deliberate + pass-only campaigns after each fix; store summaries here.