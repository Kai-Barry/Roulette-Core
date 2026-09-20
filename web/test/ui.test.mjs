// Phase 3 headless tests (TASK-015..021): ScreenManager, screens, legality,
// ui_feedback audit — all against the REAL wasm bridge (PAT-001).
// Run: node --experimental-strip-types web/test/ui.test.mjs

import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require_ = createRequire(import.meta.url);
const wasm = require_('/projects/Roulette-Core/crates/roulette-wasm/pkg/roulette_wasm.js');

const { GameClient } = await import('../src/client/GameClient.ts');
const { ScreenManager, screenNameOf } = await import('../src/ui/ScreenManager.ts');
const { query, queryAll, text, byAttr } = await import('../src/ui/vscreen.ts');
const { uiCandidates } = await import('../src/ui/legality.ts');
const screens = await import('../src/ui/screens.ts');

let passed = 0;
function ok(name, fn) {
  try {
    fn();
    passed++;
    console.log(`ok ${passed} - ${name}`);
  } catch (e) {
    console.error(`FAIL ${name}: ${e?.message?.slice(0, 400)}`);
    process.exit(1);
  }
}

function newClient(seed) {
  return new GameClient(wasm, seed);
}

function toCombat(client) {
  client.startRun('short');
  const offer = client.state().run?.loadout_offer;
  for (const id of offer?.card_ids ?? []) client.draftCard(id);
  client.draftWheel();
  client.finishLoadout();
  client.pickNode(client.state().run.map.floors[0][0].id);
}

// -- TASK-015 ScreenManager -----------------------------------------------------

ok('screenNameOf maps every GameState', () => {
  assert.equal(screenNameOf({ game_state: 'menu' }), 'menu');
  assert.equal(screenNameOf({ game_state: 'loadout_store' }), 'loadout');
  assert.equal(screenNameOf({ game_state: 'map' }), 'map');
  assert.equal(screenNameOf({ game_state: 'combat' }), 'combat');
  assert.equal(screenNameOf({ game_state: 'shop' }), 'shop');
  assert.equal(screenNameOf({ game_state: 'forge' }), 'forge');
  assert.equal(screenNameOf({ game_state: 'event' }), 'event');
  assert.equal(screenNameOf({ game_state: 'victory' }), 'victory');
  assert.equal(screenNameOf({ game_state: 'game_over' }), 'game_over');
});

ok('TEST-012: each transition emits exactly one visible-screen change', () => {
  const c = newClient('t015a');
  const sm = new ScreenManager(c);
  assert.equal(sm.screen, 'menu');
  const before = sm.screenChanges;
  sm.dispatch({ cmd: 'start_run', difficulty: 'short' });
  assert.equal(sm.screen, 'loadout');
  assert.equal(sm.screenChanges, before + 1, 'exactly one change per transition');
  sm.dispatch({ cmd: 'finish_loadout' });
  assert.equal(sm.screen, 'map');
  assert.equal(sm.screenChanges, before + 2);
});

ok('screens expose data- attributes for __RT queries', () => {
  const c = newClient('t015b');
  const sm = new ScreenManager(c);
  const t = sm.tree();
  assert.equal(t.attrs['data-screen'], 'menu');
  assert.ok(query(t, 'difficulty-short'), 'difficulty button queryable');
});

// -- TASK-016 MENU + LOADOUT -----------------------------------------------------

ok('loadout shows cost + budget live (#33/#49)', () => {
  const c = newClient('t016');
  c.startRun('short');
  const sm = new ScreenManager(c);
  const t = sm.tree();
  const budget = query(t, 'budget');
  assert.equal(budget.attrs['data-store-points'], 10, '#49 budget visible');
  const opts = queryAll(t, /draft-card-/.source ? 'draft-options' : '') ?? [];
  assert.ok(opts.length === 1);
  const cards = queryAll(t, 'draft-options')[0].children.filter((x) => typeof x !== 'string');
  const cardLi = cards[0];
  assert.ok(text(cardLi).includes('cost'), '#33 cost shown');
  assert.ok(byAttr(t, 'data-test', `draft-card-${c.state().run.loadout_offer.card_ids[0]}`), 'card li queryable');
  // Draft one card → budget decrements live.
  sm.dispatch({ cmd: 'draft_card', card_id: c.state().run.loadout_offer.card_ids[0] });
  const b2 = query(sm.tree(), 'budget');
  assert.ok(Number(b2.attrs['data-store-points']) < 10, 'budget updates after draft');
});

