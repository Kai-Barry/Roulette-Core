/**
 * TASK-015/019: ScreenManager keyed by GameState (PAT-003) + feedback
 * instrumentation for the lens gates.
 *
 * - Renders the keyed screen for the current state into a vnode tree; state
 *   transitions emit exactly one visible-screen change (TEST-012).
 * - `dispatch(cmd)` applies through the client and records a `ui_feedback`
 *   record (command → channels touched, wall-clock ms) — TEST-013's data.
 * - `?headless=1&audit=1` needs no browser: everything here is data.
 */

import type { Command, EngineEvent, GameStateJson } from '../engine/schema.ts';
import type { GameClient, ClientEvent } from '../client/GameClient.ts';
import { h, type VNode } from './vscreen.ts';
import * as S from './screens.ts';

export type ScreenName = 'menu' | 'loadout' | 'map' | 'combat' | 'shop' | 'forge' | 'event' | 'victory' | 'game_over';

/** TASK-019: one record per dispatched command. */
export interface UiFeedback {
  command: Command;
  /** Channels the command visibly touched. */
  channels: ('events' | 'state' | 'screen')[];
  /** Wall-clock apply→render latency in ms (gate #57: ≤ 100 ms). */
  ms: number;
  ok: boolean;
}

const RENDERERS: Record<ScreenName, (ctx: S.ScreenCtx) => VNode> = {
  menu: S.menuScreen,
  loadout: S.loadoutScreen,
  map: S.mapScreen,
  combat: S.combatScreen,
  shop: S.shopScreen,
  forge: S.forgeScreen,
  event: S.eventScreen,
  victory: S.endScreen,
  game_over: S.endScreen,
};

export function screenNameOf(state: GameStateJson): ScreenName {
  const gs = state.game_state;
  if (gs === 'loadout_store') return 'loadout';
  if (gs === 'victory' || gs === 'game_over') return gs;
  return (gs === 'menu' || gs === 'map' || gs === 'combat' || gs === 'shop' || gs === 'forge' || gs === 'event')
    ? gs : 'menu';
}

export class ScreenManager {
  private client: GameClient;
  private unsub: () => void;
  /** ui_feedback records (TASK-019 audit). */
  feedback: UiFeedback[] = [];
  private _screen: ScreenName;
  private _tree: VNode | null = null;
  /** How many visible-screen changes have occurred since construction. */
  screenChanges = 0;

  constructor(client: GameClient) {
    this.client = client;
    this._screen = screenNameOf(client.state());
    this.unsub = client.on((ev) => this.onClientEvent(ev));
  }

  get screen(): ScreenName {
    return this._screen;
  }

  /** Current vnode tree (lazily rendered, pure data). */
  tree(): VNode {
    if (!this._tree) this.render();
    return this._tree!;
  }

  dispose(): void {
    this.unsub();
  }

  /** Force a re-render (also used by tests). */
  render(): VNode {
    const st = this.client.state();
    const tree = RENDERERS[this._screen]({
      state: st, events: this.client.events, dispatch: (c) => this.dispatch(c),
    });
    this._tree = tree;
    return tree;
  }

  /** Apply a command; records ui_feedback (channels + latency). Errors are
   * recorded as ok:false and rethrown for the caller to surface. */
  dispatch(cmd: Command): EngineEvent[] {
    const t0 = Date.now();
    const before = this._screen;
    const channels: UiFeedback['channels'] = [];
    let events: EngineEvent[] = [];
    const ms = () => Date.now() - t0;
    try {
      events = this.client.raw(cmd);
      channels.push('events', 'state');
      if (this._screen !== before) channels.push('screen');
      this.feedback.push({ command: cmd, channels, ms: ms(), ok: true });
      this.render();
      return events;
    } catch (err) {
      this.feedback.push({ command: cmd, channels: [], ms: ms(), ok: false });
      throw err;
    }
  }

  private onClientEvent(ev: ClientEvent): void {
    if (ev.kind === 'stateChanged') {
      const after = screenNameOf(ev.state);
      if (after !== this._screen) {
        this._screen = after;
        this.screenChanges++;
      }
    }
  }
}