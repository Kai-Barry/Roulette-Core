/**
 * Phase 4 gates — headless three.js (no WebGL): scene graph introspection,
 * PS1 shader data, wheel visuals from WheelConfig, telemetry-driven spin
 * agreement with BallLanded (TEST-010), camera rig, no3d escape hatch.
 */

import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require_ = createRequire(import.meta.url);
const wasm = require_('/projects/Roulette-Core/crates/roulette-wasm/pkg/roulette_wasm.js');
const { GameClient } = await import('../src/client/GameClient.ts');
function newClient(seed) {
  return new GameClient(wasm, seed);
}
import {
  RIG_NAMES,
  RenderManager,
} from '../src/render/RenderManager.ts';
import { createPs1Pass, PS1_FRAGMENT, PS1_VERTEX } from '../src/render/PS1Shader.ts';
import { buildWheelVisual, telemetryFrames, wheelFaceDrawList, SLOT_PALETTE } from '../src/render/WheelVisual.ts';
import { Color, Vector3 } from 'three';

let passed = 0;
const failures = [];
function ok(name, fn) {
  try {
    fn();
    passed++;
    console.log(`ok ${passed} - ${name}`);
  } catch (e) {
    failures.push({ name, error: e });
    console.log(`FAIL ${name}: ${e.message}`);
  }
}

/** Client that finished loadout so a battle exists (needed for telemetry). */
function clientInCombat(seed = 'render1') {
  const c = newClient(seed);
  c.startRun('short');
  const offer = c.state().run?.loadout_offer;
  for (const id of offer?.card_ids ?? []) c.draftCard(id);
  c.draftWheel();
  c.finishLoadout();
  c.pickNode(c.state().run.map.floors[0][0].id);
  assert.equal(c.state().game_state, 'combat');
  return c;
}

/** REQ-002: the render layer reads the wheel from engine state — here the
 * battle's player wheel config. */
function wheelOf(c) {
  const b = c.state().battle;
  assert.ok(b?.player_wheel, 'battle carries player_wheel config');
  return b.player_wheel;
}

ok('TASK-023: nine-camera rig (§11.3) with distinct deterministic positions', () => {
  const rm = new RenderManager(wasm);
  assert.deepEqual(rm.graph().rig, [...RIG_NAMES]);
  assert.equal(rm.rig.length, 9);
  const seen = new Set(rm.rig.map((r) => `${r.camera.position.x},${r.camera.position.y},${r.camera.position.z}`));
  assert.equal(seen.size, 9, 'all rig positions distinct');
  // cameras look at table center
  for (const r of rm.rig) {
    assert.ok(r.camera.rotation.x !== 0 || r.camera.rotation.y !== 0, 'camera oriented at target');
  }
});

ok('TASK-023: scene graph built from client state; ?no3d skips renderer', () => {
  const c = clientInCombat();
  const rm = new RenderManager(c, { no3d: true });
  const g = rm.graph();
  assert.ok(g.meshes >= 4, 'table, enemy table, wheel, ball present');
  assert.equal(g.active, 'table_front');
  // switching active camera is data-only in headless
  rm.active = 'wheel_top';
  assert.equal(rm.graph().active, 'wheel_top');
});

ok('TASK-023: PS1 pass — pixelation+dither+quantization shader data', () => {
  const pass = createPs1Pass(true);
  assert.ok(PS1_FRAGMENT.includes('floor(px) + 0.5'), 'pixelation snap');
  assert.ok(PS1_FRAGMENT.includes('bayer4'), 'ordered dither');
  assert.ok(PS1_FRAGMENT.includes('* 31.0'), '5-bit quantization');
  assert.ok(PS1_VERTEX.includes('projectionMatrix'));
  pass.setResolution(320, 180);
  assert.deepEqual(pass.material.uniforms.uResolution.value, [320, 180]);
  assert.equal(pass.material.uniforms.uDither.value, 1);
  const noDither = createPs1Pass(false);
  assert.equal(noDither.material.uniforms.uDither.value, 0);
});

