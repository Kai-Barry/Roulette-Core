# Balance Principles

Distilled from *The Art of Game Design* (Schell, 3rd ed.), Chapter 11. Master check (Lens #47): **Does my game feel right? Why or why not?**

## The Twelve Most Common Types of Game Balance

### 1. Fairness
- **Symmetrical games** give equal resources/powers to all players — best when determining who is best. Handle small asymmetries ("who goes first?") by random selection or by giving the advantage to the weaker player (e.g., "youngest goes first").
- **Asymmetrical games** give different resources/abilities for four reasons: (a) to simulate a real-world situation, (b) to let players explore the gamespace from new angles, (c) personalization — let players play to their strengths, (d) to level the playing field (handicapping). Asymmetry demands significant balancing effort; use **rock-paper-scissors** relationships so no single strategy dominates.

### 2. Challenge vs. Success
Keep players in the **flow channel** — the narrow margin between boredom (too easy) and anxiety (too hard). Techniques:
- Increase difficulty gradually with player skill.
- "Give them a break" — alternate challenge intensity with rest periods.
- Let players choose difficulty levels.
- Playtest and adjust the learning curve: challenge should rise slightly faster than skill, but never overwhelm.

### 3. Meaningful Choices
Choices must have real impact on what happens next. Traps to avoid:
- Choices that don't matter (50 vehicles that all drive the same).
- Choices no one would want.
- **Dominant strategies** — one choice clearly better than the rest. Once discovered, choice is gone; rebalance until the dominant strategy no longer dominates (a hidden one players find is an "exploit").

The number of choices should match the number of the player's desires:
- Choices > desires → overwhelmed
- Choices < desires → frustrated
- Choices = desires → freedom and fulfillment

**Triangularity** (Lens #33) is the most exciting meaningful choice: play it safe for a small reward vs. take a big risk for a big reward (the Space Invaders saucer). If a prototype "just isn't fun," it is usually missing triangularity. Balance triangularity using expected value (the Qix fast/slow rectangle example: keep expected value constant across risk levels).

### 4. Skill vs. Chance
Too much chance negates skill; too much skill makes games tense and predictable. Choose per audience: skill games are judgment systems (who is best?); chance games are relaxed and casual. Consider head-to-head preferences of your target players (differences by age, gender, culture).

### 5. Head vs. Hands
How much thinking vs. dexterity? Mix them for variety (action-platformer puzzles + boss fights that need both puzzle-solving and dexterity). Whatever mix is chosen, communicate it clearly — mismatched expectations (Pac Man 2) confuse players.

### 6. Competition vs. Cooperation
Both are basic urges. Competition lets players test skill and status in socially safe ways; cooperation lets players partake in actions impossible alone. Interesting designs blend them (team sports, Joust's solo/duo modes).

### 7. Short vs. Long
Games too short prevent meaningful strategies; too long invites boredom. Win/lose conditions determine length — tune them. Watch for player-modified house rules (Monopoly's Free Parking jackpot) that lengthen games beyond design intent.

### 8. Rewards
Rewards fulfill player desires. Types: **praise, points, prolonged play, a gateway, spectacle, expression, powers, resources, completion**. Principles:
- Rewards must be understood — a reward the player doesn't understand is no reward.
- Variable reward schedules keep rewards exciting.
- Reward build-up matters: too fast feels cheap, too slow feels punishing.

### 9. Punishment
Paradoxically increases enjoyment when used properly because it: creates endogenous value (losing things makes things matter), makes risk exciting, increases challenge. Types (in ascending severity): **shaming, loss of points, shortened play, terminated play, setback, removal of powers, resource depletion**. Punish weakly at first; save strong punishments for deliberate violations, and balance them with commensurately strong rewards.

### 10. Freedom vs. Controlled Experience
Total freedom is boring and expensive; total control is a movie. Deliberately choose where the player is free and where the game takes over (see the Aladdin VR example: taking away camera freedom in one scene because every player wanted the same thing — and nobody noticed).

### 11. Simple vs. Complex
- **Innate complexity**: complex rules ("unless," "except," "but") — often a bad sign, but sometimes needed for simulation or balance.
- **Emergent complexity**: simple rules producing complex situations (Go) — the praised kind.
- Aim for **balanced surprises** from a simple ruleset. Avoid excessive first-order (obvious) or second-order (corner-case) optimal strategies.

### 12. Detail vs. Imagination
Only detail what you can do well; let imagination fill lower-quality gaps (subtitles beat bad synthesized speech). Details that reuse and inspire imagination beat exhaustive low-quality simulation.

## Game Balancing Methodologies

- **Use the Lens of the Problem Statement** — state the balance problem clearly before trying solutions.
- **Doubling and halving** (Brian Reynolds/Sid Meier) — change values by 2×, not 10%, so the effect is clearly felt; binary-search from there.
- **Train your intuition by guessing exactly** — guess precise values (13.8, not "around 15"), then test; your intuition calibrates over time.
- **Document your model** — write the relationships you think exist between the values you are balancing.
- **Tune your model as you tune the game** — when experiments contradict the model, fix the model.
- **Plan to balance** — build in easy (ideally live) tuning of values you expect to adjust.
- **Don't let the players balance it** — players have a conflict of interest; they'll choose power and kill challenge. Difficulty levels are the sanctioned exception.

## Balancing Game Economies

A **game economy** is defined by two meaningful decisions: *how do I earn?* and *how do I spend?* ("Money" = anything tradeable — skill points count.) Balance economies against all the balance types at once:

- Fairness: can any player gain unfair advantage through earning or buying?
- Challenge: can purchases trivialize the game? Is earning too hard?
- Choices: enough ways to earn and to spend?
- Chance: skill-based or luck-based earning?
- Cooperation: can players pool funds? Can collusion exploit holes?
- Time: is earning paced reasonably?
- Rewards/punishment: do they interact correctly with the economy?
- Freedom: can players earn and buy the way they want?

Control how much money the game creates and the earn/spend paths. Watch for runaway feedback loops in both directions (rich get richer; death spirals).

## Dynamic Game Balancing

Adjusting difficulty to the player in real time (e.g., rubber-banding). Risks: players feel cheated when the game is caught adjusting; invisible rubber-banding must stay invisible. If used, keep it subtle and playtest heavily — it can easily do more harm than good.

## Key Audit Questions for Balance

- Is every one of the twelve balance types deliberately decided rather than accidental?
- Are there any dominant strategies or exploits left in the current build?
- Does every risk/reward pair keep expected value fair (triangularity balanced)?
- Do the economy's earn/spend loops close without runaway inflation or starvation?
- Does difficulty track player skill growth (flow channel), not just a fixed ramp?
- Would doubling or halving the suspect value make the problem obvious?