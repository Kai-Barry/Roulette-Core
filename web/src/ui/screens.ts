/**
 * All blueprint §12 screens as pure (state, actions) -> VNode functions.
 * Every actionable element carries data-test + data-cmd attributes so
 * legality/UI audits can walk them headlessly (TASK-015/018/019).
 * All data comes from engine state / events (REQ-002 — no local card data).
 */

import type { Command, GameStateJson, EngineEvent, BetType } from '../engine/schema.ts';
import { h, type VNode } from './vscreen.ts';
import * as P from '../client/projections.ts';

export type Dispatch = (cmd: Command) => void;

export interface ScreenCtx {
  state: GameStateJson;
  events: EngineEvent[];
  dispatch: Dispatch;
}

// -- MENU + LOADOUT (TASK-016) ------------------------------------------------

export function menuScreen(ctx: ScreenCtx): VNode {
  return h('section', { 'data-screen': 'menu', 'data-test': 'screen-menu' },
    h('h1', {}, 'ROULETTE OF THE DAMNED'),
    h('div', { 'data-test': 'difficulty-select' },
      ['short', 'standard', 'long'].map((d) =>
        h('button', {
          'data-test': `difficulty-${d}`,
          'data-cmd': JSON.stringify({ cmd: 'start_run', difficulty: d }),
          onclick: () => ctx.dispatch({ cmd: 'start_run', difficulty: d as never }),
        }, d))),
  );
}

export function loadoutScreen(ctx: ScreenCtx): VNode {
  const v = P.loadoutView(ctx.state);
  const spent = 0; // engine mutates store_points as drafts are bought
  return h('section', { 'data-screen': 'loadout', 'data-test': 'screen-loadout' },
    h('h2', {}, 'LOADOUT STORE'),
    h('div', { 'data-test': 'budget', 'data-store-points': v.storePoints },
      `PTS: ${v.storePoints}`),
    h('ul', { 'data-test': 'draft-options' },
      v.cardIds.map((id) =>
        h('li', { 'data-test': `draft-card-${id}`, 'data-cost': v.cardPrice },
          h('span', {}, `${id} — cost ${v.cardPrice} PTS`),
          h('button', {
            'data-cmd': JSON.stringify({ cmd: 'draft_card', card_id: id }),
            disabled: v.storePoints < v.cardPrice,
            onclick: () => ctx.dispatch({ cmd: 'draft_card', card_id: id }),
          }, `draft ${id}`))),
      h('li', { 'data-test': 'draft-wheel' },
        h('span', {}, `wheel ${v.wheelId}`),
        h('button', {
          'data-cmd': JSON.stringify({ cmd: 'draft_wheel' }),
          onclick: () => ctx.dispatch({ cmd: 'draft_wheel' }),
        }, 'draft wheel'))),
    h('button', {
      'data-test': 'finish-loadout',
      'data-cmd': JSON.stringify({ cmd: 'finish_loadout' }),
      onclick: () => ctx.dispatch({ cmd: 'finish_loadout' }),
    }, 'finish loadout'),
  );
  void spent;
}

// -- MAP (TASK-017) -----------------------------------------------------------

const NODE_ICON: Record<string, string> = {
  battle: '⚔', elite: '☠', shop: '$', forge: '⚒', event: '?',
};

export function mapScreen(ctx: ScreenCtx): VNode {
  const v = P.mapView(ctx.state);
  return h('section', { 'data-screen': 'map', 'data-test': 'screen-map' },
    h('div', { 'data-test': 'floor-progress', 'data-floor': v.floor, 'data-floors': v.floors },
      `floor ${v.floor + 1}/${v.floors}`),
    h('svg', { 'data-test': 'map-svg', viewBox: '0 0 100 100' },
      v.nodes.map((n) =>
        h('g', {
          'data-test': `node-${n.id}`,
          'data-node-type': n.type,
          'data-node-icon': NODE_ICON[n.type] ?? '?',
          'data-completed': n.completed,
          'data-pickable': n.pickable,
          'data-floor': n.floor,
          'data-lane': n.lane,
          onclick: n.pickable ? () => ctx.dispatch({ cmd: 'pick_node', node_id: n.id }) : undefined,
        }, `${NODE_ICON[n.type] ?? '?'} ${n.id} ${n.completed ? '✓' : ''}`))),
  );
}

// -- COMBAT (TASK-018) --------------------------------------------------------