ok('TASK-024: wheel face draw-list painted from WheelConfig (REQ-002)', () => {
  const c = clientInCombat();
  const cfg = wheelOf(c);
  assert.ok(cfg.numbers.length > 0, 'wheel config from engine state');
  const ops = wheelFaceDrawList(cfg);
  const wedges = ops.filter((o) => o.kind === 'wedge');
  assert.equal(wedges.length, cfg.numbers.length, 'one wedge per canonical slot');
  assert.deepEqual(wedges.map((o) => Number(o.text)), [...cfg.numbers], 'slot order preserved');
  // palette colors derived ONLY from config fields
  const greens = new Set(cfg.green_numbers);
  const reds = new Set(cfg.red_numbers);
  for (let i = 0; i < cfg.numbers.length; i++) {
    const num = cfg.numbers[i];
    const expect = cfg.slot_colors[String(num)]
      ?? (greens.has(num) ? 'green' : reds.has(num) ? 'red' : 'black');
    assert.equal(wedges[i].fill, `#${new Color(SLOT_PALETTE[expect]).getHexString()}`, `slot ${num} color`);
  }
  // hub op present
  assert.ok(ops.some((o) => o.kind === 'hub'));
});

ok('TASK-024: wheel visual meshes keyed to slot count', () => {
  const c = clientInCombat();
  const cfg = wheelOf(c);
  const wv = buildWheelVisual(cfg);
  assert.equal(wv.slotNumbers.length, cfg.numbers.length);
  const face = wv.group.children.find((x) => x.name === 'wheel_spin');
  assert.ok(face, 'spin group present');
  wv.setWheelAngle(1.5);
  assert.ok(Math.abs(face.rotation.y - 1.5) < 1e-9);
  wv.setBallPositions([0.3]);
  const ball = wv.group.children.find((x) => x.name === 'ball_0');
  assert.ok(Math.abs(ball.position.x - Math.cos(0.3) * 3.6) < 1e-9);
});

ok('TEST-010: final telemetry frame settled slot === BallLanded (per spin)', () => {
  const c = clientInCombat('telem1');
  // Per-battle wheel snapshot: the wheel config is fixed for a battle, and
  // both the landed event and telemetry must read against the SAME config.
  let cfg = wheelOf(c);
  let events = [];
  for (let i = 0; i < 40; i++) {
    c.placeBet('red', 1);
    events = c.spin();
    if (events.some((e) => e.event === 'ball_landed') && c.state().game_state === 'combat') break;
    if (c.state().game_state !== 'combat') {
      const run = c.state().run;
      const all = run.map.floors.flat();
      const cur = all.find((n) => n.id === run.current_node);
      const pickable = all.filter((n) => !n.completed && cur.connections.includes(n.id));
      c.pickNode((pickable.find((n) => n.node_type === 'combat') ?? pickable[0]).id);
      if (c.state().game_state === 'combat') cfg = wheelOf(c);
    }
  }
  const landed = events.filter((e) => e.event === 'ball_landed' && e.side === 'player').map((e) => e.number ?? e.slot ?? null);
  assert.ok(landed.length > 0, 'BallLanded event emitted');
  const wv = buildWheelVisual(cfg);
  const frames2 = telemetryFrames(c.spinTelemetry(0));
  assert.ok(frames2.length > 0, 'telemetry side channel populated');
  const last = frames2[frames2.length - 1];
  const settled = wv.settledSlot(last.ballAngles[0], last.wheelAngle);
  assert.equal(settled, landed[landed.length - 1], 'settled wedge equals BallLanded');
});

ok('TEST-010 ×N: agreement holds across 20 consecutive landed spins', () => {
  const c = clientInCombat('telem2');
  let agreed = 0;
  let spins = 0;
  let wv = buildWheelVisual(wheelOf(c));
  for (let i = 0; i < 240 && spins < 20; i++) {
    if (c.state().game_state !== 'combat') {
      const run = c.state().run;
      const all = run.map.floors.flat();
      const cur = all.find((n) => n.id === run.current_node);
      const pickable = all.filter((n) => !n.completed && (cur ? cur.connections.includes(n.id) : n.floor === 0));
      const next = pickable.find((n) => n.node_type === 'combat') ?? pickable[0];
      if (!next) break;
      c.pickNode(next.id);
      if (c.state().game_state === 'combat') wv = buildWheelVisual(wheelOf(c));
      continue;
    }
    const b = c.state().battle;
    if (!b || (b.chips_pool ?? 0) < 1) continue;
    c.placeBet('red', 1);
    const events = c.spin();
    const landedEv = [...events].reverse().find((e) => e.event === 'ball_landed' && e.side === 'player');
    if (!landedEv) continue;
    spins++;
    const frames = telemetryFrames(c.spinTelemetry(0));
    if (!frames.length) continue;
    const last = frames[frames.length - 1];
    if (wv.settledSlot(last.ballAngles[0], last.wheelAngle) === (landedEv.number ?? landedEv.slot)) agreed++;
  }
  assert.ok(spins >= 20, `performed ${spins} landed spins`);
  assert.equal(agreed, spins, `telemetry/landed agreement ${agreed}/${spins} (TEST-010 is exact)`);
});

