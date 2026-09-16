/**
 * TS contract for the roulette-core serde JSON surface (TASK-007, REQ-001/002).
 *
 * RISK-004 fallback: handwritten types verified by the drift test
 * `crates/roulette-wasm/tests/ts_schema_snapshot.rs`, which extracts the
 * `Command` and `EngineEvent` variant lists from the Rust source and fails
 * the build if this file drifts. Regenerate tag lists by hand whenever that
 * test names a missing/unknown tag.
 *
 * Serde conventions (normative): `Command` is internally tagged `cmd`,
 * `EngineEvent` is internally tagged `event`, both snake_case; `BetType` is
 * externally tagged ({"number": 5} / "red"); state snapshots are plain
 * structs with snake_case fields.
 */

export const COMMAND_TAGS = [
  'start_run', 'draft_card', 'draft_wheel', 'finish_loadout', 'pick_node',
  'play_card', 'buy_draw', 'place_bet', 'remove_bet', 'clear_bets', 'rebet',
  'sacrifice', 'predict', 'spin', 'purchase', 'forge_take', 'forge_reroll',
  'event_choose', 'buy_level', 'customize', 'undo',
] as const;

export const EVENT_TAGS = [
  'run_started', 'node_picked', 'battle_started', 'card_played', 'draw_bought',
  'bet_placed', 'bet_removed', 'bets_cleared', 'rebet', 'sacrificed',
  'prediction_shown', 'spin_started', 'ball_landed', 'spin_resolved',
  'intent_executed', 'round_ended', 'battle_ended', 'node_completed',
  'node_failed', 'reward_cards', 'chips_changed', 'purchased', 'card_gained',
  'wheel_gained', 'healed', 'forge_applied', 'forge_rerolled', 'event_chosen',
  'color_level_bought', 'slot_color_cycled', 'slot_added', 'slot_removed',
  'slot_number_set', 'wheel_saved', 'wheel_cancelled', 'game_over',
  'run_victory', 'undone',
] as const;

export type CommandTag = (typeof COMMAND_TAGS)[number];
export type EventTag = (typeof EVENT_TAGS)[number];

// -- shared payloads ---------------------------------------------------------

export type Difficulty = 'short' | 'medium' | 'long';
export type SlotColor = 'red' | 'black' | 'green' | 'gold' | 'purple' | 'cyan' | 'crimson';
export type Side = 'player' | 'enemy';
export type RoundOutcome = 'in_progress' | 'player_victory' | 'player_defeat' | 'sudden_death';

export type BetType =
  | 'red' | 'black' | 'green' | 'odd' | 'even' | 'gold' | 'purple' | 'cyan' | 'crimson'
  | { number: number }
  | { dozen: number }
  | { column: number };

export interface Bet { bet_type: BetType; amount: number }

export type CustomizeOp =
  | { op: 'cycle_color'; slot: number }
  | { op: 'add_slot'; number: number }
  | { op: 'remove_slot'; slot: number }
  | { op: 'set_number'; slot: number; number: number }
  | { op: 'save' }
  | { op: 'cancel' };

// -- Command union (tag = "cmd") ---------------------------------------------

export type Command =
  | { cmd: 'start_run'; difficulty: Difficulty }
  | { cmd: 'draft_card'; card_id: string }
  | { cmd: 'draft_wheel' }
  | { cmd: 'finish_loadout' }
  | { cmd: 'pick_node'; node_id: string }
  | { cmd: 'play_card'; hand_index: number }
  | { cmd: 'buy_draw' }
  | { cmd: 'place_bet'; bet: BetType; amount: number }
  | { cmd: 'remove_bet'; bet: BetType; amount: number }
  | { cmd: 'clear_bets' }
  | { cmd: 'rebet' }
  | { cmd: 'sacrifice' }
  | { cmd: 'predict' }
  | { cmd: 'spin' }
  | { cmd: 'purchase'; item_index: number }
  | { cmd: 'forge_take'; op_index: number }
  | { cmd: 'forge_reroll' }
  | { cmd: 'event_choose'; choice_id: string }
  | { cmd: 'buy_level'; color: SlotColor }
  | { cmd: 'customize'; op: CustomizeOp }
  | { cmd: 'undo' };

// -- EngineEvent union (tag = "event") ---------------------------------------

export type BattleResultEvent =
  | { result: 'victory'; rewards: string[] }
  | { result: 'defeat' }
  | { result: 'game_over' };

