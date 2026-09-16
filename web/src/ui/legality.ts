/**
 * Gate #56 support: which commands are legal in a given state?
 *
 * Single source of truth is the ENGINE: candidates are derived from engine
 * state the same way the policy bots do, but this module is conservative —
 * it returns what the UI may ENABLE. Actual legality is proven by the
 * engine accepting/rejecting; TEST-015 walks every candidate in every state
 * and asserts enable-state == engine outcome.
 */

import type { Command, GameStateJson } from '../engine/schema.ts';

export function pickableIds(state: GameStateJson): string[] {
  const run = state.run;
  if (!run) return [];
  const all = run.map.floors.flat();
  const cur = run.current_node;
  if (!cur) return all.filter((n) => n.floor === 0).map((n) => n.id);
  const curNode = all.find((n) => n.id === cur);
  const ids = [...(curNode?.connections ?? [])];
  if (state.game_state === 'map' && curNode && !curNode.completed && !ids.includes(cur)) ids.push(cur);
  return all.filter((n) => ids.includes(n.id)).map((n) => n.id);
}

/** Commands the UI may show enabled for this state (superset allowed; the
 * engine remains the arbiter — this only feeds control enablement). */
export function uiCandidates(state: GameStateJson): Command[] {
  const out: Command[] = [];
  const gs = state.game_state;
  const run = state.run;
  const battle = state.battle;
  if (gs === 'menu') {
    out.push({ cmd: 'start_run', difficulty: 'short' });
  } else if (gs === 'loadout_store') {
    const offer = run?.loadout_offer;
    if (offer) {
      for (const id of offer.card_ids) out.push({ cmd: 'draft_card', card_id: id });
      out.push({ cmd: 'draft_wheel' });
    }
    out.push({ cmd: 'finish_loadout' });
  } else if (gs === 'map') {
    for (const id of pickableIds(state)) out.push({ cmd: 'pick_node', node_id: id });
  } else if (gs === 'combat' && battle) {
    for (let i = 0; i < (battle.hand?.length ?? 0); i++) out.push({ cmd: 'play_card', hand_index: i });
    out.push({ cmd: 'buy_draw' });
    if ((battle.chips_pool ?? 0) > 0) {
      for (const bet of ['red', 'black', 'green', 'odd', 'even'] as const) {
        out.push({ cmd: 'place_bet', bet, amount: 1 });
      }
    }
    if (battle.bets?.length) {
      const b0 = battle.bets[0] as unknown as { bet_type: never; amount: number };
      out.push({ cmd: 'remove_bet', bet: b0.bet_type, amount: b0.amount });
      out.push({ cmd: 'clear_bets' });
    }
    out.push({ cmd: 'spin' });
    out.push({ cmd: 'rebet' });
    out.push({ cmd: 'sacrifice' });
  } else if (gs === 'shop') {
    const items = run?.shop_offer?.items ?? [];
    for (let i = 0; i < items.length; i++) out.push({ cmd: 'purchase', item_index: i });
    for (const id of pickableIds(state)) out.push({ cmd: 'pick_node', node_id: id });
  } else if (gs === 'event') {
    for (const c of eventChoiceIds(state)) out.push({ cmd: 'event_choose', choice_id: c });
    for (const id of pickableIds(state)) out.push({ cmd: 'pick_node', node_id: id });
  } else if (gs === 'forge') {
    const ops = run?.forge_offer?.ops ?? [];
    for (let i = 0; i < ops.length; i++) out.push({ cmd: 'forge_take', op_index: i });
    out.push({ cmd: 'forge_reroll' });
    for (const id of pickableIds(state)) out.push({ cmd: 'pick_node', node_id: id });
  }
  return out;
}

/** Event choice ids for the current event (presentation-safe fallback). */
export function eventChoiceIds(state: GameStateJson): string[] {
  const title = state.run?.current_event_title ?? '';
  if (title === 'THE HOODED SPECTRE') return ['inject_syringe', 'accept_magnet', 'decline'];
  return ['decline'];
}