ok('TASK-024: wheel angle drives ball-relative motion (frames change)', () => {
  const c = clientInCombat('telem3');
  c.placeBet('red', 1);
  c.spin();
  const frames = telemetryFrames(c.spinTelemetry(0));
  assert.ok(frames.length > 0, 'frames after a spin');
  const first = frames[0];
  const last = frames[frames.length - 1];
  const moved = Math.abs(first.wheelAngle - last.wheelAngle) > 1e-4
    || Math.abs(first.ballAngles[0] - last.ballAngles[0]) > 1e-4;
  assert.ok(moved, 'telemetry frames animate (wheel or ball move)');
});

ok('REQ-002: renderer never invents slots — config identity round-trips', () => {
  const c = clientInCombat('telem4');
  const cfg1 = wheelOf(c);
  const wv = buildWheelVisual(cfg1);
  assert.deepEqual(wv.slotNumbers, cfg1.numbers, 'slot order is engine canonical');
});

ok('TASK-024: RenderManager.attachWheel mounts engine-config wheel; no rebuild on same config', () => {
  const c = clientInCombat();
  const rm = new RenderManager(c, { no3d: true });
  const cfg = wheelOf(c);
  rm.attachWheel(cfg);
  assert.equal(rm.wheelSlotCount, cfg.numbers.length, 'slots derived from WheelConfig');
  assert.ok(rm.graph().meshes >= 6, 'wheel visual meshes added');
  // Same config again → identity preserved (no rebuild).
  const before = rm.graph().meshes;
  rm.attachWheel(cfg);
  assert.equal(rm.graph().meshes, before, 'no rebuild for identical config');
  // Mutated config (forge-style slot recolor) → rebuild picks it up.
  const mutated = { ...cfg, slot_colors: { ...cfg.slot_colors, '3': 'gold' } };
  rm.attachWheel(mutated);
  assert.equal(rm.wheelSlotCount, mutated.numbers.length);
});

ok('TASK-024: manager telemetry playback — settled wheel equals BallLanded (TEST-010 path)', () => {
  const c = clientInCombat('telem5');
  const rm = new RenderManager(c, { no3d: true });
  rm.attachWheel(wheelOf(c));
  c.placeBet('red', 1);
  const events = c.spin();
  const landedEv = [...events].reverse().find((e) => e.event === 'ball_landed' && e.side === 'player');
  assert.ok(landedEv, 'spin landed');
  rm.playSpin(0, c.spinTelemetry(0));
  assert.ok(rm.playbackActive, 'playback started from telemetry bytes');
  // Tick through the whole playback at 1/60 s per tick (2 frames/tick).
  for (let i = 0; i < 100000 && rm.playbackActive; i++) rm.tick(1 / 60);
  assert.ok(!rm.playbackActive, 'playback finished');
  const last = rm.graph().playback.frame;
  assert.ok(last > 0, `advanced ${last} telemetry frames`);
  // Final wheel/ball state must be the telemetry tail — same agreement TEST-010 asserts.
  const frames = telemetryFrames(c.spinTelemetry(0));
  const tail = frames[frames.length - 1];
  const wv = rm.wheelVisual();
  assert.ok(wv, 'wheel visual mounted');
  const settled = wv.settledSlot(tail.ballAngles[0], tail.wheelAngle);
  assert.equal(settled, landedEv.number, 'manager playback settles on BallLanded');
});

ok('TASK-023: webgl flag off in Node — renderer stays null, graph intact', () => {
  const c = clientInCombat();
  const rm = new RenderManager(c);
  assert.equal(rm.renderer, null, 'no GL context in Node');
  assert.ok(rm.graph().meshes >= 4);
  rm.render(); // no-op without renderer; must not throw
});

