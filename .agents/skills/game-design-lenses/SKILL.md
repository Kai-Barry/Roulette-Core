---
name: game-design-lenses
description: This skill should be used when the user asks to "audit my game design", "check gameplay design", "review game mechanics", "does my game follow its design", "design a game level or mechanic", "balance a game", "make my game fun", "apply the lenses from The Art of Game Design", or mentions gameplay design, game feel, the elemental tetrad, game balance, or the Schell lenses. It distills the design principles of Jesse Schell's *The Art of Game Design: A Book of Lenses* into workflows for verifying that a game (design doc or implementation) actually delivers its intended gameplay.
---

# Game Design Lenses

Distilled from Jesse Schell's *The Art of Game Design: A Book of Lenses* (3rd edition).
The book's core method: examine a game through many "lenses" — each a set of questions that
expose one aspect of the design. A game follows its gameplay design when it delivers the
**essential experience** through all four tetrad elements (mechanics, story, aesthetics,
technology) while staying balanced, transparent, and interesting over time.

## Core Concepts (use before anything else)

1. **The game is not the experience.** The game *engenders* an experience in the player's mind.
   State the intended essential experience in one sentence; judge every decision against it.
2. **The Elemental Tetrad** — every game is made of mechanics, story, aesthetics, and technology.
   All four must reinforce one theme; the weakest element caps the whole design.
3. **The Rule of the Loop** — the more times a design is tested and improved, the better it gets.
   Verification of gameplay is never a single pass: state problems → prototype → playtest → repeat.
