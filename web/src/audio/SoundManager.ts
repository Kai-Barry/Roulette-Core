/**
 * TASK-028: SoundManager (§13 audio, blueprint GOAL-006) — procedural Web
 * Audio, engine-event-driven only (REQ-002 audio analogue: recipes fire on
 * engine events / sim events, never invented by the renderer).
 *
 * Three buses (§12.9): music 55% / drone 15% / sfx 80% — defaults come from
 * the persisted SettingsStore (TASK-022), so user volume edits survive.
 *
 * Headless-safe: in Node (or before a user gesture) there is no
 * AudioContext; every recipe records into `played` for TEST-013
 * diagnostics instead of audible assertions. `unlock()` builds the real
 * graph on first user gesture.
 */

import type { EngineEvent } from '../engine/schema.ts';
import type { TimedSimEvent } from '../client/GameClient.ts';
import { DEFAULT_VOLUMES, type Bus, type SettingsStore } from '../ui/settings.ts';

/** All §13.4 recipe names — existence-checked by TEST-013. */
export const SFX_RECIPES = [
  'roulette_click',
  'peg_bounce',
  'ball_settle',
  'shotgun',
  'ball_split',
  'settle_timeout',
  'spin_whoosh',
  'intent_sting',
  'outcome_win',
  'outcome_lose',
  'outcome_push',
] as const;
export type SfxRecipe = (typeof SFX_RECIPES)[number];

export type SoundDiagnostics = {
  ctx: 'web' | 'headless';
  buses: Record<Bus, number>;
  recipes: readonly string[];
  lastPlayed: Array<{ recipe: string; at: number }>;
  simQueue: number;
  music: { playing: boolean; tempo: number | null; layers: string[] };
};

type Ctx = {
  destination: unknown;
  currentTime: number;
  sampleRate: number;
  createGain(): { gain: { value: number; setValueAtTime(v: number, t: number): void; linearRampToValueAtTime(v: number, t: number): void; exponentialRampToValueAtTime(v: number, t: number): void }; connect(n: unknown): unknown };
  createOscillator(): { type: string; frequency: { value: number; setValueAtTime(v: number, t: number): void; exponentialRampToValueAtTime(v: number, t: number): void }; connect(n: unknown): unknown; start(t?: number): void; stop(t: number): void };
  createBuffer(channels: number, length: number, rate: number): { getChannelData(c: number): Float32Array };
  createBufferSource(): { buffer: unknown; connect(n: unknown): unknown; start(t?: number): void; stop(t?: number): void };
  createBiquadFilter(): { type: string; frequency: { value: number; setValueAtTime(v: number, t: number): void; exponentialRampToValueAtTime(v: number, t: number): void }; Q: { value: number }; connect(n: unknown): unknown };
};

/** Minimal Web Audio surface the recipes use (typed loosely on purpose:
 * the real AudioContext satisfies it structurally; tests stub nothing). */
type AudioLike = Ctx | null;

const now = () => (typeof performance !== 'undefined' ? performance.now() : Date.now());

export class SoundManager {
  private store: SettingsStore | null;
  private ctx: AudioLike;
  private master: { gain: { value: number }; connect(n: unknown): unknown } | null = null;
  private gains: Partial<Record<Bus, { gain: { value: number }; connect(n: unknown): unknown }>> = {};
  private noise: unknown = null;
  /** TEST-013: last N recipes played, newest last. */
  readonly played: Array<{ recipe: string; at: number }> = [];
  /** TASK-029: sim events queued for playback-synced triggering. */
  private simQueue: TimedSimEvent[] = [];
  private simCursor = 0;
  private encounter: { playing: boolean; tempo: number | null; layers: string[] } = {
    playing: false, tempo: null, layers: [],
  };

  constructor(store?: SettingsStore) {
    this.store = store ?? null;
    const Ctor = (globalThis as { AudioContext?: unknown }).AudioContext as
      | (new () => Ctx)
      | undefined;
    this.ctx = Ctor ? new Ctor() : null;
    if (this.ctx) this.buildGraph();
  }

