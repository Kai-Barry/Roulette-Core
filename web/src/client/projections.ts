/**
 * TASK-012: pure projections from engine state JSON into view models.
 *
 * Every function here is pure: (GameStateJson, optional extras) -> view.
 * No engine calls, no DOM, no mutation — headless-unit-testable with
 * fixtures captured from real engine output (PAT-001).
 */

import type {
  BattleStateJson,
  Bet,
  GameStateJson,
  MapNode,
  RunStateJson,
  SlotColor,
  WheelConfig,
} from '../engine/schema.ts';

export type GameScreen =
  | 'menu' | 'loadout' | 'map' | 'combat' | 'shop' | 'event' | 'forge' | 'victory' | 'game_over';

export interface HudView {
  hp: number; maxHp: number; chips: number; storePoints: number;
  difficulty: string; floor: number; undoDepth: number;
}

export interface MapNodeView {
  id: string; floor: number; lane: number; type: string;
  connections: string[]; completed: boolean; pickable: boolean;
}

export interface MapView {
  floor: number; floors: number; nodes: MapNodeView[];
}

export interface CombatView {
  phase: string; round: number; maxRounds: number; suddenDeath: boolean;
  playerHp: number; enemyHp: number;
  hand: string[]; handCount: number; drawPile: number; discardPile: number;
  bets: Bet[]; enemyBets: Bet[]; chipsPool: number; enemyChipsPool: number;
  enemyIntent: string | null;
  canBet: boolean; canSpin: boolean; canPlay: boolean;
  playerWheel: WheelConfig; enemyWheel: WheelConfig;
  playerColorLevels: Record<string, number>;
}

export interface ShopItemView {
  index: number; kind: 'card' | 'wheel' | 'blood_infusion';
  name: string; price: number; affordable: boolean;
}

export interface ShopView { items: ShopItemView[]; chips: number }

export interface ForgeOpView {
  index: number; id: string; name: string; description: string;
  rarity: string; cost: number; affordable: boolean; free: boolean;
}

export interface ForgeView {
  ops: ForgeOpView[]; chips: number; freeOpsRemaining: number; rerollsUsed: number;
  canCustomize: boolean;
}

export interface EventView { title: string; flavor: string | null }

export interface LoadoutView {
  cardIds: string[]; wheelId: string; cardPrice: number;
  storePoints: number;
}

export function screenOf(state: GameStateJson): GameScreen {
  const gs = state.game_state;
  if (gs === 'loadout_store') return 'loadout';
  if (gs === 'game_over') return 'game_over';
  return gs as GameScreen;
}

export function hud(state: GameStateJson): HudView {
  const run = state.run;
  if (!run) return { hp: 0, maxHp: 0, chips: 0, storePoints: 0, difficulty: '-', floor: 0, undoDepth: state.undo_depth };
  return {
    hp: run.hp, maxHp: run.max_hp, chips: run.chips, storePoints: run.store_points,
    difficulty: run.difficulty, floor: run.current_floor, undoDepth: state.undo_depth,
  };
}

/** §2.3 map flow: nodes on the current floor are pickable from the start;
 * on deeper floors only nodes connected from completed ancestors. */
export function pickableNodeIds(run: RunStateJson): Set<string> {
  if (run.current_node === null) {
    return new Set((run.map.floors[0] ?? []).map((n) => n.id));
  }
  const nodes = run.current_floor === 0 ? (run.map.floors[0] ?? []) : (run.map.floors[run.current_floor] ?? []);
  return new Set(nodes.map((n) => n.id));
}

export function mapView(state: GameStateJson): MapView {
  const run = state.run!;
  const floors = run.map.floors;
  const all: MapNode[] = floors.flat();
  const curFloorNodes = floors[run.current_floor] ?? [];
  const pickable = pickableNodeIds(run);
  return {
    floor: run.current_floor,
    floors: floors.length,
    nodes: all.map((n) => ({
      id: n.id, floor: n.floor, lane: n.lane, type: n.node_type,
      connections: n.connections, completed: n.completed,
      pickable: curFloorNodes.some((c) => c.id === n.id) && pickable.has(n.id) && !n.completed,
    })),
  };
}

