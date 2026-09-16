/**
 * TASK-011: typed `GameClient` owning the wasm `EngineHandle`.
 *
 * Methods mirror the `Command` enum exactly (schema.ts). Every successful
 * apply emits `ClientEvent`s to subscribers; failures throw the engine's
 * `EngineErrorJson` verbatim (SEC-002). Environment-agnostic: the wasm module
 * is injected so Node tests and the browser share one code path (PAT-001).
 */

import type {
  BetType,
  Command,
  CustomizeOp,
  Difficulty,
  EngineErrorJson,
  EngineEvent,
  GameStateJson,
  SlotColor,
} from '../engine/schema.ts';

export type WasmModule = {
  new_engine(seed: string): EngineHandleLike;
};
export type EngineHandleLike = {
  apply_command(cmd_json: string): string;
  get_state_json(): string;
  undo(): boolean;
  undo_depth(): number;
  seed(): string;
  spin_telemetry(side: number): Uint8Array;
  spin_sim_events(side: number): string;
};

/** One physics sim event with its telemetry frame index (TASK-029 sound
 * hooks §5.4). Shape mirrors `SimEvent` from `phys/events.rs` (serde JSON). */
export type TimedSimEvent = {
  frame: number;
  event:
    | { PegHit: { ball: number } }
    | { DividerTick: { ball: number } }
    | { BallSettled: { ball: number; slot: number } }
    | { SettleTimeout: { ball: number } }
    | { ShotgunFired: { count: number } }
    | { BallSplit: { ball: number } };
};

export type ClientEvent =
  | { kind: 'stateChanged'; state: GameStateJson }
  | { kind: 'events'; events: EngineEvent[] }
  | { kind: 'error'; error: EngineErrorJson };
export type Subscriber = (ev: ClientEvent) => void;

/** The engine returns error JSON via a thrown string (wasm-bindgen JsValue). */
export class EngineError extends Error {
  readonly payload: EngineErrorJson;
  constructor(msg: string, payload: EngineErrorJson | null) {
    super(msg);
    this.name = 'EngineError';
    this.payload = payload ?? { error: true, kind: 'unknown', message: msg };
  }
}

export class GameClient {
  /** Immutable for a client instance; `fromHandle` uses the handle's seed. */
  seed: string;
  private handle: EngineHandleLike;
  private subscribers = new Set<Subscriber>();
  /** Full EngineEvent log since boot (REQ-003). */
  events: EngineEvent[] = [];
  /** Full applied-command log since boot (REQ-006 replay). */
  commands: Command[] = [];

  constructor(wasm: WasmModule, seed: string) {
    this.seed = seed;
    this.handle = wasm.new_engine(seed);
  }

  /** Attach an already-created handle (e.g. for resume/replay). */
  static fromHandle(handle: EngineHandleLike): GameClient {
    const c = Object.create(GameClient.prototype) as GameClient;
    c.handle = handle;
    c.seed = handle.seed();
    c.subscribers = new Set();
    c.events = [];
    c.commands = [];
    return c;
  }

  on(fn: Subscriber): () => void {
    this.subscribers.add(fn);
    return () => this.subscribers.delete(fn);
  }

  private emit(ev: ClientEvent) {
    for (const fn of this.subscribers) fn(ev);
  }

  state(): GameStateJson {
    return JSON.parse(this.handle.get_state_json());
  }

  /** Apply one command object (escape hatch for dynamic/scripted commands). */
  raw(cmd: Command): EngineEvent[] {
    this.commands.push(cmd);
    let ev: EngineEvent[];
    try {
      ev = JSON.parse(this.handle.apply_command(JSON.stringify(cmd)));
    } catch (msg) {
      const payload = parseError(msg);
      const err = new EngineError(payload?.message ?? String(msg), payload);
      this.emit({ kind: 'error', error: err.payload });
      throw err;
    }
    this.events.push(...ev);
    this.emit({ kind: 'events', events: ev });
    this.emit({ kind: 'stateChanged', state: this.state() });
    return ev;
  }

  undoDepth(): number {
    return this.handle.undo_depth();
  }

  /** Undo via the wasm handle directly (bypasses the command log — the
   * replay log must stay a pure list of applied forward commands). */
  undo(): boolean {
    const ok = this.handle.undo();
    if (ok) this.emit({ kind: 'stateChanged', state: this.state() });
    return ok;
  }

  /** Binary spin telemetry frames for `side` (0 player, 1 enemy), TASK-008. */
  spinTelemetry(side: 0 | 1): Uint8Array {
    return this.handle.spin_telemetry(side);
  }

  /** Physics sim events with telemetry frame indices for `side` (0 player,
   * 1 enemy), TASK-029 §5.4 sound hooks. Side channel — never in the event
   * log. Returns `[]` when the side was not simulated (uniform fast path). */
  spinSimEvents(side: 0 | 1): TimedSimEvent[] {
    const json = this.handle.spin_sim_events(side);
    if (!json || json === '[]') return [];
    const parsed: unknown = JSON.parse(json);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (e): e is TimedSimEvent =>
        typeof e === 'object' && e !== null &&
        typeof (e as TimedSimEvent).frame === 'number' &&
        typeof (e as TimedSimEvent).event === 'object' && (e as TimedSimEvent).event !== null,
    );
  }

  // -- Command-mirroring methods -------------------------------------------

  startRun(difficulty: Difficulty) { return this.raw({ cmd: 'start_run', difficulty }); }
  draftCard(cardId: string) { return this.raw({ cmd: 'draft_card', card_id: cardId }); }
  draftWheel() { return this.raw({ cmd: 'draft_wheel' }); }
  finishLoadout() { return this.raw({ cmd: 'finish_loadout' }); }
  pickNode(nodeId: string) { return this.raw({ cmd: 'pick_node', node_id: nodeId }); }
  playCard(handIndex: number) { return this.raw({ cmd: 'play_card', hand_index: handIndex }); }
  buyDraw() { return this.raw({ cmd: 'buy_draw' }); }
  placeBet(bet: BetType, amount: number) { return this.raw({ cmd: 'place_bet', bet, amount }); }
  removeBet(bet: BetType, amount: number) { return this.raw({ cmd: 'remove_bet', bet, amount }); }
  clearBets() { return this.raw({ cmd: 'clear_bets' }); }
  rebet() { return this.raw({ cmd: 'rebet' }); }
  sacrifice() { return this.raw({ cmd: 'sacrifice' }); }
  predict() { return this.raw({ cmd: 'predict' }); }
  spin() { return this.raw({ cmd: 'spin' }); }
  purchase(itemIndex: number) { return this.raw({ cmd: 'purchase', item_index: itemIndex }); }
  forgeTake(opIndex: number) { return this.raw({ cmd: 'forge_take', op_index: opIndex }); }
  forgeReroll() { return this.raw({ cmd: 'forge_reroll' }); }
  eventChoose(choiceId: string) { return this.raw({ cmd: 'event_choose', choice_id: choiceId }); }
  buyLevel(color: SlotColor) { return this.raw({ cmd: 'buy_level', color }); }
  customize(op: CustomizeOp) { return this.raw({ cmd: 'customize', op }); }
}

function parseError(msg: unknown): EngineErrorJson | null {
  if (typeof msg !== 'string') return null;
  try {
    const j = JSON.parse(msg);
    if (j && j.error === true) return j as EngineErrorJson;
  } catch {
    /* thrown string was not JSON */
  }
  return null;
}