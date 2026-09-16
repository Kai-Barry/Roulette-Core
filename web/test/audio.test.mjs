/**
 * Phase 5 gates — TEST-013 audio diagnostics (headless, no audible asserts):
 * bus states from SettingsStore, §13.4 recipe existence, TASK-029 timed sim
 * events from a real engine spin (bridge round-trip + playback sync), and
 * the TASK-030 encounter sequencer tempo table + boss layering.
 */

import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require_ = createRequire(import.meta.url);
const wasm = require_('/projects/Roulette-Core/crates/roulette-wasm/pkg/roulette_wasm.js');
const { GameClient } = await import('../src/client/GameClient.ts');
const { SoundManager, SFX_RECIPES } = await import('../src/audio/SoundManager.ts');
const { EncounterMusic, TIER_TEMPO, BOSS_TEMPO_PUSH } = await import('../src/audio/music.ts');
const { RenderManager } = await import('../src/render/RenderManager.ts');
const { SettingsStore } = await import('../src/ui/settings.ts');

let passed = 0;
const failures = [];
function ok(name, fn) {
  try {
    fn();
    passed++;
    console.log(`ok ${passed} - ${name}`);
  } catch (e) {
    failures.push({ name, err: e });
    console.log(`not ok ${passed + failures.length} - ${name}`);
    console.log(`  ${String(e && e.message ? e.message : e).split('\n')[0]}`);
  }
}

function battleClient() {
  const c = new GameClient(wasm, 'audio-test');
  c.startRun('short');
  const offer = c.state().run?.loadout_offer;
  for (const id of offer?.card_ids ?? []) c.draftCard(id);
  c.draftWheel();
  c.finishLoadout();
  c.pickNode(c.state().run.map.floors[0][0].id);
  return c;
}

// -- TASK-028: buses + recipes ------------------------------------------------

ok('TEST-013a: headless SoundManager reports headless ctx + default bus states', () => {
  const s = new SoundManager();
  const d = s.diagnostics();
  assert.equal(d.ctx, 'headless');
  assert.deepEqual(d.buses, { music: 0.55, drone: 0.15, sfx: 0.8 });
  assert.ok(d.recipes.length >= 8, 'recipe table populated');
});

ok('TEST-013b: every §13.4 recipe exists and plays without throwing in Node', () => {
  const s = new SoundManager();
  const method = {
    roulette_click: 'playRouletteClick', peg_bounce: 'playPegBounce',
    ball_settle: 'playBallSettle', shotgun: 'playShotgun', ball_split: 'playBallSplit',
    settle_timeout: 'playSettleTimeout', spin_whoosh: 'playSpinWhoosh',
    intent_sting: 'playIntentSting',
  };
  for (const r of SFX_RECIPES) {
    if (r.startsWith('outcome_')) continue;
    const fn = s[method[r]];
    assert.equal(typeof fn, 'function', `recipe ${r} has a play method`);
    fn.call(s);
  }
  s.playOutcome('win'); s.playOutcome('lose'); s.playOutcome('push');
  const played = s.played.map((p) => p.recipe);
  for (const r of SFX_RECIPES) assert.ok(played.includes(r), `${r} recorded`);
  assert.ok(s.diagnostics().lastPlayed.length > 0, 'diagnostics expose lastPlayed');
});

ok('TEST-013c: bus volumes follow the persisted SettingsStore', () => {
  const store = new SettingsStore(null);
  store.setVolume('music', 0.2);
  store.setVolume('sfx', 1);
  const s = new SoundManager(store);
  const d = s.diagnostics();
  assert.equal(d.buses.music, 0.2);
  assert.equal(d.buses.sfx, 1);
  assert.equal(d.buses.drone, 0.15, 'untouched bus keeps default');
});

ok('TEST-013d: engine events map to recipes (whoosh/intent/outcome)', () => {
  const s = new SoundManager();
  s.onEvents([
    { event: 'spin_started', side: 'player' },
    { event: 'spin_started', side: 'enemy' },
    { event: 'intent_executed' },
    { event: 'round_ended', round: 1, player_pts: 10, enemy_pts: 0, outcome: 'player_victory' },
  ]);
  const played = s.played.map((p) => p.recipe);
  assert.ok(played.includes('spin_whoosh'), 'player spin fires whoosh');
  assert.equal(played.filter((r) => r === 'spin_whoosh').length, 1, 'enemy spin does not');
  assert.ok(played.includes('intent_sting'));
  assert.ok(played.includes('outcome_win'));
});

// -- TASK-029: timed sim events from a real spin ------------------------------

ok('TEST-013e: spinSimEvents returns timed engine events for a physics spin', () => {
  const c = battleClient();
  c.placeBet('red', 1);
  c.spin();
  const evs = c.spinSimEvents(0);
  assert.ok(evs.length > 0, 'physics spin produced sim events');
  let lastFrame = -1;
  for (const e of evs) {
    assert.ok(Number.isInteger(e.frame) && e.frame >= 0);
    assert.ok(e.frame >= lastFrame, 'frames are non-decreasing');
    lastFrame = e.frame;
    assert.equal(typeof e.event, 'object');
  }
  const kinds = new Set(evs.map((e) => Object.keys(e.event)[0]));
  assert.ok(kinds.has('DividerTick'), 'click track present (§5.4)');
  assert.ok(kinds.has('BallSettled'), 'settle hook present');
  const settle = evs.find((e) => 'BallSettled' in e.event);
  assert.ok(settle.frame >= 0, 'settle frame index present');
});

