// TASK-012/013/014 headless tests: GameClient, projections, persistence
// against the REAL wasm bridge (PAT-001 — no mocks, fixtures from engine).
// Run: node --experimental-strip-types web/test/client-core.test.mjs

import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require_ = createRequire(import.meta.url);
const wasm = require_('/projects/Roulette-Core/crates/roulette-wasm/pkg/roulette_wasm.js');

const { GameClient, EngineError } = await import('../src/client/GameClient.ts');
const proj = await import('../src/client/projections.ts');
const { Persistence, memoryStorage, stateHash } = await import('../src/client/persistence.ts');
const { boot, makeRt } = await import('../src/main.ts');

let passed = 0;
function ok(name, fn) {
  try {
    fn();
    passed++;
    console.log(`ok ${passed} - ${name}`);
  } catch (e) {
    console.error(`FAIL ${name}: ${e?.message}`);
    process.exit(1);
  }
}

function toCombat(client) {
  client.startRun('short');
  const offer = client.state().run?.loadout_offer;
  for (const id of offer?.card_ids ?? []) client.draftCard(id);
  client.draftWheel();
  client.finishLoadout();
  client.pickNode(client.state().run.map.floors[0][0].id);
}

// -- TASK-011 GameClient ------------------------------------------------------

ok('GameClient boots and mirrors state', () => {
  const c = new GameClient(wasm, 'task011');
  assert.equal(c.seed, 'task011');
  assert.equal(c.state().game_state, 'menu');
  assert.equal(c.state().undo_depth, 0);
});

ok('GameClient emits stateChanged + events to subscribers', () => {
  const c = new GameClient(wasm, 'task011b');
  const seen = [];
  c.on((ev) => seen.push(ev.kind));
  c.startRun('short');
  assert.ok(seen.includes('events'), 'events emitted');
  assert.ok(seen.includes('stateChanged'), 'stateChanged emitted');
});

ok('GameClient mirrors Command enum method-for-method', () => {
  const cmds = ['startRun', 'draftCard', 'draftWheel', 'finishLoadout', 'pickNode', 'playCard',
    'buyDraw', 'placeBet', 'removeBet', 'clearBets', 'rebet', 'sacrifice', 'predict', 'spin',
    'purchase', 'forgeTake', 'forgeReroll', 'eventChoose', 'buyLevel', 'customize'];
  for (const m of cmds) assert.equal(typeof GameClient.prototype[m], 'function', m);
});

ok('engine errors throw typed EngineError with verbatim payload (SEC-002)', () => {
  const c = new GameClient(wasm, 'task011c');
  try {
    c.pickNode('f0l0'); // illegal in menu state
    assert.fail('should have thrown');
  } catch (e) {
    assert.ok(e instanceof EngineError);
    assert.equal(e.payload.error, true);
    assert.ok(typeof e.payload.kind === 'string');
  }
});

ok('undo depth grows on draft and undo() restores (REQ-009)', () => {
  const c = new GameClient(wasm, 'task011d');
  c.startRun('short');
  assert.equal(c.undoDepth(), 0, 'start_run is not undoable by design');
  const offer = c.state().run.loadout_offer;
  const ptsBefore = c.state().run.store_points;
  c.draftCard(offer.card_ids[0]);
  assert.ok(c.undoDepth() > 0);
  c.undo();
  assert.equal(c.state().run.store_points, ptsBefore, 'points restored after undo');
  assert.equal(c.undoDepth(), 0);
});

ok('spin telemetry side channel present after a spin', () => {
  const c = new GameClient(wasm, 'task011e');
  toCombat(c);
  c.placeBet({ number: 5 }, 1);
  c.spin();
  const t = c.spinTelemetry(0);
  assert.ok(t.byteLength > 16, 'telemetry bytes exist');
  const dv = new DataView(t.buffer, t.byteOffset, t.byteLength);
  const frames = dv.getUint32(0, true);
  assert.ok(frames > 100, `got ${frames} frames`);
});

// -- TASK-012 projections (fixtures from real engine) -------------------------

const combatFixture = (() => {
  const c = new GameClient(wasm, 'task012');
  toCombat(c);
  return c.state();
})();

ok('screenOf maps game_state to screens', () => {
  assert.equal(proj.screenOf({ game_state: 'menu' }), 'menu');
  assert.equal(proj.screenOf({ game_state: 'loadout_store' }), 'loadout');
  assert.equal(proj.screenOf({ game_state: 'game_over' }), 'game_over');
});

