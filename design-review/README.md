# Design-Review Screenshot Set — Roulette of the Damned (Phase 4 3D frontend)

Captured 2026-09-14 from the live Vite build (`http://127.0.0.1:5199/?seed=design-gl`),
seed `design-gl`, difficulty `medium`. **GL shots read the raw WebGL back buffer**
(readPixels inside the render rAF), so they show the three.js scene exactly as the
PS1 pipeline composites it — the compositor does not screenshot WebGL canvases
reliably in this Chromium build. `ui-01-menu-fullpage.png` is a full-page
compositor shot showing DOM UI + dark canvas as a user sees it.

## What is visible in the GL shots
- Clear 3D rendering: wheel disc, ball, felt, betting sectors, chip stacks,
  croupier silhouettes, dust, fog, enemy table (red-tinted), PS1 dithering +
  5-bit quantization (banding visible by design).
- Colors sampled from raw pixels: gold/amber (66,41,8) wheel sectors, felt green
  (8,90,41), red sector (181,8,16), PS1-quantized grays.
- Wheel visual (TASK-024) is attached in-combat: 37 slots (WheelConfig-driven).

## Files

| File | Section | Notes |
|---|---|---|
| ui-01-menu-fullpage.png | Menu | full-page compositor shot (DOM UI over canvas) |
| 01-menu.png | Menu | GL buffer only — menu scene dressing |
| 02-loadout.png | Loadout | draft cards offered (seed-dependent) |
| 03-loadout-drafted.png | Loadout | after 2 drafts |
| 04-map.png | Map | board_wide rig; node graph |
| 05-combat-prebet.png | Combat (betting) | player wheel + felt + enemy table |
| 06-combat-bets.png | Combat (bets placed) | red 5 / black 3 |
| 07-spin-early.png | Spin telemetry | ball in flight, early frames |
| 08-spin-mid.png | Spin telemetry | mid-flight |
| 09-round-resolved.png | Round resolved | post-landed frame |
| 10-combat-late.png | Combat, later | after policy rounds (back on map here) |
| 11-rig-*.png (9) | §11.3 nine-camera rig | table_front, wheel_close, wheel_top, wheel_orbit, board_wide, enemy_side, dramatic_low, table_left, table_right |
| 12-node-entry.png | Second node | entering next combat |
| 13-second-combat.png | Second combat | green-bet spin resolved |

## How these were captured (reproducible)
```bash
node /tmp/shots/gltour.mjs   # puppeteer-core + /usr/bin/chromium, ?seed=design-gl
```
- Fresh run: `start_run(medium)` → draft legal candidates → `finish_loadout` →
  `pick_node` → `place_bet`/`spin` via `window.__RT.dispatch` (REQ-005: engine
  commands only, no synthetic UI).
- Rig angles via `window.__RT.render.active = <rig>` (§11.3).
- Pixels: `gl.readPixels` in a double-rAF (canvas has no preserveDrawingBuffer).

## Known capture caveats (for reviewers)
1. **Dark frame edges are intentional** (PS1 fog + vignette dressing). Center
   regions show the lit table.
2. **Menu GL shot is intentionally sparse** — the menu is DOM-first; the 3D
   scene shows dressing only.
3. `10-combat-late` shows the map screen because the flat red/black policy lost
   the first battle before this shot — the combat visuals from the same run are
   in 05–09 and 12–13.
4. 5-bit quantization banding is a deliberate PS1 aesthetic (§11), not a bug.
5. Deck Customizer is DOM-UI driven (no separate 3D scene); its layout is
   visible in `ui-01-menu-fullpage.png`'s sibling screens (see 02–03 for the
   loadout cards UI in the DOM full-page variant).

## Open design questions this set surfaces
- Is the felt sector palette readable at 320×180 internal res? (11-rig-wheel_close is the stress case)
- Does the enemy table red-tint read clearly in enemy_side rig?
- Is ball visibility acceptable mid-spin (07/08) at low res?

## Card gallery (`cards/` subdirectory)

All **107 cards** from `content/cards.ron` (the single source of truth the engine
embeds — REQ-002), rendered with the existing `index.html` card design system
(`.card-item`, Outfit font, gold cost badge, rarity accents). Static DOM capture —
no game state involved, so compositor screenshots are reliable here.

| File | Group | Cards |
|---|---|---|
| cards/cards-00-gallery-full.png | Full gallery | all 107, grouped by type (tall: 1280×4208) |
| cards/cards-01-payout.png | payout | 24 |
| cards/cards-02-physics.png | physics | 27 |
| cards/cards-03-chaos.png | chaos | 3 |
| cards/cards-04-board.png | board | 20 |
| cards/cards-05-paint.png | paint | 12 |
| cards/cards-06-utility.png | utility | 15 |
| cards/cards-07-money.png | money | 6 |

Card visual language: left border + badge color = rarity
(common gray / uncommon green / rare blue / legendary gold), gold circle = cost (⚡),
description text is the verbatim RON `description`. Rarity accents are an added
presentation layer for review only (the in-game combat hand UI currently shows
plain id buttons — this gallery previews the intended card treatment).

Rebuild/re-shoot:
```bash
node /tmp/shots/cards/cardshots.mjs   # gallery.html + cards.json live in /tmp/shots/cards/
```