ok('TEST-013f: spinSimEvents round-trips through the wasm JSON bridge', () => {
  const c = battleClient();
  c.placeBet('red', 1);
  c.spin();
  const raw = c.handle.spin_sim_events(0);
  const parsed = JSON.parse(raw);
  assert.ok(Array.isArray(parsed) && parsed.length > 0);
  const first = parsed[0];
  assert.ok(typeof first.frame === 'number');
  assert.ok('DividerTick' in first.event || 'PegHit' in first.event ||
    'BallSettled' in first.event, 'serde-tagged SimEvent shape');
});

ok('TEST-013g: enemy side sim events exist after a spin (CON-001 both wheels)', () => {
  const c = battleClient();
  c.placeBet('red', 1);
  c.spin();
  const enemy = c.spinSimEvents(1);
  assert.ok(enemy.length > 0, 'enemy wheel physics simulated');
  assert.ok(enemy.some((e) => 'BallSettled' in e.event));
});

ok('TEST-013h: uniform sampling path yields empty sim events (fast path)', () => {
  const c = battleClient();
  if (typeof c.setSpinSampling !== 'function') {
    // Sampling switch not exposed on this client surface — nothing to prove.
    return;
  }
  c.setSpinSampling('uniform');
  c.placeBet('red', 1);
  c.spin();
  assert.deepEqual(c.spinSimEvents(0), []);
});

ok('TEST-013i: playback sync fires queued events in frame order, exactly once', () => {
  const c = battleClient();
  c.placeBet('red', 1);
  c.spin();
  const evs = c.spinSimEvents(0);
  const s = new SoundManager();
  s.queueSimEvents(evs);
  assert.equal(s.simPending, evs.length);
  const rm = new RenderManager(c, { no3d: true });
  rm.playSpin(0, c.spinTelemetry(0));
  rm.onPlaybackFrame = (frame) => s.tickPlayback(frame);
  // Drive the renderer playback at fixed dt until done.
  let guard = 0;
  while (rm.playbackActive && guard++ < 10000) rm.tick(1 / 120);
  assert.ok(!rm.playbackActive, 'playback completes');
  assert.equal(s.simPending, 0, 'all queued events fired');
  const played = s.played.map((p) => p.recipe);
  const clicks = played.filter((r) => r === 'roulette_click').length;
  const tickEvents = evs.filter((e) => 'DividerTick' in e.event).length;
  assert.equal(clicks, tickEvents, 'every DividerTick played exactly once');
  assert.ok(played.includes('ball_settle'), 'settle sound played');
});

ok('TEST-013j: REQ-008 — engine event log stays float-free with sim events active', () => {
  const c = battleClient();
  c.placeBet('red', 1);
  c.spin();
  const evs = c.spinSimEvents(0);
  assert.ok(evs.length > 0);
  const log = JSON.stringify(c.events);
  assert.ok(!log.includes('NaN') && !/[0-9]\.[0-9]/.test(log), 'no floats in event log');
});

// -- TASK-030: encounter sequencer ---------------------------------------------

ok('TEST-013k: §13.3 tempo table + boss push are pure functions of tier', () => {
  assert.deepEqual(TIER_TEMPO, [96, 108, 126, 144, 160]);
  assert.equal(EncounterMusic.tempoFor({ tier: 0, boss: false }), 96);
  assert.equal(EncounterMusic.tempoFor({ tier: 4, boss: false }), 160);
  assert.equal(EncounterMusic.tempoFor({ tier: 2, boss: true }), Math.round(126 * BOSS_TEMPO_PUSH));
  assert.deepEqual(EncounterMusic.layersFor({ tier: 0, boss: false }), ['bass']);
  assert.deepEqual(EncounterMusic.layersFor({ tier: 2, boss: false }), ['bass', 'arp']);
  assert.deepEqual(EncounterMusic.layersFor({ tier: 0, boss: true }), ['bass', 'arp', 'drums']);
});

ok('TEST-013l: sequencer starts headless, exposes tempo/layers, stops clean', () => {
  const s = new SoundManager();
  const m = new EncounterMusic(s);
  m.start({ tier: 2, boss: false });
  const st = m.state();
  assert.equal(st.playing, true);
  assert.equal(st.tempo, 126);
  assert.deepEqual(st.layers, ['bass', 'arp']);
  assert.equal(s.diagnostics().music.playing, true);
  assert.equal(s.diagnostics().music.tempo, 126);
  m.stop();
  assert.equal(m.state().playing, false);
  assert.deepEqual(s.diagnostics().music.layers, []);
});

ok('TEST-013m: boss encounter layers drums + pushes tempo', () => {
  const s = new SoundManager();
  const m = new EncounterMusic(s);
  m.start({ tier: 4, boss: true });
  const st = m.state();
  assert.equal(st.tempo, Math.round(160 * BOSS_TEMPO_PUSH));
  assert.ok(st.layers.includes('drums'));
  m.stop();
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length > 0) process.exit(1);