  /** Build buses (master → destination, per-bus gains) — web only. */
  private buildGraph(): void {
    const ctx = this.ctx as Ctx;
    const master: { gain: { value: number }; connect(n: unknown): unknown } =
      ctx.createGain() as never;
    this.master = master;
    master.connect(ctx.destination);
    for (const bus of ['music', 'drone', 'sfx'] as const) {
      const g: { gain: { value: number }; connect(n: unknown): unknown } = ctx.createGain() as never;
      g.connect(this.master);
      this.gains[bus] = g;
    }
    // Shared 1 s pink-ish noise buffer for noise-based recipes.
    const buf = ctx.createBuffer(1, ctx.sampleRate, ctx.sampleRate);
    const data = (buf as { getChannelData(c: number): Float32Array }).getChannelData(0);
    let b0 = 0, b1 = 0, b2 = 0;
    for (let i = 0; i < data.length; i++) {
      const w = Math.random() * 2 - 1;
      b0 = 0.99765 * b0 + w * 0.099;
      b1 = 0.963 * b1 + w * 0.2965;
      b2 = 0.57 * b2 + w * 1.0526;
      data[i] = (b0 + b1 + b2 + w * 0.1848) * 0.15;
    }
    this.noise = buf;
    this.syncVolumes();
  }

  /** Push current SettingsStore volumes onto the bus gains (§12.9). */
  syncVolumes(): void {
    for (const bus of ['music', 'drone', 'sfx'] as const) {
      const v = this.store ? this.store.volume(bus) : DEFAULT_VOLUMES[bus];
      const g = this.gains[bus];
      if (g) g.gain.value = v;
    }
  }

  /** First user gesture: resume a suspended context (browser autoplay). */
  unlock(): void {
    const ctx = this.ctx as (Ctx & { resume?: () => Promise<void> }) | null;
    if (ctx?.resume) void ctx.resume();
  }

  private record(recipe: string): void {
    this.played.push({ recipe, at: now() });
    if (this.played.length > 256) this.played.shift();
  }

  // -- primitive voices ------------------------------------------------------

  private tone(bus: Bus, freq: number, dur: number, vol: number, type = 'square', slideTo?: number): void {
    const ctx = this.ctx;
    if (!ctx) return;
    const t = ctx.currentTime;
    const osc = ctx.createOscillator();
    osc.type = type;
    osc.frequency.setValueAtTime(freq, t);
    if (slideTo) osc.frequency.exponentialRampToValueAtTime(slideTo, t + dur);
    const g = ctx.createGain();
    g.gain.setValueAtTime(vol, t);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    osc.connect(g);
    g.connect(this.gains[bus] ?? this.master);
    osc.start(t);
    osc.stop(t + dur + 0.02);
  }

  private noiseBurst(bus: Bus, dur: number, vol: number, from: number, to: number): void {
    const ctx = this.ctx;
    if (!ctx || !this.noise) return;
    const t = ctx.currentTime;
    const src = ctx.createBufferSource();
    src.buffer = this.noise;
    const f = ctx.createBiquadFilter();
    f.type = 'bandpass';
    f.frequency.setValueAtTime(from, t);
    f.frequency.exponentialRampToValueAtTime(to, t + dur);
    f.Q.value = 1.2;
    const g = ctx.createGain();
    g.gain.setValueAtTime(vol, t);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    src.connect(f);
    f.connect(g);
    g.connect(this.gains[bus] ?? this.master);
    src.start(t);
    src.stop(t + dur + 0.02);
  }

  // -- §13.4 recipes -----------------------------------------------------------

  playRouletteClick(): void { this.record('roulette_click'); this.tone('sfx', 2100, 0.035, 0.5); }
  playPegBounce(): void {
    this.record('peg_bounce');
    this.tone('sfx', 3150, 0.06, 0.35, 'triangle');
    this.tone('sfx', 4700, 0.05, 0.2, 'triangle');
  }
  playBallSettle(): void { this.record('ball_settle'); this.tone('sfx', 190, 0.12, 0.6, 'sine', 90); }
  playShotgun(): void { this.record('shotgun'); this.noiseBurst('sfx', 0.16, 0.8, 900, 180); }
  playBallSplit(): void { this.record('ball_split'); this.tone('sfx', 700, 0.09, 0.4, 'square', 1600); }
  playSettleTimeout(): void { this.record('settle_timeout'); this.tone('sfx', 110, 0.35, 0.5, 'sawtooth', 70); }
  playSpinWhoosh(): void { this.record('spin_whoosh'); this.noiseBurst('sfx', 0.45, 0.4, 300, 2400); }
  playIntentSting(): void {
    this.record('intent_sting');
    this.tone('sfx', 220, 0.28, 0.4, 'sawtooth', 180);
  }
  playOutcome(result: 'win' | 'lose' | 'push'): void {
    this.record(`outcome_${result}`);
    if (result === 'win') {
      this.tone('sfx', 523, 0.14, 0.4, 'triangle');
      this.tone('sfx', 659, 0.14, 0.35, 'triangle');
      this.tone('sfx', 784, 0.22, 0.35, 'triangle');
    } else if (result === 'lose') {
      this.tone('sfx', 220, 0.3, 0.4, 'sawtooth', 140);
    } else {
      this.tone('sfx', 440, 0.16, 0.3, 'sine');
    }
  }

