/**
 * TASK-014: headless app shell bootstrap.
 *
 * `boot({ seed, headless })` constructs the full GameClient (+ persistence
 * autosave) with no rendering and no audio — the same client the Phase 3/4
 * UI mounts on. Exposed to `window.__RT` when running in a browser with
 * `?headless=1` (REQ-004); in Node the returned object is the harness API.
 */

import type { Command, EngineEvent, GameStateJson } from '../engine/schema.ts';
import { GameClient } from './client/GameClient.ts';
import { Persistence, localStorageLike, stateHash, type StorageLike } from './client/persistence.ts';
import { ScreenManager, type ScreenName, type UiFeedback } from './ui/ScreenManager.ts';
import { uiCandidates } from './ui/legality.ts';
import { SettingsStore, type Bus } from './ui/settings.ts';

export interface RtApi {
  client: GameClient;
  state(): GameStateJson;
  events(n?: number): EngineEvent[];
  cmd(c: Command): EngineEvent[];
  /** Replay a policy over legal candidates until the run terminates. */
  run(policy: (state: GameStateJson) => Command | null, maxCommands?: number): {
    outcome: string; commands: number; events: number;
  };
  stateHash(): string;
  /** Current visible screen name (ScreenManager, PAT-003). */
  screen(): ScreenName;
  /** Current screen vnode tree — pure data, queryable via data-* attrs. */
  tree(): ReturnType<ScreenManager['tree']>;
  /** Dispatch through the ScreenManager (ui_feedback recorded). */
  dispatch(c: Command): EngineEvent[];
  /** TASK-019 audit log. */
  uiFeedback(): UiFeedback[];
  /** TASK-018 legality helper. */
  candidates(): Command[];
  /** TASK-022 settings. */
  settings: SettingsStore;
  setVolume(bus: Bus, v: number): void;
}

export interface BootOptions {
  seed: string;
  headless?: boolean;
  wasm: unknown; // WasmModule (typed loosely to keep this file env-agnostic)
  storage?: StorageLike;
  /** Inject an existing handle (resume path builds one internally). */
  handle?: unknown;
}

export function boot(opts: BootOptions): RtApi {
  const client = new GameClient(opts.wasm as never, opts.seed);
  const persistence = new Persistence(opts.wasm as never, opts.storage ?? localStorageLike());
  // TASK-013 autosave: on every event batch, persist snapshot + command log.
  client.on((ev) => {
    if (ev.kind === 'events') persistence.save(client);
  });
  return makeRt(client, persistence);
}

export function makeRt(client: GameClient, persistence?: Persistence, settings?: SettingsStore): RtApi {
  const screens = new ScreenManager(client);
  const settingsStore = settings ?? new SettingsStore(null);
  return {
    client,
    state: () => client.state(),
    events: (n = 20) => client.events.slice(-n),
    cmd: (c) => client.raw(c),
    stateHash: () => stateHash(client.state()),
    screen: () => screens.screen,
    tree: () => screens.tree(),
    dispatch: (c) => screens.dispatch(c),
    uiFeedback: () => screens.feedback,
    candidates: () => uiCandidates(client.state()),
    settings: settingsStore,
    setVolume: (bus, v) => settingsStore.setVolume(bus, v),
    run(policy, maxCommands = 5000) {
      const terminal = new Set(['victory', 'game_over']);
      let commands = 0;
      while (commands < maxCommands) {
        const gs = client.state().game_state;
        if (gs === 'menu') client.raw({ cmd: 'start_run', difficulty: 'short' });
        else if (terminal.has(gs)) break;
        else {
          const c = policy(client.state());
          if (!c) break;
          client.raw(c);
        }
        commands++;
      }
      const finalGs = client.state().game_state;
      return { outcome: finalGs, commands, events: client.events.length };
    },
  };
}

/** Browser entry: install `window.__RT` under ?headless=1 (REQ-004). */
export function installRtWindow(rt: RtApi): void {
  if (typeof window !== 'undefined') {
    (window as unknown as Record<string, unknown>).__RT = rt;
  }
}