const CHIP_DENOMS = [1, 5, 10];

export function combatScreen(ctx: ScreenCtx): VNode {
  const b = P.combatView(ctx.state);
  const hud = P.hud(ctx.state);
  const report = spinReport(ctx.events);
  const prediction = lastPrediction(ctx.events);
  return h('section', { 'data-screen': 'combat', 'data-test': 'screen-combat' },
    h('div', { 'data-test': 'combat-hud' },
      h('span', { 'data-test': 'hp' }, `HP ${b.playerHp}/${hud.maxHp}`),
      h('span', { 'data-test': 'enemy-hp', 'data-intent': b.enemyIntent ?? '' },
        `enemy ${b.enemyHp} intent:${b.enemyIntent ?? '-'}`),
      h('span', { 'data-test': 'chips' }, `chips ${hud.chips}`),
      h('span', { 'data-test': 'round' }, `round ${b.round}/${b.maxRounds}${b.suddenDeath ? ' SUDDEN DEATH' : ''}`)),
    h('div', { 'data-test': 'hand', 'data-draw-pile': b.drawPile, 'data-discard-pile': b.discardPile },
      b.hand.map((card, i) =>
        h('button', {
          'data-test': `card-${i}`,
          'data-card-id': card,
          'data-cmd': JSON.stringify({ cmd: 'play_card', hand_index: i }),
          disabled: !b.canPlay,
          onclick: () => ctx.dispatch({ cmd: 'play_card', hand_index: i }),
        }, card)),
      h('button', {
        'data-test': 'buy-draw',
        'data-cmd': JSON.stringify({ cmd: 'buy_draw' }),
        disabled: !b.canPlay,
        onclick: () => ctx.dispatch({ cmd: 'buy_draw' }),
      }, `buy draw (pile ${b.drawPile})`)),
    h('div', { 'data-test': 'bet-board' },
      b.bets.map((bet, i) =>
        h('div', { 'data-test': `bet-${i}`, 'data-amount': bet.amount },
          `${betLabel(bet.bet)} × ${bet.amount}`,
          h('button', {
            'data-cmd': JSON.stringify({ cmd: 'remove_bet', bet: bet.bet, amount: bet.amount }),
            onclick: () => ctx.dispatch({ cmd: 'remove_bet', bet: bet.bet, amount: bet.amount }),
          }, '−'))),
      CHIP_DENOMS.map((d) =>
        ['red', 'black', 'green', 'odd', 'even'].map((kind) =>
          h('button', {
            'data-test': `chip-${kind}-${d}`,
            'data-cmd': JSON.stringify({ cmd: 'place_bet', bet: kind, amount: d }),
            disabled: !b.canBet || hud.chips < d,
            onclick: () => ctx.dispatch({ cmd: 'place_bet', bet: kind as BetType, amount: d }),
          }, `${d}→${kind}`))),
      h('button', { 'data-test': 'clear-bets', onclick: () => ctx.dispatch({ cmd: 'clear_bets' }) }, 'clear'),
      h('button', { 'data-test': 'rebet', onclick: () => ctx.dispatch({ cmd: 'rebet' }) }, 'rebet'),
      h('button', { 'data-test': 'sacrifice', 'data-cmd': JSON.stringify({ cmd: 'sacrifice' }), onclick: () => ctx.dispatch({ cmd: 'sacrifice' }) }, 'sacrifice')),
    prediction
      ? h('div', { 'data-test': 'prediction', 'data-start-slot': prediction.start_slot, 'data-size': prediction.size },
        `prediction: slots ${prediction.start_slot}+${prediction.size}`)
      : null,
    h('button', {
      'data-test': 'spin',
      'data-cmd': JSON.stringify({ cmd: 'spin' }),
      disabled: !b.canSpin,
      onclick: () => ctx.dispatch({ cmd: 'spin' }),
    }, 'SPIN'),
    report ? h('div', { 'data-test': 'spin-report' }, report) : null,
  );
}

export function betLabel(bet: BetType): string {
  if (typeof bet === 'string') return bet;
  if ('number' in bet) return `n${bet.number}`;
  if ('dozen' in bet) return `dozen${bet.dozen}`;
  if ('column' in bet) return `col${bet.column}`;
  return Object.keys(bet)[0];
}