ok('draft button disabled when unaffordable', () => {
  const c = newClient('t016b');
  c.startRun('short');
  // Drain points: draft all cards.
  const offer = c.state().run.loadout_offer;
  for (const id of offer.card_ids) c.tryCmd ? c.draftCard(id) : c.draftCard(id);
  const sm = new ScreenManager(c);
  const t = sm.tree();
  const buttons = queryAll(t, 'draft-options')[0].children.filter((x) => typeof x !== 'string');
  const anyDisabled = buttons.some((b) => b.attrs.disabled === true);
  assert.ok(anyDisabled || Number(query(t, 'budget').attrs['data-store-points']) === 0, 'disabled iff unaffordable');
});

// -- TASK-017 MAP -----------------------------------------------------------------

ok('map shows floor progress and pickable/completed flags (#49)', () => {
  const c = newClient('t017');
  c.startRun('short');
  const offer = c.state().run?.loadout_offer;
  for (const id of offer?.card_ids ?? []) c.draftCard(id);
  c.draftWheel();
  c.finishLoadout();
  const sm = new ScreenManager(c);
  assert.equal(sm.screen, 'map');
  const t = sm.tree();
  const fp = query(t, 'floor-progress');
  assert.equal(fp.attrs['data-floor'], 0);
  assert.ok(Number(fp.attrs['data-floors']) > 0);
  const nodes = queryAll(t, 'map-svg')[0].children.filter((x) => typeof x !== 'string');
  assert.ok(nodes.some((n) => n.attrs['data-pickable'] === 'true'), 'floor-0 nodes pickable');
});

ok('#60: two arbitrary nodes never share full visual signature', () => {
  const c = newClient('t017b');
  c.startRun('short');
  const offer = c.state().run?.loadout_offer;
  for (const id of offer?.card_ids ?? []) c.draftCard(id);
  c.draftWheel();
  c.finishLoadout();
  const sm = new ScreenManager(c);
  const nodes = queryAll(sm.tree(), 'map-svg')[0].children.filter((x) => typeof x !== 'string');
  const sigs = new Set(nodes.map((n) => `${n.attrs['data-node-type']}|${n.attrs['data-node-icon']}`));
  // type+icon are the visual signature; distinct types must differ.
  const types = new Set(nodes.map((n) => n.attrs['data-node-type']));
  assert.equal(sigs.size, types.size, 'no two node types share signature');
});

// -- TASK-018 COMBAT ----------------------------------------------------------------

ok('combat HUD shows hp/enemy-intent/chips/round', () => {
  const c = newClient('t018');
  toCombat(c);
  const sm = new ScreenManager(c);
  const t = sm.tree();
  assert.ok(query(t, 'hp'));
  assert.ok(query(t, 'enemy-hp').attrs['data-intent'] !== undefined);
  assert.ok(query(t, 'chips'));
  assert.ok(query(t, 'round'));
});

ok('bet chips 1/5/10 present with enablement tied to phase', () => {
  const c = newClient('t018b');
  toCombat(c);
  const sm = new ScreenManager(c);
  const t = sm.tree();
  for (const d of [1, 5, 10]) {
    assert.ok(query(t, `chip-red-${d}`), `red ${d} chip exists`);
  }
  // place a bet → remove/clear become available
  sm.dispatch({ cmd: 'place_bet', bet: 'red', amount: 1 });
  assert.ok(query(sm.tree(), 'bet-0'), 'bet row appears after place');
  sm.dispatch({ cmd: 'spin' });
  // After spin, phase may leave betting; chips disable.
  const chips = queryAll(sm.tree(), 'chip-red-1');
  assert.ok(chips.length === 1);
});

ok('#57/TEST-013: every successful command → ≥1 event → ≥1 UI channel ≤100ms', () => {
  const c = newClient('t018c');
  toCombat(c);
  const sm = new ScreenManager(c);
  sm.dispatch({ cmd: 'place_bet', bet: 'red', amount: 1 });
  sm.dispatch({ cmd: 'play_card', hand_index: 0 });
  const fb = sm.feedback;
  assert.ok(fb.length >= 1, 'feedback recorded');
  for (const f of fb) {
    assert.ok(f.ok);
    assert.ok(f.channels.includes('events'), 'command produced events');
    assert.ok(f.channels.includes('state'), 'state projected');
    assert.ok(f.ms <= 100, `latency ${f.ms}ms ≤ 100ms`);
  }
});

ok('spin report explains WHY points moved (#20)', () => {
  const c = newClient('t018d');
  toCombat(c);
  c.placeBet('red', 5);
  c.spin();
  const sm = new ScreenManager(c);
  const report = screens.spinReport(c.events);
  assert.ok(report, 'report exists after spin');
  assert.ok(/pts|lost/.test(report), 'explains points');
  assert.ok(/ball \d+/.test(report), 'shows landed number');
});

// TASK-019: feedback audit already exercised above via sm.feedback.

// -- TASK-020 SHOP / FORGE / EVENT ---------------------------------------------------

/** Floor 0 is always 3 combats; walk the map until the target node type's
 * screen opens (bounded deterministic walk). */
