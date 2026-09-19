# Roulette.OS — Engine Testing Harness: Gameplay Lens Audit (post-play)

Scope: playable harness (`index.html` = `web/index.html`, served at :8777) played
end-to-end in all three modes, cross-checked against `docs/ROULETTE_OS_GAME_BLUEPRINT.md`
and the Rust engine (`roulette-core`). Method: Schell lens workflow Steps 1–8; prior
doc-level audit in `.agents_tmp/design_audit.md` re-verified against current code.

---

## 1. Essential experience & theme (Lens #1, #9)

**Intended (§1.1):** *"A 3D roulette table in a cursed tavern is your weapon: you
manipulate the physics of the wheel, the rules of the board, and the odds themselves
using cheat cards to out-gamble sinister croupiers."* Core verb: **bending odds**.

**Delivered by the harness:** a simplified odds-bending sandbox — cards mutate the
wheel (extra greens, gold jackpot, sector recolors), payouts compound (Double Down),
and the wheel is uniformly sampled (no physics sim). It teaches the *economy and card
verbs* of the design, not the physics-is-a-weapon pillar.

**Verdict: partially on-theme.** The cheat-fantasy verb ("corrupt the board, then
bet") is present and fun, but three intended-experience elements are missing or
betrayed: no prediction band (Pillar 5), no physics (Pillar 1), and card play is free
(economy's risk axis absent, see F2).

### Prior doc-level findings (F1–F8) — re-verified in code
| Prior | Status | Evidence |
|---|---|---|
| F1 prediction certainty | ✅ Fixed | `Simulator::predict` honesty roll on independent `prediction` stream; `prediction_accuracy` default 0.90; decoy runs from separate stream (`phys/simulator.rs:383–410`) |
| F2 special-color bet dominance | ⚠️ Partial | Engine per-wheel tables kept; `dealers_claw` number=16× vs crimson 6× still dominated; harness has no special-color bets at all |
| F3 pool semantics | ✅ Fixed | `RunState::complete_battle` folds `chips_pool` + venture bank; victory = pool > enemy pool (`run/state.rs:433–470`) |
| F4 enemy HP vestigial in points mode | ⚠️ Open | `enemy_hp` only decremented in legacy `CombatMode::Damage` (`battle/resolve.rs:491`); still dead state in points mode |
| F5 dealers_claw difficulty | ✅ Fixed | `content/enemies.ron:50` = 0.85 |
| F6 NaN validator | ✅ Fixed | `validate.rs:96` `is_finite()` |
| F7 compound_interest port | ⚠️ Open | 0 matches in `content/cards.ron` |
| F8 EV-dominance gate | ✅ Largely | `roulette-sim ev` rates all 107 cards vs bands; no gate *assert* but reporting exists |

---

## 2. Gameplay-blocking findings (harness)

### 🔴 H1 — Card costs are never charged (Lens #5 Endogenous Value, #29 Economy)
Every card displays a cost (0/1/2) but `executeSpin()` never reads `card.cost` — no
⚡/chips are spent. The blueprint's core economy (cards cost Essence, §2.5/§6) is
suspended. Consequences: `chip_surge` (+15 free) and `greed_pact` (+30 free) are pure
profit; combined +45 chips per turn makes bankruptcy impossible and the betting risk
axis decorative. The **only** real costs left in the harness are HP (enemy attacks)
and the stake itself.

### 🔴 H2 — Greed Pact text lies (Lens #56 Transparency, #20 Judgment)
Card says "Sacrifice 1 Hand to gain 30 Chips." Implementation: `chips += 30` with no
hand loss, no hand sacrifice, nothing. A card whose description misrepresents its
effect is a trust-breaking defect — players cannot build strategy on stated rules.

### 🔴 H3 — Second Chance is a dead card (Lens #21 Operative Actions, #56)
`rerollOnLoss` is set when played, then never read anywhere — a losing spin is never
resprung. Playing it (cost 2!) does literally nothing. Costly no-ops are the worst
transparency failure: the player pays for a rule the game doesn't enforce.

### 🔴 H4 — Jackpot Slot flips the wheel player-favorable and makes every bet identical (Lens #31 Dominant Strategies, #33 Triangularity, #28 Challenge/Success)
With the gold 77 slot on the wheel, *any* winning bet check short-circuits
(`if (targetSlot.color === 'Gold') win = true`) and the payout is forced to 50×
regardless of bet type. Math: 1 gold slot in 38 → EV = 50/38 − 1 = **+31.6%** per
unit staked, for Red, Black, Even, High — everything. One played card converts the
entire bet menu into one +EV button. All-in × 50 → 2,500 PTS in a single hand vs a
200 PTS quota (Point Round) or 100 HP (combat modes): instant win, interest curve
collapsed (Lens #71). It also *removes* choice: bet type no longer matters while the
jackpot is live. If the +EV cheat turn is intended (Balatro-style), it must be
bounded: e.g. gold pays 50× only for a dedicated GOLD bet; other bets pay their
normal multiplier on gold.

### 🔴 H5 — Redraw Hand is free and unlimited (Lens #31, #29)
"🔄 Redraw Hand" re-rolls the 4-card hand at zero cost, unlimited times per turn.
Optimal play is degenerate: redraw until the hand holds the best combo
(jackpot + double_down + surge + greed…). Hand management stops being a decision.
Give redraw a cost (chips/HP/hand-sacrifice) or once-per-turn limit.

### 🟠 H6 — 1v1 Survival and Elemental D6 Horde are the same game (Lens #21 Space, #58 Modes distinct, #12 Problem statement)
Both combat modes: identical HUD logic, identical flat 15 HP enemy attack, identical
win/lose conditions (drain 100 target HP). The Horde mode has **no D6, no horde, no
elementals** — its name promises mechanics that do not exist. Players clicking the
third button feel lied to (same failure class as H2). Either implement a minimal D6
twist (e.g. attack = 2d6 + tier, horde grants 3 waves) or relabel the mode.

### 🟠 H7 — Stale HUD after mode switch with finished encounter (Lens #57 Feedback, #60 Modes)
Repro: finish an encounter (VICTORY modal up) → switch modes → HUD shows the *old*
mode's numbers (e.g. "Player HP 100/100" + Target 0/100 while Turn 1) and the modal
stays up showing the previous mode's outcome. `selectMode` resets state but
`updateHUD()` is only called from the spin path; `resetEncounter()` → `init()` draws
the hand but does not refresh HUD labels/values for the new mode. One-line fix:
call `updateHUD()` in `resetEncounter()`.

### 🟠 H8 — Outcome modal can be invisible after PLAY AGAIN → next instant win (Lens #57, #60)
Repro: victory → PLAY AGAIN → immediately play Chip Surge + all-in win → outcome
modal *sometimes* not visible (element list shows no modal; log shows VICTORY).
Related to the CSS `visibility` transition fix: on a *fresh* page the modal shows,
but on a same-session re-show right after a hide, the transition can be skipped.
Verify `.modal` transition uses `visibility` with a delay and that `.active` toggles
both opacity and visibility synchronously; simplest hardening: after removing
`.active`, force `display:none` via a 250ms `setTimeout` so the next show re-triggers
the transition.

*Note:* the stale "DEFEAT! Bankrupt with 100/200 PTS" modal that survived a mode
switch (observed) is the same family as H7 — modal state must be cleared by
`resetEncounter()` (it does clear it; the observed case was switching *before*
PLAY AGAIN — H7's fix covers it).

### 🟡 H9 — Hand refill can duplicate cards (Lens #21)
`executeSpin` refill draws with replacement into an existing hand → duplicate cards
in hand; `drawNewHand` (used by Redraw) draws without replacement. Minor, but
inconsistent hand rules confuse card-counting players.

---

## 3. Balance areas (Lens #28–#46)

- **Skill vs chance (#35):** base bets are honest European odds (−2.7% EV for
  color/parity/exact) — the *wheel* is fairly hostile, and cards are the skill axis.
  Good shape, exactly the "beat fair odds by cheating" theme. The skill axis is
  undermined by H5 (free redraw) and H4 (jackpot erases bet choice).
- **Green math (#33 triangularity):** Green 14× on 1/37 = −62% EV; with 1 extra
  green (Green Corruption) −24%; with 2 extra +13.5%. So green betting only becomes
  viable after *two* corruptions — a real risk-ladder, arguably the best-designed
  number in the harness. Consider pricing Green Corruption so stacking two is a
  genuine investment (it's currently free — H1).
- **Rewards/punishment (#39/#40):** combat modes: −15 HP per spin, flat, no
  telegraph, no variance. Punishment is predictable background drain, not threat;
  blueprint enemy intents (§10.5) would need choices to dodge. Acceptable for a
  harness; note before reusing this UI in the real frontend.
- **Economy feedback (#29):** with H1 fixed, cost-1/2 cards become real; until then
  the chip economy only loses via stakes. Point Round quota 200 in 4 hands with 50
  chips is a fair challenge *only* without jackpot/redraw abuse (2 all-in red wins ≈
  200 PTS).

---

## 4. Interface & interest (Lens #48–#60, #61–#70)

- **Feedback (#57):** every state change lands somewhere — log (color-coded),
  HUD, result panel, wheel animation. Good. Missing: chip/score *delta* emphasis
  (payout log line does the job, acceptable).
- **Transparency (#56):** bet pills state multipliers honestly (2.0×/14×/36×);
  gold jackpot's "any bet pays 50×" is *not* stated anywhere on the pills —
  discoverable surprise, but after H4's fix, pills should show gold-specific odds.
- **Modes (#60):** visually labeled, HUD relabels — but functionally two of three
  modes are the same (H6).
- **Onboarding (#48):** one-line init hint; enough for a testing harness.
- **Interest curve (#71):** Point Round has a proper micro-curve (4 hands, rising
  stake, finale = quota). Combat modes flatten to "win every all-in before 7 spins."
  H4's jackpot strategy makes the curve collapse to a single spike.

---

## 5. Prioritized fix list

| # | Fix | Cost | Lens |
|---|---|---|---|
| 1 | H1: charge `card.cost` chips in `executeSpin` (skip 0-cost; block play when chips < cost) | small | #5, #29 |
| 2 | H3: implement Second Chance — on loss, if `rerollOnLoss`, respin once (consume flag, log "Second Chance respin!") | small | #21, #56 |
| 3 | H2: make Greed Pact honest — clear hand (sacrifice) then +30 chips, or fix text | small | #56, #20 |
| 4 | H4: gold slot pays 50× only on a GOLD bet; other bets resolve normally on gold lands (or drop forced-win line) | small | #31, #33 |
| 5 | H5: Redraw costs 5 chips (or 1 sacrifice), once per turn | small | #31, #29 |
| 6 | H7: call `updateHUD()` inside `resetEncounter()`; clear modal on mode switch | trivial | #57, #60 |
| 7 | H8: harden modal re-show (transition restart) | trivial | #57 |
| 8 | H6: differentiate Horde (e.g. 2d6 damage + 3 escalating waves) or rename mode | medium | #21, #58 |
| 9 | H9: consistent no-duplicate hand refill | trivial | #21 |
| 10 | Engine follow-ups from prior audit: wire score→enemy-HP or doc `max_hp` as damage-mode-only (F4); port `compound_interest` (F7) | small | #21, #39 |

## 6. Verdict

The **Rust engine** is design-faithful: prior blockers F1/F3/F5/F6 are fixed,
economy semantics are pinned, EV tooling enforces the balance bands. The **harness**
successfully tests the card/economy verbs and the theme's odds-bending fantasy, but
currently teaches four wrong rules (free cards, lying Greed Pact, dead Second
Chance, any-bet jackpot) and one degenerate strategy (free redraw fishing). Fix the
top five table rows and the harness becomes a truthful miniature of the blueprint's
core loop.