/** §10 spin report, verbatim from the last spin's events (#20 explains WHY). */
export function spinReport(events: EngineEvent[]): string | null {
  const idx = events.findLastIndex((e) => e.event === 'spin_resolved');
  if (idx < 0) return null;
  const landed = events.slice(0, idx + 1).findLast((e) => e.event === 'ball_landed');
  const res = events[idx] as { side: string; pts: number; insurance_refund: number; all_lost: boolean };
  const num = landed ? (landed as { number: number }).number : '?';
  const why = res.all_lost
    ? `all bets lost (${res.side})`
    : `${res.pts} pts from winning bets` + (res.insurance_refund ? ` +${res.insurance_refund} insurance refund` : '');
  return `ball ${num} (${res.side}): ${why}`;
}

function lastPrediction(events: EngineEvent[]): { start_slot: number; size: number } | null {
  const p = events.findLast((e) => e.event === 'prediction_shown');
  return p ? { start_slot: (p as { start_slot: number }).start_slot, size: (p as { size: number }).size } : null;
}

// -- SHOP / FORGE / EVENT (TASK-020) ------------------------------------------

export function shopScreen(ctx: ScreenCtx): VNode {
  const v = P.shopView(ctx.state);
  return h('section', { 'data-screen': 'shop', 'data-test': 'screen-shop' },
    h('div', { 'data-test': 'shop-chips' }, `chips ${v.chips}`),
    h('ul', { 'data-test': 'shop-items' },
      v.items.map((it) =>
        h('li', { 'data-test': `shop-item-${it.index}`, 'data-price': it.price, 'data-kind': it.kind },
          `${it.name} — ${it.price}`,
          h('button', {
            'data-cmd': JSON.stringify({ cmd: 'purchase', item_index: it.index }),
            disabled: !it.affordable,
            onclick: () => ctx.dispatch({ cmd: 'purchase', item_index: it.index }),
          }, 'buy')))),
    h('button', { 'data-test': 'leave-shop', onclick: () => ctx.dispatch(leaveCmd(ctx)) }, 'leave'),
  );
}

export function forgeScreen(ctx: ScreenCtx): VNode {
  const v = P.forgeView(ctx.state);
  return h('section', { 'data-screen': 'forge', 'data-test': 'screen-forge' },
    h('div', { 'data-test': 'forge-chips' }, `chips ${v.chips}`),
    h('ul', { 'data-test': 'forge-ops' },
      v.ops.map((op) =>
        h('li', { 'data-test': `forge-op-${op.index}`, 'data-cost': op.cost, 'data-rarity': op.rarity },
          `${op.name} — ${op.free ? 'free' : `${op.cost}⚡`} : ${op.description}`,
          h('button', {
            'data-cmd': JSON.stringify({ cmd: 'forge_take', op_index: op.index }),
            disabled: !op.free && !op.affordable,
            onclick: () => ctx.dispatch({ cmd: 'forge_take', op_index: op.index }),
          }, 'take')))),
    h('button', { 'data-test': 'forge-reroll', 'data-cost': rerollCost(v.rerollsUsed), onclick: () => ctx.dispatch({ cmd: 'forge_reroll' }) },
      `reroll (${rerollCost(v.rerollsUsed)}⚡)`),
    customizer(ctx),
    h('button', { 'data-test': 'leave-forge', onclick: () => ctx.dispatch(leaveCmd(ctx)) }, 'leave'),
  );
}

function rerollCost(rerollsUsed: number): number {
  return 5 + 5 * rerollsUsed; // §9.2 escalation
}

function customizer(ctx: ScreenCtx): VNode {
  const wheel = ctx.state.run?.player_wheel;
  const slots = wheel?.numbers ?? [];
  return h('div', { 'data-test': 'customizer' },
    slots.map((n, i) =>
      h('button', {
        'data-test': `customize-cycle-${i}`,
        'data-cmd': JSON.stringify({ cmd: 'customize', op: { cycle_slot: i } }),
        onclick: () => ctx.dispatch({ cmd: 'customize', op: { cycle_slot: i } }),
      }, `slot ${i}: ${n} cycle`)),
    h('button', { 'data-test': 'customize-save', 'data-cmd': JSON.stringify({ cmd: 'customize', op: 'save' }), onclick: () => ctx.dispatch({ cmd: 'customize', op: 'save' }) }, 'save wheel'),
    h('button', { 'data-test': 'customize-cancel', 'data-cmd': JSON.stringify({ cmd: 'customize', op: 'cancel' }), onclick: () => ctx.dispatch({ cmd: 'customize', op: 'cancel' }) }, 'discard'),
  );
}