  // -- engine event mapping (REQ-002: recipes fire only on engine events) ----

  onEvents(events: EngineEvent[]): void {
    for (const e of events) {
      switch (e.event) {
        case 'spin_started': if (e.side === 'player') this.playSpinWhoosh(); break;
        case 'round_ended':
          this.playOutcome(
            e.outcome === 'player_victory' ? 'win'
            : e.outcome === 'player_defeat' ? 'lose'
            : 'push',
          );
          break;
        case 'intent_executed': this.playIntentSting(); break;
        default: break;
      }
    }
  }

  // -- TASK-029: playback-synced click/bounce track ---------------------------

  /** Queue the LAST spin's timed sim events (from `client.spinSimEvents`).
   * Events fire as the renderer playback cursor crosses their frame. */
  queueSimEvents(events: TimedSimEvent[]): void {
    this.simQueue = [...events].sort((a, b) => a.frame - b.frame);
    this.simCursor = 0;
  }

  /** Fire all queued events whose frame <= `frame` (renderer calls this
   * each playback tick so clicks/bounces stay in sync — §5.4). */
  tickPlayback(frame: number): void {
    while (this.simCursor < this.simQueue.length && this.simQueue[this.simCursor].frame <= frame) {
      const e = this.simQueue[this.simCursor].event;
      this.simCursor++;
      if ('DividerTick' in e) this.playRouletteClick();
      else if ('PegHit' in e) this.playPegBounce();
      else if ('BallSettled' in e) this.playBallSettle();
      else if ('ShotgunFired' in e) this.playShotgun();
      else if ('BallSplit' in e) this.playBallSplit();
      else if ('SettleTimeout' in e) this.playSettleTimeout();
    }
  }

  /** Whether the playback-synced queue has pending events (diagnostics). */
  get simPending(): number {
    return this.simQueue.length - this.simCursor;
  }

  // -- TASK-030 hooks (EncounterMusic drives these) ----------------------------

  setEncounter(state: { playing: boolean; tempo: number | null; layers: string[] }): void {
    this.encounter = state;
  }

  /** Sequencer voice: schedule a note at an absolute (AudioContext) time.
   * Headless: records only — TEST-013 checks state, never sound. */
  scheduleNote(bus: Bus, freq: number, at: number, dur: number, type: string): void {
    this.record(`music:${type}:${Math.round(freq)}`);
    const ctx = this.ctx;
    if (!ctx) return;
    const osc = ctx.createOscillator();
    osc.type = type;
    osc.frequency.setValueAtTime(freq, at);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, at);
    g.gain.linearRampToValueAtTime(0.25, at + 0.01);
    g.gain.exponentialRampToValueAtTime(0.0001, at + dur);
    osc.connect(g);
    g.connect(this.gains[bus] ?? this.master);
    osc.start(at);
    osc.stop(at + dur + 0.02);
  }

  /** Sequencer voice: scheduled noise hit through a bandpass (hats). */
  scheduleNoise(bus: Bus, at: number, dur: number, freq: number): void {
    this.record(`music:noise:${Math.round(freq)}`);
    const ctx = this.ctx;
    if (!ctx || !this.noise) return;
    const src = ctx.createBufferSource();
    src.buffer = this.noise;
    const f = ctx.createBiquadFilter();
    f.type = 'bandpass';
    f.frequency.setValueAtTime(freq, at);
    f.Q.value = 2;
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.3, at);
    g.gain.exponentialRampToValueAtTime(0.0001, at + dur);
    src.connect(f);
    f.connect(g);
    g.connect(this.gains[bus] ?? this.master);
    src.start(at);
    src.stop(at + dur + 0.02);
  }

  // -- TEST-013 diagnostics (headless bus/recipe checks; no audible asserts) ---

  diagnostics(): SoundDiagnostics {
    return {
      ctx: this.ctx ? 'web' : 'headless',
      buses: {
        music: this.store ? this.store.volume('music') : DEFAULT_VOLUMES.music,
        drone: this.store ? this.store.volume('drone') : DEFAULT_VOLUMES.drone,
        sfx: this.store ? this.store.volume('sfx') : DEFAULT_VOLUMES.sfx,
      },
      recipes: SFX_RECIPES,
      lastPlayed: this.played.slice(-32),
      simQueue: this.simPending,
      music: { ...this.encounter },
    };
  }
}