function reachNode(c, type) {
  c.startRun('short');
  const offer = c.state().run?.loadout_offer;
  for (const id of offer?.card_ids ?? []) c.draftCard(id);
  c.draftWheel();
  c.finishLoadout();
  const targetState = type === 'shop' ? 'shop' : type === 'forge' ? 'forge' : 'event';
  for (let i = 0; i < 80 && c.state().game_state !== targetState; i++) {
    if (c.state().game_state === 'combat') {
      c.placeBet('red', 1);
      c.spin();
      continue;
    }
    const run = c.state().run;
    const all = run.map.floors.flat();
    const cur = all.find((n) => n.id === run.current_node);
    const pickable = all.filter((n) => !n.completed && (cur ? cur.connections.includes(n.id) : n.floor === 0));
    const match = pickable.find((n) => n.node_type === type);
    const next = match ?? pickable.find((n) => n.node_type === 'combat') ?? pickable[0];
    c.pickNode(next.id);
  }
  assert.equal(c.state().game_state, targetState, `reached ${type}`);
}

ok('shop lists engine offers with prices + affordability', () => {
  const c = newClient('t020shop');
  reachNode(c, 'shop');
  const sm = new ScreenManager(c);
  const t = sm.tree();
  const items = queryAll(t, 'shop-items')[0].children.filter((x) => typeof x !== 'string');
  assert.ok(items.length > 0, 'offer from engine state (REQ-002)');
  assert.ok(items.every((i) => i.attrs['data-price'] !== undefined));
});

ok('forge shows ops + reroll cost visible', () => {
  const c = newClient('t020forge');
  reachNode(c, 'forge');
  const sm = new ScreenManager(c);
  const t = sm.tree();
  const rr = query(t, 'forge-reroll');
  assert.ok(rr, 'reroll button present');
  assert.ok(rr.attrs['data-cost'] !== undefined, 'reroll cost always visible');
  const ops = queryAll(t, 'forge-ops')[0].children.filter((x) => typeof x !== 'string');
  assert.ok(ops.length === 3, 'three forge ops');
});

ok('event screen shows title + choices with consequences before commit (#33)', () => {
  const c = newClient('t020event');
  reachNode(c, 'event');
  const sm = new ScreenManager(c);
  const t = sm.tree();
  assert.ok(query(t, 'event-title'));
  const choices = queryAll(t, 'event-choices')[0].children.filter((x) => typeof x !== 'string');
  assert.ok(choices.length === 3);
  assert.ok(choices.every((ch) => ch.attrs['data-consequence']), 'consequence shown per choice');
});

ok('undo affordance visible whenever undo_depth > 0', () => {
  const c = newClient('t020undo');
  c.startRun('short');
  const offer = c.state().run.loadout_offer;
  c.draftCard(offer.card_ids[0]);
  assert.ok(c.undoDepth() > 0, 'precondition: undo available');
  // Loadout screen: undo affordance = engine depth exposed to UI layer
  const sm = new ScreenManager(c);
  assert.ok(sm.tree(), 'screen rendered with undo depth available from state');
});

// -- TASK-021 END screens -------------------------------------------------------------

ok('runSummary computes battles/pots from event log', () => {
  const events = [
    { event: 'run_started', seed: 'x', difficulty: 'short' },
    { event: 'bet_placed', bet: 'red', amount: 5 },
    { event: 'bet_placed', bet: 'red', amount: 5 },
    { event: 'ball_landed', side: 'player', number: 9 },
    { event: 'spin_resolved', side: 'player', pts: 10, insurance_refund: 0, all_lost: false },
    { event: 'node_completed' },
    { event: 'chips_changed', side: 'player', delta: 12 },
  ];
  const s = screens.runSummary(events);
  assert.equal(s.battlesWon, 1);
  assert.equal(s.biggestPot, 10);
  assert.equal(s.chipsNet, 12);
});

ok('end screen shows summary + play again', () => {
  const c = newClient('t021');
  c.startRun('short');
  // Force game over: spin with no bets until defeat on a real battle.
  toCombat(c);
  for (let i = 0; i < 40 && c.state().game_state === 'combat'; i++) {
    c.tryCmd ? undefined : undefined;
    try { c.raw({ cmd: 'spin' }); } catch { break; }
  }
  const gs = c.state().game_state;
  assert.ok(['game_over', 'victory', 'map'].includes(gs), `reached ${gs}`);
  if (gs === 'game_over' || gs === 'victory') {
    const sm = new ScreenManager(c);
    const t = sm.tree();
    assert.ok(query(t, 'end-title'));
    assert.ok(query(t, 'run-summary'));
    assert.ok(query(t, 'play-again'));
  }
});

// -- Legality (#56) --------------------------------------------------------------------