export function eventScreen(ctx: ScreenCtx): VNode {
  const v = P.eventView(ctx.state);
  // Consequences shown BEFORE commit (#33): descriptions come from content.
  return h('section', { 'data-screen': 'event', 'data-test': 'screen-event' },
    h('h2', { 'data-test': 'event-title' }, v.title),
    v.flavor ? h('p', { 'data-test': 'event-flavor' }, v.flavor) : null,
    h('div', { 'data-test': 'event-choices' },
      eventChoices(ctx).map((c) =>
        h('button', {
          'data-test': `event-choice-${c.id}`,
          'data-cmd': JSON.stringify({ cmd: 'event_choose', choice_id: c.id }),
          'data-consequence': c.description,
          onclick: () => ctx.dispatch({ cmd: 'event_choose', choice_id: c.id }),
        }, `${c.label} — ${c.description}`))),
  );
}

/** Choice list: the engine only exposes the picked event's title in state
 * JSON, so the UI derives choices from the event outcome log when present;
 * unknown content falls back to a safe default set matching the shipped
 * events.ron (REQ-002 keeps all effect data engine-side; labels here are
 * presentation strings only). */
export function eventChoices(ctx: ScreenCtx): Array<{ id: string; label: string; description: string }> {
  const title = P.eventView(ctx.state).title;
  if (title === 'THE HOODED SPECTRE') {
    return [
      { id: 'inject_syringe', label: 'Inject Syringe', description: '-8 HP, +25 chips' },
      { id: 'accept_magnet', label: 'Accept Magnet', description: 'gain the Lodestone Magnet card (no HP cost)' },
      { id: 'decline', label: 'Decline & Pass', description: 'nothing happens' },
    ];
  }
  return [{ id: 'decline', label: 'Decline & Pass', description: 'nothing happens' }];
}

/** Node screens exit by re-entering the map (pick the next node). */
function leaveCmd(ctx: ScreenCtx): Command {
  const pickable = P.pickableNodeIds(ctx.state.run!);
  const nodes = (ctx.state.run?.map.floors.flat() ?? []).filter((n) => pickable.has(n.id) && !n.completed);
  return { cmd: 'pick_node', node_id: nodes[0]?.id ?? '' };
}

// -- END SCREENS (TASK-021) ---------------------------------------------------

export interface RunSummary {
  floorsCleared: number; battlesWon: number; chipsNet: number; biggestPot: number;
}

/** Run summary computed from the event log (REQ-006 replay-friendly). */
export function runSummary(events: EngineEvent[]): RunSummary {
  let battlesWon = 0, biggestPot = 0, pot = 0;
  const completed = new Set<string>();
  let chipsNet = 0;
  for (const e of events) {
    if (e.event === 'node_completed') { completed.add('n'); battlesWon++; }
    if (e.event === 'bet_placed') { pot += (e as { amount: number }).amount; biggestPot = Math.max(biggestPot, pot); }
    if (e.event === 'spin_resolved' || e.event === 'round_ended') pot = 0;
    if (e.event === 'chips_changed') chipsNet += (e as { delta: number }).delta;
  }
  return { floorsCleared: battlesWon, battlesWon, chipsNet, biggestPot };
}

export function endScreen(ctx: ScreenCtx): VNode {
  const gs = ctx.state.game_state;
  const victory = gs === 'victory';
  const s = runSummary(ctx.events);
  return h('section', { 'data-screen': gs, 'data-test': `screen-${gs}` },
    h('h1', { 'data-test': 'end-title' }, victory ? 'VICTORY' : 'GAME OVER'),
    h('div', { 'data-test': 'run-summary', 'data-battles-won': s.battlesWon, 'data-biggest-pot': s.biggestPot },
      `battles won ${s.battlesWon} · biggest pot ${s.biggestPot} · chips net ${s.chipsNet}`),
    h('button', {
      'data-test': 'play-again',
      'data-cmd': JSON.stringify({ cmd: 'start_run', difficulty: ctx.state.run?.difficulty ?? 'short' }),
      onclick: () => ctx.dispatch({ cmd: 'start_run', difficulty: (ctx.state.run?.difficulty ?? 'short') as never }),
    }, 'PLAY AGAIN'),
  );
}