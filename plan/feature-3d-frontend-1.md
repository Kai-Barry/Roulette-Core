---
goal: 3D Web Frontend for Roulette.OS — three.js client over the roulette-core WASM engine, AI-playable by design
version: 1.0
date_created: 2026-09-14
owner: Roulette-Core maintainers
status: 'Planned'
tags: [feature, architecture, frontend, threejs, wasm, ai-playtest]
---

# Introduction

![Status: Planned](https://img.shields.io/badge/status-Planned-blue)

Implementation plan for the **full 3D game frontend** ("Roulette of the Damned" / Roulette.OS) built on top of the completed, headless `roulette-core` Rust engine. The frontend consumes the engine exclusively through its serde `Command`/`EngineEvent` API (wasm-bindgen bridge), never reimplements game rules, and is **AI-playable by construction**: every feature ships with a headless test path so an AI agent can playtest full runs in Node or in a DOM-less browser page, reading event-tail assertions instead of pixels, burning near-zero tokens.

Design compliance with the `game-design-lenses` skill (Schell) is not a final review step — every UI/flow phase carries **measurable lens gates** that block task completion until met.

## 1. Requirements & Constraints

### 1.1 Hard requirements

- **REQ-001**: The frontend MUST NOT reimplement any game rule. All state transitions flow through `roulette_core::api::Engine::apply(&Command)` via the WASM bridge; the client renders `EngineEvent` streams and serialized state only.
- **REQ-002**: Engine state is the single source of truth for card/economy data. The frontend MUST NOT duplicate the blueprint §6.3 card table; card definitions arrive from the engine's content (RON) via state serialization. This deliberately replaces the blueprint's `CardDatabase.ts` duplication.
- **REQ-003**: Every gameplay feature MUST be verifiable headless: `node` scripts drive the wasm engine and assert on `EngineEvent` streams with zero rendering, zero screenshots, zero manual browser interaction.
- **REQ-004**: A `?headless=1` page mode MUST boot the full app with WebGL and heavy DOM disabled, exposing a console/scriptable driver (`window.__RT`) for scripted play in a real browser when DOM-level verification is required.
- **REQ-005**: An autoplay policy bot MUST be able to complete full runs (map → combat → shop/forge → boss → victory/defeat) unattended, deterministically for a fixed seed, using only legal `Command`s.
- **REQ-006**: Deterministic seed reproduction: for seed S and command log L, replaying L must produce an identical event log on every machine/browser (engine guarantee from the core plan; the frontend must not inject nondeterministic state).
- **REQ-007**: Tech stack per blueprint §1.5, with one substitution: TypeScript strict + Vite + three.js (sole runtime dep) + pure DOM/CSS overlays (no UI framework) + Web Audio; game rules come from WASM (see ALT-001).
- **REQ-008**: The normative `EngineEvent` log remains float-free and replayable. Any float telemetry for 3D animation MUST be delivered on a side channel that is not part of the event log (see TASK-008).
- **REQ-009**: All player-visible state changes MUST reach the player within ≤ 100 ms through at least one channel (visual, textual, or audio) — Lens #57 feedback gate, verified headlessly by asserting UI-audit mode emits a `ui_feedback` record per state change (TASK-019).

### 1.2 Security & integrity

- **SEC-001**: No secrets, tokens, or user data enter the frontend; the wasm module ships only engine logic and RON content.
- **SEC-002**: The bridge MUST validate every inbound command JSON against the serde schema (reject unknown fields) and surface `EngineError` verbatim to the UI log; no client-side "fix-ups" of illegal commands.

### 1.3 Constraints

- **CON-001**: Engine-side work is limited to additive, feature-gated additions (telemetry snapshot, wasm crate). The normative engine logic, event log, and content schemas MUST NOT change semantics. All engine changes require the existing 156-test suite to stay green.
- **CON-002**: Content volume is engine-owned and currently 107 cards / 8 wheels / 6 enemies / 18 board upgrades / 10 curses / 5 events / 8 forge ops. Blueprint §6.3 specifies 113 cards; the 6-card gap is an engine content task OUT of frontend scope (tracked as ASSUMPTION-001); the frontend renders whatever the content crate provides.
- **CON-003**: Single-repo layout: all frontend code lives in `web/` (the existing `web/index.html` harness moves to `web/harness/` during migration and is deleted at Phase 7 completion).
- **CON-004**: Token economy: test suites MUST default to event/log assertions; screenshot pixel-diffing is allowed ONLY for the three visual-snapshot tests declared in TEST-006.
- **CON-005**: Performance budgets (measured headlessly where possible): engine `apply(Spin)` median ≤ 16 ms × 20 in wasm; three.js render loop ≥ 30 FPS on the reference scene; app bundle ≤ 2 MB gzipped (excluding wasm).

### 1.4 Guidelines & patterns

- **GUD-001**: AI-playtest protocol (mandatory per phase): each phase's tasks are "done" only when `npm run ai:check` passes — a scripted suite that boots the current build headlessly, plays scripted command sequences, and asserts phase-specific invariants from §6 Testing.
- **GUD-002**: Lens gates (game-design-lenses): UI tasks carry lens citations with measurable acceptance criteria; a gate failure blocks the task. Core gates: #48 accessibility/onboarding, #49 visible progress, #56 transparency, #57 feedback, #58 juiciness, #60 mode clarity, #33 meaningful choices, #38 flow challenge-matching, #20 judgment.
- **GUD-003**: No dominant strategies may be (re)introduced client-side: any client affordance (auto-bet suggestions, EV hints) must derive from the engine's own EV tooling, never from client-side heuristics that could contradict DEC-001's probabilistic prediction model.
- **PAT-001**: Event-driven rendering (mirror of engine PAT-004): UI and 3D are pure functions of `(EngineState snapshot, EngineEvent stream)`; no component mutates game state directly — components dispatch `Command`s through `GameClient`.
- **PAT-002**: Golden-event-tail testing: assertions match the tail of the serialized event log (last N events, field-wise), never screenshots or timing, except where CON-004 allows.
- **PAT-003**: Screens as state machines keyed by `GameState` (engine `run.state`), one DOM container per state; CSS classes toggle visibility — the same `visibility` + opacity-transition pattern proven in the harness (fixes the modal-respawn bug class).

## 2. Implementation Steps

### Implementation Phase 0 — AI Test Infrastructure (build FIRST so every later phase is AI-verifiable)

- GOAL-001: Stand up the headless, token-cheap AI playtest infrastructure before any UI code exists.

| Task | Description | Completed | Date |
|------|-------------|-----------|------|
| TASK-001 | Spike `web/test/node-wasm.mjs`: Node ≥ 22 loads the wasm bridge (TASK-010 output) via `WebAssembly` + a glue shim, boots `Engine::new(content, seed)`, applies `StartRun`, prints the event stream JSON. Exit criterion: one full run boots and logs `RunStarted` + `BattleStarted` events in < 2 s. | | |
| TASK-002 | Implement `web/test/matchers.mjs`: `expectEvents(log, [ {event:'spin_resolved', …}, … ])` tail matcher (PAT-002), `expectInvariant(fn)` helpers, and a `runScripted(cmds)` driver that applies commands, collects events, and pretty-prints failures as compact JSON diffs (one line per failure — token-cheap output). | | |
| TASK-003 | Implement `tools/ai-play.mjs`: policy bots `random`, `greedy` (highest-EV legal bet per engine state), `scripted <file>` (JSON command log). Loop: pick legal `Command` from engine state snapshot → apply → append event → repeat until `GameOver|RunVictory` or command cap. Flags: `--seed`, `--policy`, `--max-commands`, `--json` (machine summary: commands, events, rounds, outcome, invariants-ok). | | |
| TASK-004 | Implement `web/test/invariants.mjs` — the invariant suite run after every phase (see §6 Testing list). | | |
| TASK-005 | Add `npm run ai:check` orchestrator: runs invariants + `ai-play.mjs` (random + greedy, seeds 1–3) against the wasm build; zero-exit on success; prints ≤ 20 lines on success. This is GUD-001's gate command. | | |

### Implementation Phase 1 — WASM bridge (the only engine-side surface change)

- GOAL-002: Expose `roulette-core` to JS via a `roulette-wasm` crate with a telemetry side channel, and freeze the TS↔Rust schema contract.

| Task | Description | Completed | Date |
|------|-------------|-----------|------|
| TASK-006 | Create crate `crates/roulette-wasm` (wasm-bindgen + serde-wasm-bindgen, target `wasm32-unknown-unknown`): exports `new_engine` (wrapping `Arc<Content>` + `Engine`, content RON embedded via `include_str!` at build) → `EngineHandle`; `apply_command(cmd_json) -> Result<EventsJson, ErrorJson>`; `get_state_json()` (run + battle + hand + board summary); `undo()`; `undo_depth()`; `seed()`. Additive only (CON-001). | | |
| TASK-007 | TS type generation: add `ts-rs` derive (feature-gated) on `Command`, `EngineEvent`, `GameState`, `BetType`, `SlotColor`, `WheelConfig`, `BattleState` view structs; commit generated `web/src/engine/schema.ts`; add drift test `crates/roulette-wasm/tests/ts_schema_snapshot.rs` failing on uncommitted type changes. | | |
| TASK-008 | Telemetry side channel (REQ-008, cargo feature `telemetry`): add `Simulator::snapshot() -> (f64 wheel_angle, Vec<f64> ball_angles)` (additive inherent accessor — private fields stay private); wasm export `spin_telemetry(spin_id) -> Uint8Array` returning fixed-dt frames (f32-packed wheel_angle + N ball angles) for the visual layer only; the normative event log stays untouched. | | |
| TASK-009 | Perf gate `crates/roulette-wasm/tests/perf.rs` (native): `apply(Spin)` under `SpinSampling::Physics` median ≤ 16 ms × 20 over 200 spins on the default wheel; fails the build if exceeded. | | |
| TASK-010 | Node integration: `wasm-pack build --target nodejs` output consumed by TASK-001's harness (built in CI cache, not committed to git). Verify `ai:check` runs green against the real bridge. | | |

### Implementation Phase 2 — Game client core (TS, headless-testable, no rendering)

- GOAL-003: A typed `GameClient` that owns the wasm handle, dispatches `Command`s, projects state, persists, and replays.

| Task | Description | Completed | Date |
|------|-------------|-----------|------|
| TASK-011 | `web/src/client/GameClient.ts`: wraps `EngineHandle`; methods mirror the `Command` enum exactly (`startRun`, `pickNode`, `playCard`, `placeBet`, `spin`, …); emits `ClientEvent`s (`stateChanged`, `events`) to subscribers; throws typed `EngineErrorJson` upward (SEC-002). | | |
| TASK-012 | `web/src/client/projections.ts`: pure functions projecting engine state JSON into view models (`MapView`, `CombatView`, `ShopView`, `ForgeView`, `EventView`, `HudView`). Every projection is a pure function — unit-tested headlessly in `web/test/projections.test.mjs` with fixtures captured from real engine output (no hand-made mocks — PAT-001). | | |
| TASK-013 | Persistence: autosave engine snapshots (serde JSON from `get_state_json` + full command log) to `localStorage` on every event; resume-on-load replays the command log through a fresh engine and asserts the state hash matches (REQ-006 verified end-to-end); corrupt-snapshot path falls back to clean boot. | | |
| TASK-014 | Headless app shell: `web/src/main.ts` boots with `?headless=1` → no `RenderManager`, no audio, DOM screens stubbed; still full `GameClient`. `window.__RT = { client, state(), events(n), cmd(c), run(policy) }` (REQ-004) — the same console-API pattern the harness smoke tests used successfully on `index.html`. | | |

### Implementation Phase 3 — DOM UI (all screens; lens-gated; verified via `__RT`)

- GOAL-004: Every blueprint §12 screen as DOM overlays keyed by `GameState`, each with a lens gate and a headless verification path.

| Task | Description | Completed | Date |
|------|-------------|-----------|------|
| TASK-015 | Screen framework: `ScreenManager` keyed by `GameState` (PAT-003); `?headless=1` renders screens as data (no CSS animation timing dependencies); every screen exposes `data-` attributes for `__RT` queries. Verify: state transitions emit exactly one visible-screen change (TEST-012). | | |
| TASK-016 | MENU + LOADOUT screens: draft UI (5 cards + 1 wheel, store-points budget), difficulty selector. Lens gate #33: each draft option shows cost + one-line effect, and no option is flagged dominant by the engine's EV calculator (GUD-003); gate #49: budget spent shown live. | | |
| TASK-017 | MAP screen: branch map renderer (SVG/DOM), node tooltips (tier, round limit), pick flow. Lens gates #49: current floor/total, completed nodes visually marked; #60: node types color+icon distinct (test: two arbitrary nodes never share full visual signature). | | |
| TASK-018 | COMBAT screen: HUD (HP, enemy HP + intent, chips/PTS, round x/y), hand (draw pile count, playable states), bet chips 1/5/10 + merge/remove/clear/rebet, sacrifice, prediction UI (sector overlay from `PredictionShown`), spin report panel (§10 resolution breakdown verbatim from events). Lens gates: #57 every command → ≥1 event → ≥1 UI change ≤100 ms (TEST-013 ui-feedback); #56 all actionable controls enabled iff legal per engine state (TEST-015 legality audit walks every `Command` in every state); #20 spin report always explains WHY points were gained/lost. | | |
| TASK-019 | Feedback instrumentation for gates: UI-audit mode (`?headless=1&audit=1`) records `ui_feedback` records (command → channels touched) consumed by TEST-013; covers #58 juiciness inventory: chip place, card play, damage thud, bell, peg tick (audio recipes verified by existence + dispatch, not by listening). | | |
| TASK-020 | SHOP + FORGE + EVENT screens: offers from engine state (REQ-002 — no local card data), forge ops incl. customizer (`CustomizeOp` slot cycle/add/remove/set-number), event choices with consequences shown BEFORE commit (#33). Gates: undo affordance visible whenever `undo_depth > 0` (#20 fair judgment); forge reroll cost always visible. | | |
| TASK-021 | END screens (Victory/GameOver) + PLAY AGAIN; run summary (floors, chips spent, biggest pot) computed from the event log (REQ-006 replay). Gate #49: progress recap visible. | | |
| TASK-022 | Settings + mobile mode (§12.9): three volume buses persisted; `?headless=1` skips audio entirely. | | |

### Implementation Phase 4 — 3D renderer (three.js), visual layer only

- GOAL-005: Blueprint §11 scene: two wheels + felts + lighting + dust + PS1 post pass; all animation driven by engine telemetry/events; AI-verifiable via data-level checks plus the 3 allowed snapshot tests.

| Task | Description | Completed | Date |
|------|-------------|-----------|------|
| TASK-023 | `web/src/render/RenderManager.ts` + `PS1Shader.ts`: scene bootstrap, pixelation+dither+quantization post pass, 9-camera rig (§11.3), `?no3d=1` escape hatch for token-cheap checks. | | |
| TASK-024 | `WheelVisual.ts`: wheel/ball meshes + canvas textures from `WheelConfig` (REQ-002: numbers/colors from engine state — the render never invents slots). Spin animation consumes `spin_telemetry` frames (TASK-008); landing frame MUST match `EngineEvent::BallLanded` number — TEST-010 telemetry/landed-agreement. | | |
| TASK-025 | Table felt, chip stacks (instanced), croupier figures, fog + dust particles; raycast picking: felt sectors → `placeBet`, cards → `playCard`, bell → `spin` (same commands as DOM — PAT-001, single path). | | |
| TASK-026 | Intent animation + special-color on-land effects (§10.7) driven purely by `EngineEvent`s. | | |
| TASK-027 | Perf gate: reference headless render (SwiftShader) must sustain 33 ms/frame on the reference scene; automated in the TEST-006 snapshot run. | | |

### Implementation Phase 5 — Audio (procedural, engine-event-driven)

- GOAL-006: Blueprint §13 audio: drone, title layers, step-sequenced encounter music, SFX recipes; physics click/bounce tracks driven by engine-forwarded sim events.

| Task | Description | Completed | Date |
|------|-------------|-----------|------|
| TASK-028 | `SoundManager.ts`: three buses (music 55% / drone 15% / sfx 80%), settings persistence, all §13.4 SFX recipes as Web Audio graphs. | | |
| TASK-029 | Physics-driven click/bounce: surface `SimEvent::DividerTick/PegHit` via the bridge event stream (additive forwarding under the `telemetry` feature, CON-001); renderer triggers them in sync. | | |
| TASK-030 | Encounter-music sequencer with §13.3 tempo table; boss layering. Headless check: bus states + recipe existence via an audio-diagnostics export (TEST-013), no audible assertions. | | |

### Implementation Phase 6 — Balancer/EV studio + full-game AI playtest expansion

- GOAL-007: Ship the §15 balancer tooling against the real engine and widen the AI playtest to full-run strategy exploration.

| Task | Description | Completed | Date |
|------|-------------|-----------|------|
| TASK-031 | `balancer.html`: card library from engine content, 5 scenario presets, damage heatmap, EV metrics, ratings — reusing engine EV output via wasm (REQ-002; §14.3 rating bands are engine-owned). | | |
| TASK-032 | Extend `tools/ai-play.mjs` with `policy=ev` (uses engine-published per-bet EV from state) and multi-seed campaign mode (`--campaign 100` → aggregate JSON: win rate by difficulty, avg run length, economy curves) — the design-lens balance evidence stream (GUD-002). | | |
| TASK-033 | Softlock sweep automation: for every `GameState`, enumerate all `Command`s; assert ≥1 legal command exists in every reachable non-terminal state — prevents the bankruptcy soft-lock class fixed in the harness. | | |

### Implementation Phase 7 — Lens audit, E2E, migration, ship

- GOAL-008: Formal game-design-lenses audit, full-run E2E, harness migration, deployment.

| Task | Description | Completed | Date |
|------|-------------|-----------|------|
| TASK-034 | Run the full lens audit (game-design-lenses skill, all-100-lenses checklist via `scripts/generate_audit.py --all`) against the built game; produce `.agents_tmp/design_audit_3d.md`; fix all gameplay-blocking findings before ship; log decisions in `docs/DECISIONS.md`. | | |
| TASK-035 | E2E suite (browser, headless page): one full run per difficulty via `__RT.run(policy)`, plus UI-level checks (screens render, no console errors, feedback audit clean). ≤ 5 screenshots total for visual regression (TEST-006). | | |
| TASK-036 | Migrate `web/index.html` harness → `web/harness/` → delete after parity check: every harness feature must exist in the real build (card costs, redraw, skip-turn, second chance, gold/Exact# semantics — all engine-authoritative now). | | |
| TASK-037 | Deployment: Vite build, Dockerfile + nginx static serve (§1.5), CI: `ai:check` + cargo tests + tsc strict + bundle-size budget (CON-005). | | |

## 3. Alternatives

- **ALT-001**: Reimplement rules in TypeScript per the blueprint's original §16 roadmap (GameEngine.ts). Rejected: the engine now exists, is tested (156 tests), data-driven, and deterministic; a TS reimplementation would fork the normative ruleset and violate the single source of truth (REQ-001).
- **ALT-002**: Client-side physics replica (ghost sim) instead of telemetry export. Rejected as primary: divergence risk between ghost and authoritative `BallLanded` would require constant reconciliation; a telemetry export from the same simulator is exact by construction and additive (CON-001). Ghost fallback remains possible if wasm binary size is a problem (RISK-004).
- **ALT-003**: UI framework (React/Svelte) for screens. Rejected: blueprint mandates pure DOM overlays (REQ-007), and framework state would duplicate engine state — violating PAT-001's single-source-of-truth projection model.
- **ALT-004**: Pixel/screenshot-first AI testing (playtest by looking at rendered output). Rejected: token-expensive and flaky; event-tail assertions (PAT-002) cover all logic; screenshots only where pixels are the feature (CON-004).
- **ALT-005**: Ship 3D and UI together from day one. Rejected: violates the AI-playability principle — UI phases would be unverifiable headlessly; Phase 0 infrastructure must precede everything (GUD-001).

## 4. Dependencies

- **DEP-001**: `wasm-bindgen`, `serde-wasm-bindgen`, `wasm-pack`, `ts-rs` (dev) — new engine-adjacent deps, additive and feature-gated (CON-001).
- **DEP-002**: three.js r0.184 (sole runtime frontend dependency, REQ-007).
- **DEP-003**: Node ≥ 22 (present: v22.23.2) for the headless wasm harness; Vite ≥ 5 dev toolchain.
- **DEP-004**: Existing `roulette-core` 156-test suite must remain green for any engine-touching task (CON-001).
- **DEP-005**: Google Fonts (VT323, Courier Prime) — static CDN links as in the harness.

## 5. Files

- **FILE-001**: `crates/roulette-wasm/` — new crate: `src/lib.rs` (exports), `src/telemetry.rs` (feature-gated), `tests/ts_schema_snapshot.rs`, `tests/perf.rs`.
- **FILE-002**: `crates/roulette-core/src/phys/simulator.rs` — additive `snapshot()` accessor only.
- **FILE-003**: `web/src/` — `main.ts`, `client/GameClient.ts`, `client/projections.ts`, `engine/schema.ts` (generated), `render/{RenderManager,WheelVisual,PS1Shader}.ts`, `ui/{GameUI,ScreenManager}.ts`, `ui/style.css`, `audio/SoundManager.ts`.
- **FILE-004**: `web/test/` — `node-wasm.mjs`, `matchers.mjs`, `invariants.mjs`, `projections.test.mjs`, `e2e.mjs`.
- **FILE-005**: `tools/ai-play.mjs`, `tools/ai-check.mjs` — the AI playtest drivers (GUD-001 gate).
- **FILE-006**: `balancer.html` + `tools/balance-cards.ts` equivalents consuming wasm EV output.
- **FILE-007**: `web/index.html` — production app shell (title "Roulette.OS"); `web/harness/` — migrated legacy harness (deleted at TASK-036).
- **FILE-008**: `plan/feature-3d-frontend-1.md` — this plan; `docs/DECISIONS.md` — ADR entries for frontend decisions (ALT-001, telemetry side channel).

## 6. Testing

- **TEST-001**: Engine contract: every `Command` variant round-trips through the wasm bridge (serde JSON) and returns the documented events; illegal commands return `EngineError` verbatim (SEC-002). Headless, `web/test/node-wasm.mjs`.
- **TEST-002**: Invariants after every state change: chips ≥ 0, HP bounds, hand ≤ limit, no negative pools, `undo_depth ≤ 64`, event-log ordering (`BattleStarted` before `BetPlaced` before `SpinResolved` within a round).
- **TEST-003**: No-softlock: every reachable non-terminal `GameState` has ≥ 1 legal `Command` (TASK-033 suite, random + greedy policies, seeds 1–50).
- **TEST-004**: Determinism: same seed + command log → byte-identical event log; replay across Node and headless browser produces identical hashes (REQ-006).
- **TEST-005**: Full-run autoplay: greedy policy completes a SHORT run (7 floors) to `RunVictory` on seed set 1–10 within command cap 5,000; `random` policy terminates (victory or defeat) — no hangs.
- **TEST-006**: Visual regression (only screenshot tests allowed, CON-004): menu screen, combat felt with bets placed, PS1 post-pass frame — 3 PNG snapshots, threshold-diffed.
- **TEST-007**: Schema drift: `ts_schema_snapshot.rs` fails the build when Rust types change without regenerating `schema.ts` (TASK-007).
- **TEST-008**: Perf: native spin-median gate ≤ 16 ms × 20 on the default wheel (TASK-009); wasm smoke equivalent in CI.
- **TEST-009**: Persistence: kill-and-reload mid-run resumes to identical state hash; corrupt snapshot → clean boot (TASK-013).
- **TEST-010**: Telemetry agreement: the final telemetry frame's settled slot equals `EngineEvent::BallLanded` number for 100 consecutive spins per seed (TASK-024 gate).
- **TEST-011**: Economy sanity via campaign mode: win-rate by difficulty in plausible bands (SHORT > MEDIUM > LONG), no infinite-chip exploit reachable by greedy policy (GUD-003, DEC-001).
- **TEST-012**: Mode clarity: for each `GameState` switch, `ScreenManager` shows exactly one screen; HUD labels switch per mode (regression of the stale-HUD harness bug).
- **TEST-013**: Audio/UI diagnostics in headless audit mode: every state change produced ≥ 1 registered feedback channel invocation (TASK-019, REQ-009).
- **TEST-014**: Accessibility gate #48: the first-run LOADOUT screen is completable using only documented `__RT` commands (proves the onboarding path is machine-followable, i.e., human-readable).
- **TEST-015**: Legality audit (#56): walk every `Command` in every reachable state; assert each disabled control is genuinely illegal per the engine and each enabled control is legal (transparency regression of the harness's exact-number/illegal-bet bug class).

## 7. Risks & Assumptions

- **RISK-001**: wasm binary size + 107-card content embed may exceed bundle budget (CON-005). Mitigation: `wasm-opt -Oz`, content as a separate fetched JSON asset, ghost-sim fallback (ALT-002) if telemetry inflates size.
- **RISK-002**: three.js r0.184 + WebGL2 in CI (headless GL) can be flaky. Mitigation: SwiftShader flags in CI; the 3-snapshot budget keeps flake surface tiny; all logic tests remain GL-free.
- **RISK-003**: Prediction UI could re-introduce a dominant strategy if the client over-visualizes sectors. Mitigation: the sector overlay renders exactly what `PredictionShown` events declare; EV-hint affordances derive from engine-published EV only (GUD-003).
- **RISK-004**: `ts-rs` derive may not cover every enum payload shape (e.g., `BetType::Number(u32)`). Mitigation: fallback to a handwritten mapping verified by the drift test (TEST-007).
- **ASSUMPTION-001**: Engine content (107 cards etc.) is acceptable for the v1 frontend; topping up to blueprint §6.3's 113 cards is an engine-content task outside this plan's scope (REQ-002 keeps the frontend content-agnostic either way).
- **ASSUMPTION-002**: The engine's `Command`/`EngineEvent` API (849-line `api/mod.rs`) is stable enough to freeze as the frontend contract; breaking changes require a schema-drift test update, not client rewrites.
- **ASSUMPTION-003**: Browser localStorage persistence is acceptable for run saves (no cloud sync in v1).

## 8. Related Specifications / Further Reading

- `docs/ROULETTE_OS_GAME_BLUEPRINT.md` — normative game design (§1.4 tone, §2 run, §4 wheel, §11 renderer, §12 UI, §13 audio, §15 balancer, §16 schemas).
- `docs/DECISIONS.md` — ADR log; DEC-001 (probabilistic prediction) binds the frontend's prediction UI.
- `.agents_tmp/PLAN.md` — the completed engine plan (roulette-core v2.0).
- `.agents/skills/game-design-lenses/SKILL.md` + `references/` — lens definitions used by GUD-002 gates and TASK-034.
- `.agents_tmp/design_audit_ui.md` — harness-era audit; its 8 findings are regression tests for the real frontend (TASK-036).
- `.agents/skills/create-implementation-plan/SKILL.md` — plan template.