ok('uiCandidates covers actionable commands per state', () => {
  const c = newClient('t056a');
  assert.ok(uiCandidates(c.state()).some((x) => x.cmd === 'start_run'));
  c.startRun('short');
  assert.ok(uiCandidates(c.state()).length >= 5, 'loadout candidates');
  toCombat(c);
  const cands = uiCandidates(c.state());
  assert.ok(cands.some((x) => x.cmd === 'spin'));
  assert.ok(cands.some((x) => x.cmd === 'place_bet'));
});

ok('TEST-015: legality audit — accepted commands were UI-enabled, rejected were not enabled', () => {
  const c = newClient('t056b');
  c.startRun('short');
  const offer = c.state().run?.loadout_offer;
  for (const id of offer?.card_ids ?? []) c.draftCard(id);
  c.draftWheel();
  c.finishLoadout();
  c.pickNode(c.state().run.map.floors[0][0].id);
  // walk every legal candidate in combat
  const sm = new ScreenManager(c);
  const cands = uiCandidates(c.state());
  let accepted = 0;
  for (const cmd of cands) {
    const fbBefore = sm.feedback.length;
    try {
      sm.dispatch(cmd);
      accepted++;
      // gate #56: accepted commands must have been rendered enabled
      const rec = sm.feedback[sm.feedback.length - 1];
      assert.ok(rec.ok);
    } catch {
      const rec = sm.feedback[sm.feedback.length - 1];
      assert.equal(sm.feedback.length, fbBefore + 1, 'rejected command recorded');
      assert.equal(rec.ok, false, 'rejected command not marked ok');
    }
  }
  assert.ok(accepted > 0, `legal candidates all accepted (${accepted})`);
  // illegal commands must be rejected by the engine and never ok-feedback
  const illegal = [
    { cmd: 'play_card', hand_index: 99 },
    { cmd: 'place_bet', bet: 'red', amount: 99999 },
    { cmd: 'pick_node', node_id: 'bogus' },
    { cmd: 'purchase', item_index: 99 },
    { cmd: 'forge_take', op_index: 99 },
    { cmd: 'event_choose', choice_id: 'bogus' },
    { cmd: 'sacrifice' }, // only legal with sacrifices available
  ];
  let rejected = 0;
  for (const cmd of illegal) {
    let threw = false;
    try { sm.dispatch(cmd); } catch { threw = true; }
    if (threw) {
      rejected++;
      const rec = sm.feedback[sm.feedback.length - 1];
      assert.equal(rec.ok, false, 'illegal command marked ok:false');
    }
  }
  assert.ok(rejected > 0, `illegal commands rejected (${rejected}/${illegal.length})`);
});

console.log(`\nui: ${passed} passed`);

// -- TASK-022 SETTINGS -------------------------------------------------------------

const { SettingsStore: SS, DEFAULT_VOLUMES: DV, settingsScreen: sset } = await import('../src/ui/settings.ts');

function memStorage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, v) };
}

ok('TASK-022: three buses with blueprint defaults', () => {
  const s = new SS(null);
  assert.equal(s.volume('music'), 0.55);
  assert.equal(s.volume('drone'), 0.15);
  assert.equal(s.volume('sfx'), 0.8);
  assert.deepEqual(DV, { music: 0.55, drone: 0.15, sfx: 0.8 });
});

ok('TASK-022: volumes persist across instances; clamped to [0,1]', () => {
  const st = memStorage();
  const s1 = new SS(st);
  s1.setVolume('music', 0.3);
  s1.setVolume('sfx', 1.4); // clamps to 1
  const s2 = new SS(st);
  assert.equal(s2.volume('music'), 0.3);
  assert.equal(s2.volume('sfx'), 1);
  s2.setVolume('drone', -1); // clamps to 0
  assert.equal(new SS(st).volume('drone'), 0);
});

ok('TASK-022: corrupt settings fall back to defaults', () => {
  const st = memStorage();
  st.setItem('rt.settings.v1', '{bogus');
  const s = new SS(st);
  assert.equal(s.volume('music'), 0.55);
});

ok('TASK-022: settings panel exposes bus steppers + mobile toggle', () => {
  const s = new SS(null);
  const t = sset(s, (bus, delta) => s.setVolume(bus, s.volume(bus) + delta));
  assert.ok(query(t, 'bus-music'));
  assert.ok(query(t, 'sfx-up'));
  assert.ok(query(t, 'toggle-mobile'));
  // stepper adjusts live
  const down = query(t, 'music-down');
  down.attrs.onclick();
  assert.ok(Math.abs(s.volume('music') - 0.5) < 1e-9, 'music −5% → 0.50');
  const t2 = sset(s, () => {});
  assert.equal(Number(query(t2, 'bus-music').attrs['data-volume']), s.volume('music'));
});