ok('TASK-025: dressing — felt sectors, bell, chips, croupiers, dust, fog', () => {
  const c = clientInCombat();
  const rm = new RenderManager(c, { no3d: true });
  const g = rm.graph().dressing;
  assert.deepEqual(g.sectors, ['red', 'black', 'green', 'odd', 'even'], 'five felt sectors');
  assert.equal(g.bell, true);
  assert.equal(g.chipInstances, 15, '3 chips × 5 sectors');
  assert.equal(g.croupiers, 4, 'two figures = body+head each');
  assert.ok(rm.dressing.dust.geometry.getAttribute('position').count >= 200, 'dust particles present');
  assert.ok(rm.scene.fog, 'fog attached');
  // Sector meshes carry their pick payload (raycast target registry).
  for (const s of rm.dressing.sectors) {
    assert.deepEqual(s.mesh.userData.pick, { kind: 'felt', bet: s.bet });
  }
  assert.deepEqual(rm.dressing.bell.userData.pick, { kind: 'bell' });
});

ok('TASK-025: raycast picking — sector hit → place_bet payload, bell → spin (PAT-001)', () => {
  const c = clientInCombat();
  const rm = new RenderManager(c, { no3d: true });
  rm.active = 'table_front';
  const cam = rm.camera('table_front');
  // Straight-down ray onto the red sector center → felt red.
  const red = rm.dressing.sectors.find((s) => s.bet === 'red');
  const down = new Vector3(0, -1, 0);
  const hit = rm.pick('felt', undefined, red.center.clone().setY(6), down);
  assert.deepEqual(hit, { kind: 'felt', bet: 'red' });
  // Bell ray → spin payload.
  const bell = rm.dressing.bell.position.clone().setY(4);
  const hitBell = rm.pick('bell', undefined, bell, down);
  assert.deepEqual(hitBell, { kind: 'bell' });
  // Miss (empty sky) → null.
  assert.equal(rm.pick('felt', undefined, new Vector3(0, 30, 0), down), null);
  // Camera unproject path: NDC center of a sector yields the same pick.
  const ndcPoint = red.center.clone().project(cam);
  const origin = cam.position.clone();
  const dir = new Vector3(ndcPoint.x, ndcPoint.y, 0.5).unproject(cam).sub(origin).normalize();
  assert.deepEqual(rm.pick('felt', undefined, origin, dir), { kind: 'felt', bet: 'red' });
});

ok('TASK-025: dust drifts upward over time (atmosphere animates)', () => {
  const c = clientInCombat();
  const rm = new RenderManager(c, { no3d: true });
  const attr = rm.dressing.dust.geometry.getAttribute('position');
  const y0 = Array.from({ length: 5 }, (_, i) => attr.getY(i));
  for (let i = 0; i < 60; i++) rm.update(1 / 60); // ~1 s of updates
  let moved = 0;
  for (let i = 0; i < 5; i++) {
    const y1 = attr.getY(i);
    if (y1 !== y0[i]) moved++;
  }
  assert.ok(moved > 0, `dust moved for ${moved}/5 sampled particles`);
});

ok('TASK-026: special-color on-land burst from engine WheelConfig (REQ-002)', () => {
  const c = clientInCombat('fx1');
  const rm = new RenderManager(c, { no3d: true });
  const cfg = wheelOf(c);
  rm.attachWheel(cfg);
  // A special-colored slot (from the config, e.g. slot_colors gold on 7).
  const specials = Object.entries(cfg.slot_colors).filter(([, col]) => ['gold', 'purple', 'cyan', 'crimson'].includes(col));
  assert.ok(specials.length > 0, 'classic wheel has a special slot color');
  const [numStr, col] = specials[0];
  assert.ok(rm.wouldBurst('player', Number(numStr)), `slot ${numStr} (${col}) bursts`);
  // Red/black/green never burst.
  const plain = cfg.numbers.find((n) => !cfg.slot_colors[String(n)]);
  if (plain !== undefined) assert.ok(!rm.wouldBurst('player', plain), `plain slot ${plain} silent`);
  // Enemy lands never burst.
  assert.ok(!rm.wouldBurst('enemy', Number(numStr)), 'enemy land silent');
  // Event path: a player BallLanded on the special slot produces a burst effect.
  rm.onEvents([{ event: 'ball_landed', side: 'player', number: Number(numStr) }]);
  assert.equal(rm.fxState().burst, col, 'burst color from engine slot color');
  // TTL decay removes it (data-driven lifetime).
  for (let i = 0; i < 60; i++) rm.update(1 / 60);
  assert.equal(rm.fxState().burst, null, 'burst expires');
  // Plain landing produces no burst.
  rm.onEvents([{ event: 'ball_landed', side: 'player', number: plain ?? 1 }]);
  assert.equal(rm.fxState().burst, null, 'plain land no burst');
});

