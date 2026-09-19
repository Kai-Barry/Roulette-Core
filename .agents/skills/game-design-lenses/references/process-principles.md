# Process, Playtesting & Team Principles

Distilled from *The Art of Game Design* (Schell, 3rd ed.), Chapters 4, 7, 25–26 and related.

## The Iterative Loop (Chapter 7)

The informal loop: think of an idea → try it → keep changing/testing until good enough. The formal loop:

1. State the problem.
2. Brainstorm possible solutions.
3. Choose a solution.
4. List the risks of using that solution.
5. Build prototypes to mitigate the risks.
6. Test the prototypes. If good enough, stop.
7. State the new problems, and go to step 2.

**The Rule of the Loop**: the more times you test and improve your design, the better it will be. Risk mitigation means testing *the riskiest assumptions first* — a prototype that can fail fast is more valuable than a beautiful but safe one.

## The Eight Filters (Lens #13)

A finished design must survive eight tests:

1. **Artistic impulse** — does it feel right to you?
2. **Demographics** — will the intended audience like it enough?
3. **Experience design** — is it well-designed? (Does it stand up to the lenses?)
4. **Innovation** — is it novel enough?
5. **Business and marketing** — will it sell?
6. **Engineering** — is it technically buildable?
7. **Social/community** — does it meet social and community goals?
8. **Playtesting** — do playtesters enjoy it enough?

Every design decision is judged through all eight filters; a change that passes one filter may fail another. Expect to revise constantly — and to be disoriented when dominant strategies disappear ("I no longer know the right way to play my game") — that means the game has taken a big step forward.

## Risk Mitigation (Lens #14)

Stop thinking positively; consider what could go wrong, then act early to prevent it:

- What could keep this game from being great?
- How can we stop that from happening?
- Ask technical questions early ("how many animated characters can our tech support?"), core-play questions ("is the core gameplay fun? does it stay fun?"), aesthetic-fit questions ("do characters and settings fit?"), and scope questions ("how large does a level need to be?").

Build prototypes to answer the highest-risk questions first. Hierarchy of prototypes (cheapest → most expensive): **paper prototypes, physical prototypes, software prototypes** (static art, playable tech, full vertical slice).

## Playtesting (Chapter 25)

Playtesting is the most important filter. Key principles:

- **Playtest early and often** — paper prototypes before code.
- **Playtest with strangers** — friends and family are too polite and too knowledgeable.
- **Silent watching**: observe quietly; don't explain — you want to see what players do unaided.
- **Ask players to think aloud** when appropriate; note where they are confused, bored, or frustrated.
- **Separate "do they understand?" from "do they like it?"** — different fixes.
- Measure: understanding, fairness, boredom, confusion, frustration, dominant strategies, loopholes, bugs, player-invented strategies, and the micro-decisions (which button for jump? is level three too long?).
- **Playtesting is not fun-testing only**: it validates the experience against the design's intent (Lens #1).

## Technology (Chapter 26)

Technology is the tetrad element that *makes the game happen*. Distinguish:

- **Foundational technology** — the tech the game experience is built on (physics engine in a physics game).
- **Decorational technology** — tech used but not essential to the experience (high-def textures in a board-game port).

Ask: is the technology as cool as I think? Is there a "disruptive technology" I should consider instead? (Lens #93 — Crystal Ball: predict where technology is going; write down concrete predictions and their reasons.)

## Team & Documents (Chapters 23–24)

- A design is a team sport; the team is an engine of skills and personalities (Lens #89).
- Documents exist to **remember** and to **communicate** — write only what serves those purposes (Lens #90). Common doc set: game design doc, tech design doc, art bible, project plan; but skip any doc that serves no reader.
- Design documents decay fast; keep the *living* parts current (tuning tables, rules of play) and archive the rest.

## Client & Pitch (Chapters 27–28)

- Know what the client *needs*, not what they *say* they want (Lens #94) — restate the problem, propose the solution that solves the *real* problem.
- A pitch is a story about the future experience; it must be clear, engaging, and honest (Lens #95).

## Key Audit Questions for Process

- Is there a current, explicit problem statement for the design question at hand?
- What are the top risks, and is there a prototype scheduled to answer each?
- Which of the eight filters has this design decision *not* been through?
- Are playtests frequent enough that the loop is actually closing (state → build → test → state)?
- Is the technology choice foundational or decorational for this experience — and is that intentional?