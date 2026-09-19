/**
 * TASK-013: persistence + deterministic replay (REQ-006 end-to-end).
 *
 * Autosave: on every event, write the full state JSON + command log into the
 * injected storage. Resume: boot a fresh engine, replay the command log,
 * and assert the resumed state hash equals the saved hash. Corrupt snapshots
 * fall back to a clean boot.
 *
 * Storage is injectable so Node tests run the same code as the browser.
 */

import type { Command, GameStateJson } from '../engine/schema.ts';
import { GameClient, EngineError, type WasmModule } from './GameClient.ts';

const SAVE_KEY = 'roulette-os.save.v1';

export interface SaveRecord {
  version: 1;
  seed: string;
  /** Full `get_state_json` snapshot at save time (human-readable autosave). */
  state: GameStateJson;
  /** Command log to replay for resume (REQ-006). */
  commands: Command[];
}

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

/** Deterministic state hash (djb2 over recursively sorted-key JSON). */
export function stateHash(state: unknown): string {
  let h = 5381;
  for (const ch of JSON.stringify(canonical(state))) {
    h = ((h << 5) + h + ch.charCodeAt(0)) | 0;
  }
  return (h >>> 0).toString(16);
}

function canonical(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(canonical);
  if (v && typeof v === 'object') {
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(v as object).sort()) out[k] = canonical((v as Record<string, unknown>)[k]);
    return out;
  }
  return v;
}

export class Persistence {
  private readonly wasm: WasmModule;
  private readonly storage: StorageLike;
  constructor(wasm: WasmModule, storage: StorageLike = memoryStorage()) {
    this.wasm = wasm;
    this.storage = storage;
  }

  /** Save a record (called on every event by the autosave subscriber). */
  save(client: GameClient): void {
    const rec: SaveRecord = {
      version: 1,
      seed: client.seed,
      state: client.state(),
      commands: client.commands,
    };
    this.storage.setItem(SAVE_KEY, JSON.stringify(rec));
  }

  load(): SaveRecord | null {
    const raw = this.storage.getItem(SAVE_KEY);
    if (!raw) return null;
    try {
      const rec = JSON.parse(raw) as SaveRecord;
      if (rec?.version !== 1 || typeof rec.seed !== 'string' || !Array.isArray(rec.commands)) {
        return null;
      }
      return rec;
    } catch {
      return null; // corrupt snapshot → caller falls back to clean boot
    }
  }

  clear(): void {
    this.storage.removeItem(SAVE_KEY);
  }

  /** Resume: fresh engine + full command replay; verifies the state hash.
   * Returns null when there is nothing to resume or the replay mismatches
   * (caller boots clean). REQ-006 verified end-to-end. */
  resume(): { client: GameClient; record: SaveRecord } | null {
    const rec = this.load();
    if (!rec) return null;
    const client = new GameClient(this.wasm, rec.seed);
    try {
      for (const cmd of rec.commands) client.raw(cmd);
    } catch (e) {
      if (e instanceof EngineError) return null; // replay hit an illegal state
      throw e;
    }
    if (stateHash(client.state()) !== stateHash(rec.state)) return null;
    return { client, record: rec };
  }
}

export function memoryStorage(): StorageLike {
  const map = new Map<string, string>();
  return {
    getItem: (k) => map.get(k) ?? null,
    setItem: (k, v) => map.set(k, v),
    removeItem: (k) => map.delete(k),
  };
}

/** Browser localStorage-backed StorageLike (window guard keeps Node clean). */
export function localStorageLike(): StorageLike {
  if (typeof window === 'undefined' || !window.localStorage) return memoryStorage();
  return {
    getItem: (k) => window.localStorage.getItem(k),
    setItem: (k, v) => window.localStorage.setItem(k, v),
    removeItem: (k) => window.localStorage.removeItem(k),
  };
}