export type EngineEvent =
  | { event: 'run_started'; seed: string; difficulty: Difficulty }
  | { event: 'node_picked'; id: string; node_type: string }
  | { event: 'battle_started'; tier: string; round_limit: number; enemy_hp: number }
  | { event: 'card_played'; card_id: string; cost: number }
  | { event: 'draw_bought'; cost: number; card_id: string }
  | { event: 'bet_placed'; bet: BetType; amount: number }
  | { event: 'bet_removed'; bet: BetType; amount: number }
  | { event: 'bets_cleared' }
  | { event: 'rebet' }
  | { event: 'sacrificed' }
  | { event: 'prediction_shown'; start_slot: number; size: number }
  | { event: 'spin_started'; side: Side }
  | { event: 'ball_landed'; side: Side; number: number }
  | { event: 'spin_resolved'; side: Side; pts: number; insurance_refund: number; all_lost: boolean }
  | { event: 'intent_executed' }
  | { event: 'round_ended'; round: number; player_pts: number; enemy_pts: number; outcome: RoundOutcome }
  | { event: 'battle_ended'; result: BattleResultEvent }
  | { event: 'node_completed' }
  | { event: 'node_failed' }
  | { event: 'reward_cards'; ids: string[] }
  | { event: 'chips_changed'; side: Side; delta: number }
  | { event: 'purchased'; item: number; price: number }
  | { event: 'card_gained'; id: string }
  | { event: 'wheel_gained'; id: string }
  | { event: 'healed'; hp: number }
  | { event: 'forge_applied'; op: number }
  | { event: 'forge_rerolled' }
  | { event: 'event_chosen'; choice: string; hp_lost: number; chips_gained: number; cards: string[] }
  | { event: 'color_level_bought'; color: SlotColor; level: number }
  | { event: 'slot_color_cycled'; slot: number; color: SlotColor }
  | { event: 'slot_added'; number: number }
  | { event: 'slot_removed'; slot: number }
  | { event: 'slot_number_set'; slot: number; number: number }
  | { event: 'wheel_saved' }
  | { event: 'wheel_cancelled' }
  | { event: 'game_over' }
  | { event: 'run_victory' }
  | { event: 'undone' };

// -- state snapshots (get_state_json) ----------------------------------------

/** §4 wheel definition (engine-owned content; REQ-002). */
export interface WheelConfig {
  id: string;
  name: string;
  description: string;
  /** Physical slot order (§4.6 canonical sequences). */
  numbers: number[];
  green_numbers: number[];
  red_numbers: number[];
  black_numbers: number[];
  /** Permanent special colors keyed by number. */
  slot_colors: Record<string, SlotColor>;
  payout_multipliers: Record<string, number>;
}

export interface MapNode {
  id: string;
  floor: number;
  lane: number;
  node_type: string;
  connections: string[];
  completed: boolean;
}

export interface RunStateJson {
  hp: number;
  max_hp: number;
  chips: number;
  store_points: number;
  deck: string[];
  curses: string[];
  difficulty: Difficulty;
  map: { floors: MapNode[][] };
  current_floor: number | null;
  current_node: string | null;
  /** Remaining snapshot keys are feature areas (shop_offer, forge_offer, …). */
  [k: string]: unknown;
}

export interface BattleStateJson {
  phase: string;
  turn: number;
  bets: Bet[];
  enemy_bets: Bet[];
  chips_pool: number;
  enemy_chips_pool: number;
  hand: string[];
  active_played: string[];
  draw_pile: number;
  discard_pile: number;
  player_board: unknown;
  enemy_board: unknown;
  player_wheel: WheelConfig;
  enemy_wheel: WheelConfig;
  player_hp: number;
  enemy_hp: number;
  enemy_intent: string | null;
  curses: string[];
  physics: Record<string, unknown>;
  is_sudden_death: boolean;
  round: number;
  max_rounds: number;
}

export interface GameStateJson {
  seed: string;
  game_state: 'menu' | 'loadout_store' | 'map' | 'combat' | 'shop' | 'event' | 'forge' | 'victory' | 'game_over';
  run: RunStateJson | null;
  battle: BattleStateJson | null;
  undo_depth: number;
}

/** EngineError JSON (SEC-002: verbatim bridge errors). */
export interface EngineErrorJson {
  error: true;
  kind: string;
  message: string;
}

/** spin_telemetry(side) binary layout (TASK-008 side channel, little-endian):
 *  [u32 frame_count][u32 ball_count][u32 slot_count][f32 slot_width]
 *  then frame_count × [f32 wheel_angle, f32 ball0..ballN]. Fixed dt 1/120 s. */
export interface SpinTelemetryHeader {
  frameCount: number;
  ballCount: number;
  slotCount: number;
  slotWidth: number;
}

export function parseTelemetry(bytes: Uint8Array): SpinTelemetryHeader & { frames: Float32Array[] } {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const frameCount = view.getUint32(0, true);
  const ballCount = view.getUint32(4, true);
  const slotCount = view.getUint32(8, true);
  const slotWidth = view.getFloat32(12, true);
  const stride = 1 + ballCount;
  const frames: Float32Array[] = [];
  for (let f = 0; f < frameCount; f++) {
    const off = 16 + f * stride * 4;
    frames.push(new Float32Array(bytes.buffer, bytes.byteOffset + off, stride).slice());
  }
  return { frameCount, ballCount, slotCount, slotWidth, frames };
}