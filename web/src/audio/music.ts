/**
 * TASK-030: Encounter-music sequencer (§13.3) — step sequencer with a
 * lookahead scheduler, tempo keyed by enemy tier, boss layering.
 *
 * Headless-safe: the sequencer state machine (tempo, layers, step counter)
 * runs anywhere; actual audio only fires through the SoundManager buses and
 * only when an AudioContext exists. TEST-013 asserts state, not sound.
 */

import type { SoundManager } from './SoundManager.ts';

/** §13.3 tempo table: BPM per enemy tier index (0-based). Bosses add a
 * layered drum+bass pattern on top at a 10% push. */
export const TIER_TEMPO: readonly number[] = [96, 108, 126, 144, 160];
export const BOSS_TEMPO_PUSH = 1.1;

/** Sixteenth-note step patterns (16 steps = one bar). Values are semitone
 * offsets from the root; null = rest. */
const BASS_PATTERN: Array<number | null> = [
  0, null, 0, null, 3, null, 0, null, 0, null, 0, null, 5, null, 3, null,
];
const ARP_PATTERN: Array<number | null> = [
  12, null, 15, null, 12, null, 19, null, 12, null, 15, null, 12, null, 19, null,
];
const BOSS_KICK: Array<boolean> = [
  true, false, false, false, false, false, true, false, false, false, true, false, false, false, false, false,
];
const BOSS_HAT: Array<boolean> = [
  false, false, true, false, false, false, true, false, false, false, true, false, false, false, true, false,
];

export type EncounterState = {
  playing: boolean;
  tempo: number | null;
  layers: string[];
  step: number;
};

export type EncounterInput = {
  tier: number;
  boss: boolean;
};

const ROOT = 110; // A2

export class EncounterMusic {
  private sound: SoundManager;
  private timer: ReturnType<typeof setInterval> | null = null;
  private nextNoteTime = 0;
  private step = 0;
  private input: EncounterInput | null = null;
  private tempo: number | null = null;
  private audioNow: () => number;

  constructor(sound: SoundManager, audioNow?: () => number) {
    this.sound = sound;
    this.audioNow = audioNow ?? (() => performance.now() / 1000);
  }

  /** §13.3: tempo from the enemy tier; bosses push 10%. */
  static tempoFor(input: EncounterInput): number {
    const base = TIER_TEMPO[Math.min(Math.max(input.tier, 0), TIER_TEMPO.length - 1)];
    return input.boss ? Math.round(base * BOSS_TEMPO_PUSH) : base;
  }

  /** Layer list for an encounter: bass always, arp from tier 2, drums for
   * bosses (§13.3 boss layering). */
  static layersFor(input: EncounterInput): string[] {
    const layers = ['bass'];
    if (input.tier >= 2 || input.boss) layers.push('arp');
    if (input.boss) layers.push('drums');
    return layers;
  }

  start(input: EncounterInput): void {
    this.stop();
    this.input = input;
    this.tempo = EncounterMusic.tempoFor(input);
    this.step = 0;
    this.nextNoteTime = this.audioNow() + 0.05;
    // 25 ms scheduler tick with a 100 ms lookahead (standard technique).
    this.timer = setInterval(() => this.schedule(), 25);
    this.syncSound(true);
  }

  stop(): void {
    if (this.timer !== null) {
      clearInterval(this.timer);
      this.timer = null;
    }
    this.input = null;
    this.tempo = null;
    this.syncSound(false);
  }

  get playing(): boolean {
    return this.timer !== null && this.input !== null;
  }

  /** Diagnostics snapshot (TEST-013). */
  state(): EncounterState {
    return {
      playing: this.playing,
      tempo: this.tempo,
      layers: this.input ? EncounterMusic.layersFor(this.input) : [],
      step: this.step,
    };
  }

  private syncSound(on: boolean): void {
    const s = this.state();
    this.sound.setEncounter({
      playing: on && s.playing,
      tempo: on ? s.tempo : null,
      layers: on ? s.layers : [],
    });
  }

  /** Lookahead scheduler: while the next 16th is within 100 ms, schedule it. */
  private schedule(): void {
    if (!this.input || this.tempo === null) return;
    const stepDur = 60 / this.tempo / 4;
    while (this.nextNoteTime < this.audioNow() + 0.1) {
      this.playStep(this.step, this.nextNoteTime);
      this.step = (this.step + 1) % 16;
      this.nextNoteTime += stepDur;
    }
  }

  private playStep(step: number, at: number): void {
    const input = this.input as EncounterInput;
    const layers = EncounterMusic.layersFor(input);
    const tempo = this.tempo as number;
    const stepDur = 60 / tempo / 4;
    const semis = (n: number) => ROOT * Math.pow(2, n / 12);
    if (layers.includes('bass')) {
      const b = BASS_PATTERN[step];
      if (b !== null) this.sound.scheduleNote('music', semis(b), at, stepDur * 1.8, 'sawtooth');
    }
    if (layers.includes('arp')) {
      const a = ARP_PATTERN[step];
      if (a !== null) this.sound.scheduleNote('music', semis(a + 12), at, stepDur * 0.8, 'square');
    }
    if (layers.includes('drums')) {
      if (BOSS_KICK[step]) this.sound.scheduleNote('music', 55, at, stepDur * 1.5, 'sine');
      if (BOSS_HAT[step]) this.sound.scheduleNoise('music', at, stepDur * 0.4, 6000);
    }
  }
}