4. **Does my game feel right?** (Lens #47) — the master question. All analysis exists to answer it.

## Workflow: Auditing a Gameplay Design

When asked whether a game follows/fulfills its gameplay design (from a design doc,
prototype, or codebase), run this sequence. Read `references/tetrad-framework.md` first,
then the topical reference for each suspect area.

### Step 1 — Anchor the intended design
- Extract or write the **essential experience** (Lens #1) and the **problem statement** (Lens #12):
  what experience is this game trying to deliver, for whom, and what problem does it solve?
- State the theme in one sentence (Lens #9) and the four tetrad elements as they exist today.
- If the design doc and implementation disagree, record both versions — the gap itself is a finding.

### Step 2 — Decompose mechanics
Using `references/mechanics-principles.md` and `references/01-gameplay-mechanics.md`:
- Enumerate space (discrete/continuous/zero-dim, nested?), objects, attributes and states
  (and **who knows each state** — game/all/one/some), operative vs. resultant actions,
  rule levels (operational/foundational/behavioral), goals (concrete, achievable, rewarding),
  the real-skill list, and where chance enters (check the probability math).
- Trace one full player action through the tetrad: intent → mechanic → interface → aesthetics →
  experience. Gaps anywhere are defects.

### Step 3 — Check the twelve balance types
Using `references/balance-principles.md` and `references/02-balance-chance.md`:
- For each of the twelve types (fairness, challenge vs. success, meaningful choices, skill vs.
  chance, head vs. hands, competition vs. cooperation, short vs. long, rewards, punishment,
  freedom vs. control, simple vs. complex, detail vs. imagination): is the choice deliberate?
- Hunt for **dominant strategies and exploits** — a choice clearly better than the rest.
  Flag triangularity gaps: is there a safe-vs-risky choice, and does expected value stay fair?
- If an economy exists (earn + spend loops), audit its feedback loops for runaway or starvation.

### Step 4 — Verify the interface delivers the design
Using `references/puzzle-interface-principles.md` and `references/03-interface.md`:
- Transparency (#56): can players act on intent without thinking about the controls?
- Feedback (#57) and juiciness (#58): does every state change reach the player instantly
  through at least one channel? Are modes (#60) visually distinct?
- Onboarding: accessibility (#48), visible progress (#49), puzzle principles
  (easy start, sense of progress, solvability, graduated hints, an eventual answer).

### Step 5 — Judge the interest curve and flow
Using `references/interest-flow-principles.md` and `references/04-interest-story-world.md`:
- Sketch the interest curve per player archetype: hook → rising interest with rest → grand finale.
  Find and fix flat/declining segments; check for fractal structure across time scales.
- Flow: clear goals, no distractions, direct feedback, continuously matching challenge.
- Judgment (#20): fair, cared-about judgment that invites improvement.

### Step 6 — Story, world, and player guidance (if applicable)
Using `references/story-world-principles.md`:
- Does gameplay *cause* the story (or vice versa), or do they merely alternate?
- Prefer **indirect control** (constraints, goals, interface, visual design, characters, music)
  over forced paths; players must retain the feeling of freedom.
- Characters must earn their place (function), show traits through action, and relate
  meaningfully on the circumplex (friendly↔hostile, dominant↔submissive).

### Step 7 — Process checks
Using `references/process-principles.md`:
- Which of the eight filters (artistic, demographics, experience design, innovation,
  business, engineering, social, playtesting) has this design *not* been through?
- What are the top risks, and is there a prototype/scheduled test for each?
- Technology: foundational or decorational for this experience — and is that intentional?

### Step 8 — Report
Produce an audit report with:
- The stated essential experience and theme, and whether the delivered design matches.
- Per-area findings (mechanics, balance, interface, interest/flow, story/world, process),
  each citing lens numbers (e.g., "Lens #32 — choices exist but are not meaningful: the
  'heavy' armor dominates all other options").
- A prioritized fix list: gameplay-blocking issues first (dominant strategies, transparency
  failures, flow killers), then polish (juiciness, resonance, unification).
- For a generated checklist artifact, run
  `scripts/generate_audit.py --groups core balance --out audit.md` (see `--list` for groups,
  `--lenses` for explicit numbers, `--all` for the full 100).

## Reference Map

| File | Contents |
|---|---|
| `references/tetrad-framework.md` | Elemental tetrad, theme, the design-as-problem mindset; read first |
| `references/all-100-lenses.md` | Complete master list: all 100 lenses with every question |
| `references/00-core-lenses.md` | The foundational lenses (#1–#20 + Balance) |
| `references/01-gameplay-mechanics.md` | Mechanics lenses (#21–#27, #53, #71) |
| `references/02-balance-chance.md` | Balance lenses (#28–#46) |
| `references/03-interface.md` | Interface/puzzle lenses (#48–#60) |
| `references/04-interest-story-world.md` | Interest/story/world lenses (#61–#83) |
| `references/05-players-social.md` | Social lenses (#84–#88) |
| `references/06-process-team.md` | Process lenses (#13–#17, #89–#100) |
| `references/mechanics-principles.md` | Space, objects/states, actions, rules, skill, chance, endogenous value |
| `references/balance-principles.md` | The 12 balance types, balancing methodologies, economies, dynamic balancing |
| `references/puzzle-interface-principles.md` | 10 puzzle principles, 8 interface tips, transparency/feedback/juiciness/modes |
| `references/interest-flow-principles.md` | Interest curves, inherent interest, flow channel, motivation |
| `references/story-world-principles.md` | Story vs. gameplay, indirect control, worlds, characters |
| `references/process-principles.md` | Iteration loop, eight filters, risk mitigation, playtesting, tech, team |

Progressive disclosure: SKILL.md holds the workflow; references hold the details.
Load only the reference needed for the area under examination. Grep lenses quickly, e.g.
`grep -n "Lens #33" references/all-100-lenses.md`.

## Scripts

- **`scripts/generate_audit.py`** — generates a Markdown audit checklist with checkboxes
  from any selection of lenses (default: the 59-lens core gameplay audit). Lens data lives in
  `scripts/data/lenses.json`.

## When Reviewing Actual Code or a Prototype

- Playable build present: run it; watch for flow exits (confusion, boredom, frustration),
  transparency failures, missing feedback, and unfun core verbs (Lens #15: is it fun with
  no goal at all?). Then playtest questions from `references/03-interface.md` and
  `references/process-principles.md`.
- Design doc only: walk the doc through Steps 1–6 above, flagging claims that lack a
  supporting mechanic (e.g., "we want players to feel clever" but no decision points).
- Code review of gameplay systems: enumerate the implemented state model and actions
  (Step 2), check tunable-value architecture (balance-ready), feedback coverage, and
  probability math against the ten probability rules in `references/mechanics-principles.md`.

## Common Failure Patterns to Flag

- Story told in cutscenes that gameplay ignores → merge story and gameplay conflict.
- "50 cars, all the same" → meaningless choices; and any dominant strategy.
- Missing triangularity → a "just isn't fun" prototype usually lacks the safe-vs-risk choice.
- Reward players don't understand → a reward you don't understand is no reward.
- Mode confusion → players lost in an unseen interface mode (the vi problem).
- Player-set difficulty (letting players balance) → power rush, then boredom.
- Interest curve with no hook, no rests, or a fizzle before the finale.
- Detailing what can't be done well → let imagination fill low-quality gaps instead.