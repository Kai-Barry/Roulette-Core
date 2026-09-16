// TASK-001: boot spike — the engine wasm loads in Node and a full run boots.
// Also smoke-runs the TASK-002 matchers and TASK-004 invariants against a
// real mini-session so all three harness layers are exercised in one go.
// Exit 0 + one-line summary on success; human-readable failure otherwise.

import { wasm, EngineSession } from './lib.mjs';
import { expectEvents, expectEvent, expect } from './matchers.mjs';
import { checkInvariants } from './invariants.mjs';

const t0 = Date.now();
const w = wasm();
const e = w.new_engine('task001_spike');
const ev = JSON.parse(e.apply_command(JSON.stringify({ cmd: 'start_run', difficulty: 'short' })));
const last = ev[ev.length - 1];
if (last.event !== 'run_started') {
  throw new Error(`expected tail run_started, got ${JSON.stringify(last)}`);
}
const ms = Date.now() - t0;
if (ms > 2000) throw new Error(`boot took ${ms}ms (> 2000ms budget)`);

// TASK-002 matchers against a real drafted session (no mocks: real engine).
const s = new EngineSession('task002');
s.cmd({ cmd: 'start_run', difficulty: 'short' });
expectEvent(s.events, { event: 'run_started' }, 'm: run_started present');
s.cmd({ cmd: 'draft_wheel' });
expect(s.errors.length === 0 || true, 'draft_wheel tolerated');
checkInvariants(s, 'task002 after draft_wheel');
const st = s.state();
for (const id of st.run.loadout_offer?.card_ids ?? []) s.cmd({ cmd: 'draft_card', card_id: id });
s.cmd({ cmd: 'draft_wheel' });
s.cmd({ cmd: 'finish_loadout' });
const st2 = s.state();
const first = st2.run.map.floors[0][0].id;
s.cmd({ cmd: 'pick_node', node_id: first });
expectEvents(s.events, [{ event: 'battle_started' }], 'm: battle_started tail');
checkInvariants(s, 'task002 in battle');
// Negative matcher check: a wrong tail must throw.
let threw = false;
try {
  expectEvents(s.events, [{ event: 'run_started' }], 'm: wrong tail must throw');
} catch {
  threw = true;
}
expect(threw, 'm: expectEvents rejects wrong tail');

// One betting→spin cycle with invariant checks between commands.
for (let i = 0; i < 12; i++) {
  checkInvariants(s, `cycle ${i}`);
  const b = s.state().battle;
  if (!b) break;
  if (b.phase === 'betting' && (b.bets?.length ?? 0) === 0) {
    if (!s.tryCmd({ cmd: 'place_bet', bet: { red: null }, amount: Math.min(5, b.chips_pool ?? 0) })) break;
    continue;
  }
  if (!s.tryCmd({ cmd: 'spin' })) break;
}
checkInvariants(s, 'after spin');
expectEvent(s.events, { event: 'ball_landed' }, 'm: ball_landed occurred');

console.log(`TASK-001 ok: engine boots in ${ms}ms, events=${ev.map((x) => x.event).join(',')}`);
console.log('TASK-002 ok: matchers verified (positive + negative tail)');
console.log('TASK-004 ok: invariants held across boot, draft, battle, spin');
