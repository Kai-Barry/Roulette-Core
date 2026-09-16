/**
 * TASK-026: event-driven effects (§10.7) — intent telegraphs and
 * special-color on-land bursts. Pure functions of the EngineEvent stream
 * (PAT-001): an effect exists only while its event says so, and every effect
 * carries the engine data that spawned it (REQ-002).
 *
 * Headless-safe: effects are plain data + three.js objects; no GL required.
 */

import { Group, Mesh, MeshBasicMaterial, RingGeometry, Vector3 } from 'three';
import { SLOT_PALETTE } from './WheelVisual.ts';
import type { EngineEvent, WheelConfig, SlotColor } from '../engine/schema.ts';

const SPECIAL_COLORS: ReadonlySet<SlotColor> = new Set(['gold', 'purple', 'cyan', 'crimson']);

/** Which landed numbers trigger an on-land burst (special slot colors). */
export function specialBurstFor(landed: { side: string; number: number }, cfg: WheelConfig): SlotColor | null {
  if (landed.side !== 'player') return null;
  const color = cfg.slot_colors[String(landed.number)]
    ?? (cfg.green_numbers.includes(landed.number) ? 'green' : cfg.red_numbers.includes(landed.number) ? 'red' : 'black');
  return SPECIAL_COLORS.has(color) ? color : null;
}

export interface FxObjects {
  /** Burst mesh + the engine color that spawned it. */
  burst: { mesh: Mesh; color: SlotColor; ttl: number } | null;
  /** Enemy intent ring (visible while intent != null). */
  intent: { ring: Mesh; label: string; pulse: number } | null;
}

/** Build the fx layer (added to the scene once). */
export function buildFx(): { group: Group; fx: FxObjects } {
  const group = new Group();
  group.name = 'fx';
  return { group, fx: { burst: null, intent: null } };
}

/** §10.7 on-land burst: a colored shockwave ring above the landed wheel. */
export function triggerBurst(fx: FxObjects, color: SlotColor, add: (m: Mesh) => void): void {
  if (fx.burst) fx.burst.mesh.removeFromParent();
  const mesh = new Mesh(
    new RingGeometry(0.4, 0.7, 24),
    new MeshBasicMaterial({ color: SLOT_PALETTE[color], transparent: true, opacity: 0.9 }),
  );
  mesh.rotation.x = -Math.PI / 2;
  mesh.position.set(0, 3.1, 0); // above the wheel rim (wheel at y=2, r=4)
  mesh.name = 'fx_burst';
  add(mesh);
  fx.burst = { mesh, color, ttl: 0.8 };
}

/** §10.7 intent telegraph: pulsing ring over the enemy table while an intent
 * is declared. Label is the engine's intent string verbatim. */
export function setIntent(fx: FxObjects, label: string | null, add: (m: Mesh) => void): void {
  if (fx.intent) {
    fx.intent.ring.removeFromParent();
    fx.intent = null;
  }
  if (!label) return;
  const ring = new Mesh(
    new RingGeometry(1.2, 1.5, 32),
    new MeshBasicMaterial({ color: 0xff3b5c, transparent: true, opacity: 0.7 }),
  );
  ring.rotation.x = -Math.PI / 2;
  ring.position.set(0, 1.4, -26); // over the enemy table
  ring.name = 'fx_intent';
  add(ring);
  fx.intent = { ring, label, pulse: 0 };
}

/** Apply an event batch (PAT-001): effects appear only from engine events. */
export function applyEvents(fx: FxObjects, events: EngineEvent[], cfg: WheelConfig, add: (m: Mesh) => void): void {
  for (const e of events) {
    if (e.event === 'ball_landed') {
      const color = specialBurstFor({ side: e.side, number: e.number }, cfg);
      if (color) triggerBurst(fx, color, add);
    } else if (e.event === 'intent_executed') {
      setIntent(fx, null, add);
    }
  }
}

/** Per-frame decay; effects vanish when their ttl runs out (data-driven). */
export function tickFx(fx: FxObjects, dt: number): void {
  if (fx.burst) {
    fx.burst.ttl -= dt;
    const s = Math.max(0.2, 1 + (0.8 - fx.burst.ttl));
    fx.burst.mesh.scale.setScalar(s);
    if (fx.burst.ttl <= 0) {
      fx.burst.mesh.removeFromParent();
      fx.burst = null;
    }
  }
  if (fx.intent) {
    fx.intent.pulse += dt;
    fx.intent.ring.scale.setScalar(1 + Math.sin(fx.intent.pulse * 6) * 0.08);
  }
}

/** Headless audit summary (gates read this instead of pixels). */
export function fxSummary(fx: FxObjects): { burst: string | null; intent: string | null } {
  return { burst: fx.burst?.color ?? null, intent: fx.intent?.label ?? null };
}

/** Landing slot world position for burst placement (engine data only:
 * uses the canonical slot angle from WheelConfig.numbers). */
export function slotWorldPos(cfg: WheelConfig, number: number): Vector3 {
  const idx = cfg.numbers.indexOf(number);
  const n = Math.max(1, cfg.numbers.length);
  const a = idx < 0 ? 0 : (idx / n) * Math.PI * 2;
  return new Vector3(Math.cos(a) * 3.6, 3.1, Math.sin(a) * 3.6);
}