export function combatView(state: GameStateJson, colorLevels: Record<string, number> = {}): CombatView {
  const b: BattleStateJson = state.battle!;
  return {
    phase: b.phase, round: b.round, maxRounds: b.max_rounds, suddenDeath: b.is_sudden_death,
    playerHp: b.player_hp, enemyHp: b.enemy_hp,
    hand: b.hand, handCount: b.hand.length, drawPile: b.draw_pile, discardPile: b.discard_pile,
    bets: b.bets, enemyBets: b.enemy_bets, chipsPool: b.chips_pool, enemyChipsPool: b.enemy_chips_pool,
    enemyIntent: b.enemy_intent,
    canBet: b.phase === 'betting', canSpin: b.phase === 'betting', canPlay: b.phase === 'betting',
    playerWheel: b.player_wheel, enemyWheel: b.enemy_wheel,
    playerColorLevels: colorLevels,
  };
}

export function shopView(state: GameStateJson): ShopView {
  const run = state.run!;
  const offer = run.shop_offer as { items: unknown[] } | null;
  const items: ShopItemView[] = (offer?.items ?? []).map((raw, i) => {
    const it = raw as Record<string, unknown>;
    if ('card' in it) {
      const def = (it as { card: { def: { name: string }; price: number } }).card;
      return { index: i, kind: 'card', name: def.def.name, price: def.price, affordable: run.chips >= def.price };
    }
    if ('wheel' in it) {
      const w = (it as { wheel: { def: { name: string }; price: number } }).wheel;
      return { index: i, kind: 'wheel', name: w.def.name, price: w.price, affordable: run.chips >= w.price };
    }
    const bi = (it as { blood_infusion: { price: number } }).blood_infusion;
    return { index: i, kind: 'blood_infusion', name: 'Blood Infusion', price: bi.price, affordable: run.chips >= bi.price };
  });
  return { items, chips: run.chips };
}

export function forgeView(state: GameStateJson): ForgeView {
  const run = state.run!;
  const offer = run.forge_offer as
    { ops: Array<{ id: string; name: string; description: string; rarity: string; cost: number }>; free_ops_remaining: number; rerolls_used: number }
    | null;
  const ops: ForgeOpView[] = (offer?.ops ?? []).map((op, i) => ({
    index: i, id: op.id, name: op.name, description: op.description, rarity: op.rarity, cost: op.cost,
    affordable: run.chips >= op.cost,
    free: i < (offer?.free_ops_remaining ?? 0),
  }));
  return {
    ops, chips: run.chips,
    freeOpsRemaining: offer?.free_ops_remaining ?? 0,
    rerollsUsed: offer?.rerolls_used ?? 0,
    canCustomize: true,
  };
}

export function eventView(state: GameStateJson): EventView {
  const run = state.run!;
  return {
    title: run.current_event_title ?? '(event)',
    flavor: run.event_flavor ?? null,
  };
}

export function loadoutView(state: GameStateJson): LoadoutView {
  const run = state.run!;
  const offer = run.loadout_offer as { card_ids: string[]; wheel_id: string; card_price: number } | null;
  return {
    cardIds: offer?.card_ids ?? [],
    wheelId: offer?.wheel_id ?? '',
    cardPrice: offer?.card_price ?? 0,
    storePoints: run.store_points,
  };
}

/** Slot color of a wheel number, resolving color-level swaps (§4.5):
 * `green` level unlocks shift green slots; special colors are permanent. */
export function slotColorOf(wheel: WheelConfig, number: number, levels: Record<string, number> = {}): SlotColor {
  if (wheel.slot_colors[String(number)]) return wheel.slot_colors[String(number)];
  if (wheel.green_numbers.includes(number)) return 'green';
  if (wheel.red_numbers.includes(number)) return 'red';
  if (wheel.black_numbers.includes(number)) return 'black';
  return 'green';
}