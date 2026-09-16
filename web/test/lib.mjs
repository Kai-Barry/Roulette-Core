// Shared headless-test infrastructure (PLAN Phase 0, TASK-001/002/004).
// AI-playability contract: everything here runs in Node with zero rendering;
// assertions consume engine event JSON only (REQ-003, PAT-002).
import { createRequire } from 'node:module';

const require_ = createRequire(import.meta.url);

export const WASM_PKG = '/projects/Roulette-Core/crates/roulette-wasm/pkg/roulette_wasm.js';

let _wasm = null;
export function wasm() {
  if (!_wasm) _wasm = require_(WASM_PKG);
  return _wasm;
}

export class EngineSession {
  constructor(seed = 'session') {
    this.handle = wasm().new_engine(seed);
    this.seed = seed;
    this.events = []; // full EngineEvent log (JSON objects)
    this.commands = []; // full command log (JSON objects) — replayable per REQ-006
    this.errors = [];
  }

  state() {
    return JSON.parse(this.handle.get_state_json());
  }

  /** Apply one command object. Returns event array; throws EngineErrorJson on failure. */
  cmd(c) {
    this.commands.push(c);
    let ev;
    try {
      ev = JSON.parse(this.handle.apply_command(JSON.stringify(c)));
    } catch (msg) {
      const err = safeParse(msg);
      this.errors.push({ command: c, error: err ?? String(msg) });
      const e = new Error('engine error: ' + String(msg));
      e.engine = err ?? null;
      throw e;
    }
    this.events.push(...ev);
    return ev;
  }

  /** Apply, swallowing errors (policy-bot probing; engine rolls back state). */
  tryCmd(c) {
    try {
      return this.cmd(c);
    } catch {
      return null;
    }
  }

  undoDepth() {
    return this.handle.undo_depth();
  }
}

function safeParse(s) {
  try {
    return JSON.parse(s);
  } catch {
    return null;
  }
}

export const NON_TERMINAL_STATES = ['loadout_store', 'map', 'combat', 'shop', 'event', 'forge'];

// -- Candidate command generator (TASK-015 legality audit + policy bots) ----
// Enumerates plausible Commands per engine GameState. The engine is the sole
// legality authority (REQ-001): illegal candidates just error and roll back.

export function candidatesFor(state) {
  const out = [];
  const gs = state.game_state;
  const run = state.run ?? {};
  const battle = state.battle ?? {};

  if (gs === 'loadout_store') {
    const offer = run.loadout_offer;
    if (offer) {
      for (const id of offer.card_ids) out.push({ cmd: 'draft_card', card_id: id });
      out.push({ cmd: 'draft_wheel' });
    }
    out.push({ cmd: 'finish_loadout' });
  } else if (gs === 'map') {
    for (const n of mapCandidates(state)) out.push({ cmd: 'pick_node', node_id: n.id });
  } else if (gs === 'combat') {
    for (let i = 0; i < (battle.hand?.length ?? 0); i++) out.push({ cmd: 'play_card', hand_index: i });
    if ((battle.bets?.length ?? 0) === 0) {
      const amt = Math.min(5, battle.chips_pool ?? 0);
      if (amt > 0) {
        // Alternate red/black by round parity: with mirrored wheels both
        // sides betting the same type can tie forever in sudden death.
        const type = (battle.round ?? 0) % 2 === 0 ? { red: null } : { black: null };
        out.push({ cmd: 'place_bet', bet: type, amount: amt });
        out.push({ cmd: 'place_bet', bet: { green: null }, amount: amt });
        out.push({ cmd: 'place_bet', bet: { odd: null }, amount: amt });
        out.push({ cmd: 'place_bet', bet: { even: null }, amount: amt });
      }
    }
    out.push({ cmd: 'spin' });
    if (battle.bets?.length) {
      const b0 = battle.bets[0];
      out.push({ cmd: 'remove_bet', bet: b0.bet, amount: b0.amount });
      out.push({ cmd: 'clear_bets' });
    }
  } else if (gs === 'shop') {
    const items = run.shop_offer?.items ?? run.shop_offer ?? [];
    const n = Array.isArray(items) ? items.length : 0;
    for (let i = 0; i < n; i++) out.push({ cmd: 'purchase', item_index: i });
    // Exit a node screen by picking the next node on the path (CLI `pick`).
    for (const nd of mapCandidates(state)) out.push({ cmd: 'pick_node', node_id: nd.id });
  } else if (gs === 'event') {
    out.push({ cmd: 'event_choose', choice_id: 'a' });
    for (const nd of mapCandidates(state)) out.push({ cmd: 'pick_node', node_id: nd.id });
  } else if (gs === 'forge') {
    const ops = run.forge_offer?.ops ?? run.forge_offer ?? [];
    const n = Array.isArray(ops) ? ops.length : 0;
    for (let i = 0; i < Math.min(n, 3); i++) out.push({ cmd: 'forge_take', op_index: i });
    for (const nd of mapCandidates(state)) out.push({ cmd: 'pick_node', node_id: nd.id });
  }
  return out;
}

// Available map nodes: mirrors engine `pickable_nodes` (§2.3 path rule) —
// floor 0 when no current node, else current node's connections, plus the
// un-completed current node itself when back on the map (retry after a
// score-loss walk-away).
export function mapCandidates(state) {
  const run = state.run ?? {};
  const floors = run.map?.floors ?? [];
  const all = floors.flat();
  const cur = run.current_node ?? null;
  if (!cur) return all.filter((n) => n.floor === 0);
  const curNode = all.find((n) => n.id === cur);
  const ids = [...(curNode?.connections ?? [])];
  if (state.game_state === 'map' && curNode && !curNode.completed && !ids.includes(cur)) {
    ids.push(cur);
  }
  return all.filter((n) => ids.includes(n.id));
}