ok('hud projects run vitals', () => {
  const h = proj.hud(combatFixture);
  assert.ok(h.maxHp > 0, `hp ${h.maxHp}`);
  assert.equal(h.difficulty, 'short');
});

ok('mapView marks pickable uncompleted nodes on current floor', () => {
  const c = new GameClient(wasm, 'task012map');
  c.startRun('short');
  const st = c.state();
  const mv = proj.mapView(st);
  assert.ok(mv.floors > 0);
  const pickable = mv.nodes.filter((n) => n.pickable);
  assert.ok(pickable.length > 0, 'some nodes pickable at floor 0');
  assert.ok(pickable.every((n) => !n.completed));
});

ok('combatView exposes phase/hand/bets/wheels', () => {
  const c = new GameClient(wasm, 'task012c');
  toCombat(c);
  const v = proj.combatView(c.state());
  assert.equal(v.phase, 'betting');
  assert.ok(v.maxRounds > 0);
  assert.ok(v.playerWheel.numbers.length >= 2);
  assert.ok(v.enemyWheel.numbers.length >= 2);
  assert.ok(v.canBet && v.canSpin);
});

ok('slotColorOf resolves colors from engine wheel data', () => {
  const wheel = combatFixture.battle.player_wheel;
  const zero = wheel.numbers[0];
  assert.ok(['red', 'black', 'green', 'gold', 'purple', 'cyan', 'crimson'].includes(proj.slotColorOf(wheel, zero)));
});

ok('loadoutView projects the real loadout offer', () => {
  const c = new GameClient(wasm, 'task012l');
  c.startRun('short');
  const v = proj.loadoutView(c.state());
  assert.ok(v.cardIds.length > 0);
  assert.ok(v.wheelId.length > 0);
  assert.equal(v.storePoints, 10);
});

// -- TASK-013 persistence + replay --------------------------------------------

ok('autosave on events; resume replays log and matches state hash (REQ-006)', () => {
  const storage = memoryStorage();
  const p = new Persistence(wasm, storage);

  const c1 = new GameClient(wasm, 'task013');
  c1.on(() => p.save(c1));
  toCombat(c1);
  c1.placeBet({ number: 5 }, 1);
  c1.spin();

  const rec = p.load();
  assert.ok(rec, 'save exists');
  assert.ok(rec.commands.length > 5);

  const resumed = p.resume();
  assert.ok(resumed, 'resume succeeds');
  assert.equal(stateHash(resumed.client.state()), stateHash(rec.state), 'state hash matches after replay');
  assert.equal(resumed.client.state().game_state, 'combat');
});

ok('resume is deterministic: two replays from same log agree bitwise', () => {
  const storage = memoryStorage();
  const p = new Persistence(wasm, storage);
  const c1 = new GameClient(wasm, 'task013b');
  c1.on(() => p.save(c1));
  toCombat(c1);
  const a = p.resume();
  const b = p.resume();
  assert.equal(stateHash(a.client.state()), stateHash(b.client.state()));
});

ok('corrupt snapshot falls back to null (clean boot path)', () => {
  const storage = memoryStorage();
  storage.setItem('roulette-os.save.v1', '{not json');
  const p = new Persistence(wasm, storage);
  assert.equal(p.load(), null);
  assert.equal(p.resume(), null);
});

ok('mismatched replay hash refuses resume', () => {
  const storage = memoryStorage();
  const p = new Persistence(wasm, storage);
  const c1 = new GameClient(wasm, 'task013c');
  c1.on(() => p.save(c1));
  toCombat(c1);
  const rec = p.load();
  rec.state.run.chips = 99999; // tamper
  storage.setItem('roulette-os.save.v1', JSON.stringify(rec));
  assert.equal(p.resume(), null, 'hash mismatch detected');
});

// -- TASK-014 headless shell ---------------------------------------------------

ok('boot() returns __RT-shaped api and drives a run', () => {
  const rt = boot({ seed: 'task014', wasm });
  assert.equal(typeof rt.state(), 'object');
  assert.equal(typeof rt.cmd, 'function');
  assert.equal(typeof rt.events, 'function');
  rt.cmd({ cmd: 'start_run', difficulty: 'short' });
  assert.equal(rt.state().game_state, 'loadout_store');
  assert.ok(rt.events(5).length <= 5);
  assert.ok(rt.stateHash().length > 0);
});

ok('rt.run(policy) completes a full run via legal commands', () => {
  const rt = boot({ seed: 'task014b', wasm });
  const out = rt.run(() => null, 1); // policy returns null — only bootstraps
  assert.equal(out.commands, 1);
});

console.log(`\nclient-core: ${passed} passed`);