# Game Mechanics Principles

Distilled from *The Art of Game Design* (Schell, 3rd ed.), Chapter 10. Mechanics are the mechanisms of gameplay: the parts of the tetrad that make the game happen.

## Mechanic 1: Space

The space of a game is where gameplay happens. Spaces are either:

- **Discrete** — broken into units (tiles, rooms, cards); movement jumps from unit to unit (chess, tic-tac-toe).
- **Continuous** — movement is free and measured by distance (racing games, shooters).
- **Zero-dimensional** — no meaningful spatial relationship (word games, phone calls).

Spaces can be **nested** (a ship inside a room inside a level) and may be 2D or 3D, linear or branching. Ask: does the functional space of the game support the goals? Is it too cramped or too empty for the actions the player must perform?

## Mechanic 2: Objects, Attributes, and States

Objects are the nouns. Attributes are the adjectives — the aspects of an object that can change or vary. Each attribute has a set of possible states.

The critical design decision is **who knows what state**:

1. State known only by the game itself (hidden — e.g., face-down cards).
2. State known by all players (open — e.g., board position).
3. State known by only one player (secret — e.g., a player's hand).
4. State known by some but not all players (partially hidden — e.g., fog of war).

Changing who knows which states is one of the most powerful design levers — it directly changes the gameplay (information asymmetry creates deduction, bluffing, and surprise).

## Mechanic 3: Actions

Actions are the verbs. Two levels:

- **Operative actions** — what the player can literally do (move, shoot, jump, trade).
- **Resultant actions** — what the player effectively does at a larger scale (strategize, explore, negotiate).

A rich design gives players many operative actions that compose into meaningful resultant actions. Forcing multiple aspects of the game state to change with each operational action creates emergence and interest. "Verbs" define the game — the most fundamental design decision is what actions the player will perform.

## Mechanic 4: Rules

Three levels of rules (Parlett's model):

1. **Operational rules** — what players actually do to play.
2. **Foundational rules** — the underlying formal/mathematical structure of game state and how it changes.
3. **Behavioral rules** — implicit "good sportsmanship" rules that are never written but universally understood; the game is a social contract.

Also: **written rules** (manuals/tutorials), **laws** (rules governing official competition), and **house rules** (variants players invent — a sign players care about the game). Good game **goals** are **concrete** (clearly stated), **achievable** (believed possible), and **rewarding** (valued before and after achievement), with a healthy mix of short- and long-term goals.

## Mechanic 5: Skill

Distinguish **real skills** (things the player must actually be able to do) from **virtual skills** (in-game character abilities that level up regardless of player ability). Enumerate every skill the game demands — the list defines the audience and the experience. Skill categories include:

- Physical (strength, dexterity, coordination, endurance)
- Mental (memory, pattern matching, spatial reasoning, quick calculation)
- Social (reading opponents, negotiation, bluffing, teamwork)

Skills should be exercisable, improvable with practice, and demand the right level for the target audience.

## Mechanic 6: Chance

Chance means uncertainty, and uncertainty means interest. **Ten rules of probability every designer should know:**

1. Fractions are decimals are percents (½ = 0.5 = 50%).
2. Probabilities range from 0 to 1 — nothing beyond.
3. Probability = "looked for" outcomes ÷ "possible" outcomes (for equiprobable outcomes).
4. Enumerate! Listing all outcomes can solve any probability problem.
5. "OR" means add — but only for mutually exclusive events.
6. "AND" means multiply — but only for independent events.
7. One minus "doesn't" = "does" (P(at least one) = 1 − P(none)).
8. A linear random selection summed over multiple trials tends toward a bell curve (normal distribution).
9. Roll the dice — collect data and observe actual results (Monte Carlo method).
10. Geeks love showing off (Gombauld's Law) — someone will delight in analyzing your odds.

**Expected value** = chance of success × value of outcome. Players' *perceived* probability matters as much as actual probability — players overweigh rare dramatic outcomes and underweigh likely mundane ones. Perceived probability varies per player; perceived value is intangible (pride, fun, prestige). When balancing, use actual values in models, but playtest to correct for perception.

## Endogenous Value

Value in a game is **endogenous** when the things inside the game matter because of the game's own systems (Monopoly money is worthless outside the game, precious inside it). Give players an empty "funny money" account at the start of the game and fill it — players will value what the game values. Curiosity + endogenous value = motivation: the gap between what players know and want to know drives them, but only if what they're seeking has in-game worth.

## Key Audit Questions for Mechanics

- Are the game's spaces, objects, attributes, actions, and rules all clearly enumerable?
- Does every attribute's state ownership (game/all/one/some) serve the intended experience?
- Do operative actions compose into interesting resultant actions?
- Are the goals concrete, achievable, rewarding, and well-mixed across time horizons?
- Is the skill list what the design intends players to exercise?
- Where does chance enter, and does the probability math survive Rule #2 (no probability > 100%)?