ok('TASK-026: intent telegraph tracks engine enemy_intent state (REQ-002)', () => {
  const c = clientInCombat('fx2');
  const rm = new RenderManager(c, { no3d: true });
  rm.attachWheel(wheelOf(c));
  rm.syncIntent();
  const intentBefore = rm.fxState().intent;
  // Engine state drives the label verbatim (may be null pre-declaration).
  const engineIntent = c.state().battle?.enemy_intent ?? null;
  assert.equal(rm.fxState().intent, engineIntent, 'intent ring label = engine enemy_intent');
  if (intentBefore) {
    // Ring mesh present in the fx group.
    const ring = rm.fxGroup.group.children.find((m) => m.name === 'fx_intent');
    assert.ok(ring, 'intent ring mesh mounted');
    // intent_executed clears it.
    rm.onEvents([{ event: 'intent_executed' }]);
    assert.equal(rm.fxState().intent, null, 'intent_executed clears the telegraph');
  }
});

ok('TASK-026: fx burst colors derive ONLY from SLOT_PALETTE (visual consistency)', () => {
  const c = clientInCombat('fx3');
  const rm = new RenderManager(c, { no3d: true });
  const cfg = wheelOf(c);
  rm.attachWheel(cfg);
  const gold = Object.entries(cfg.slot_colors).find(([, col]) => col === 'gold');
  if (!gold) return; // config-dependent; classic has gold on 7
  rm.onEvents([{ event: 'ball_landed', side: 'player', number: Number(gold[0]) }]);
  assert.ok(rm.fx.burst, 'burst active');
  assert.equal(rm.fx.burst.mesh.material.color.getHex(), SLOT_PALETTE.gold, 'burst uses palette gold');
});

ok('TASK-027: render perf gate — full update loop sustains 33 ms/frame budget', () => {
  const c = clientInCombat('perf1');
  const rm = new RenderManager(c, { no3d: true });
  rm.attachWheel(wheelOf(c));
  rm.syncIntent();
  // Warm up JIT.
  for (let i = 0; i < 30; i++) rm.update(1 / 60);
  // Measure 120 frames (2 sim-seconds) of the full headless update path:
  // playback tick + dust drift + fx decay + (no-op) render.
  const N = 120;
  const t0 = performance.now();
  for (let i = 0; i < N; i++) rm.update(1 / 60);
  const perFrame = (performance.now() - t0) / N;
  // Budget: 30 ms per frame headless (browser adds GL cost but renders at
  // 480×270 → PS1 quad, well within SwiftShader's 33 ms TEST-006 reference).
  assert.ok(perFrame < 30, `per-frame update ${perFrame.toFixed(2)} ms < 30 ms budget`);
  // Scene size sanity: dressing + fx must not explode the graph.
  const g = rm.graph();
  assert.ok(g.meshes < 100, `scene stays lean (${g.meshes} meshes)`);
  assert.ok(g.dressing.chipInstances === 15 && g.dressing.croupiers === 4);
});

ok('TASK-027: telemetry playback throughput — 120-frame spin plays in <200 ms headless', () => {
  const c = clientInCombat('perf2');
  const rm = new RenderManager(c, { no3d: true });
  rm.attachWheel(wheelOf(c));
  c.placeBet('red', 1);
  c.spin();
  rm.playSpin(0, c.spinTelemetry(0));
  assert.ok(rm.playbackActive, 'playback armed');
  const t0 = performance.now();
  for (let i = 0; i < 100000 && rm.playbackActive; i++) rm.tick(1 / 60);
  const ms = performance.now() - t0;
  assert.ok(!rm.playbackActive, 'playback completes');
  assert.ok(ms < 200, `full spin playback ${ms.toFixed(1)} ms < 200 ms headless`);
});

console.log(`\nrender: ${passed} passed`);
if (failures.length) {
  process.exit(1);
}