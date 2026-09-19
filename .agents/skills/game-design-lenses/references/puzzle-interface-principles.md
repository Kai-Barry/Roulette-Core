# Puzzle, Interface & Feedback Principles

Distilled from *The Art of Game Design* (Schell, 3rd ed.), Chapters 12–13.

## Puzzle Principles (Chapter 12)

A puzzle makes the player **stop and think**. Puzzles are key parts of most games. **Ten puzzle principles:**

1. **Make the goal easily understood** — players bounce off puzzles they can't even picture.
2. **Make it easy to get started** — low friction to the first attempt (Lego, Sudoku's given digits).
3. **Give a sense of progress** — visible steps toward solution keep players thinking (riddles give none, and are unsatisfying alone).
4. **Give a sense of solvability** — players must believe the puzzle can be solved.
5. **Increase difficulty gradually** — order puzzles from easy to hard; the sequence is part of the design.
6. **Parallelism lets the player rest** — several independent puzzles let players switch when stuck.
7. **Pyramid structure extends interest** — small puzzles unlock one big one; understanding accumulates (Monkey Island's catalog puzzle).
8. **Hints extend interest** — graduated hints keep stuck players playing rather than quitting.
9. **Give the answer!** — in a game, a puzzle that blocks the story should eventually yield (walkthrough, hint system); a saved answer preserves forward progress.
10. **Perceptual shifts are a double-edged sword** — "aha" shifts are delightful but frustrating if arbitrary; the shift should be earnable through clues.

Also: puzzles must **test** ideas rather than stump; avoid puzzles that are merely guessing games.

## Interface Principles (Chapter 13)

Players play games **only through the interface** — every other element (mechanics, story, aesthetics) reaches the player via it.

**Eight interface tips:**

1. **Steal** — borrow interfaces that already work (conventions are assets, not embarrassments).
2. **Customize** — tailor borrowed interfaces to your game's needs; shape them to your verbs.
3. **Theme your interface** — make the interface part of the world (Metroid's visor, Fable's menus).
4. **Sound maps to touch** — the mind maps sound to physical interaction; use audio feedback for actions.
5. **Balance options and simplicity with layers** — expose few options first; reveal depth as mastery grows (physical analog: gears on a bike).
6. **Use metaphors** — familiar shapes reduce learning cost (trash can for delete).
7. **Test, test, test!** — interfaces fail in ways only players reveal.
8. **Break the rules to help your player** — when convention conflicts with clarity for *your* player, break convention.

**Physical interface** (Lens #54): choose input devices that fit the game's verbs; be sure the physicality is comfortable, appropriate, and available (does the game work on the controller it ships with? does it feel good under Lens #15 — the toy?).

**Virtual interface** (Lens #55): use it only for things that are easier that way; pop-up menus pair poorly with gamepads; keep the physical/virtual mapping consistent.

**Transparency** (Lens #56): the ideal interface is invisible. Questions: do players do what they want? Is it simple enough to use without thinking after practice? Is it intuitive for new players, customizable, and robust in edge cases (near a corner, at speed)?

**Feedback** (Lens #57): the game must continually tell the player what is happening. Feedback closes the loop between action and state change. Give feedback for both what players *need* and *want* to know, tuned to create the feeling you intend, at the moment of the player's current goal.

**Juiciness** (Lens #58): many layers of feedback for any action — visual, audio, particles, camera shake, scores popping. Juice makes simple interactions feel alive and rewarding. Any action can be juicier.

**Channels and dimensions** (Lens #59): information flows through channels (visual, audio, haptic). Use several channels to communicate in parallel; match the *dimensionality* of each channel to the data (position on screen, color for state, sound for urgency).

**Modes** (Lens #60): a mode is a reconfiguration of the interface that changes input meanings. Make different modes **look and feel clearly different** (Halo's weapon-change visuals); mode confusion is the classic failure (vi's command mode). If a player doesn't know what mode they are in, they are confused and frustrated.

## Accessibility & Onboarding Lenses

- **Lens #48 — Accessibility**: players must know how to begin; leverage what they already know; make the game draw the hand toward it (toy-quality).
- **Lens #49 — Visible Progress**: make progress visible; add interim steps of progressive success; reveal hidden progress.
- **Lens #50 — Parallelism**: let players rest by switching between independent challenges.
- **Lens #51 — Pyramid**: nest challenges so skills learned lower solve higher ones.
- **Lens #52 — Puzzle**: is this a puzzle (stop and think) or just an obstacle (act)? Both are valid; make each one what it is.

## Key Audit Questions for Interface & Puzzles

- Can a new player start playing within seconds of first contact, without a manual?
- Is every state change the player needs communicated through at least one channel immediately?
- Are modes visually distinct, and can the player always tell which mode they are in?
- Does every puzzle have a comprehensible goal, easy entry, visible progress, and a safety valve (hints/answers)?
- Is the interface themed and consistent with the world, or does it break the fantasy?
- Would playing the game "toy-like" (no goal) still feel satisfying?