/**
 * TASK-024: WheelVisual — wheel/ball meshes + canvas textures built from
 * `WheelConfig` (REQ-002: numbers/colors come from engine state; the render
 * never invents slots). Spin animation consumes `spin_telemetry` frames
 * (TASK-008, fixed 120 Hz dt); the final settled slot MUST equal
 * `EngineEvent::BallLanded` (TEST-010).
 */

import {
  CanvasTexture,
  Color,
  Group,
  Mesh,
  MeshBasicMaterial,
  SphereGeometry,
  TorusGeometry,
  CircleGeometry,
  type Texture,
} from 'three';
import type { SlotColor, WheelConfig } from '../engine/schema.ts';
import { parseTelemetry } from '../engine/schema.ts';

/** Slot color palette (§4.4). */
const PALETTE: Record<SlotColor, number> = {
  red: 0xd81838, black: 0x15161c, green: 0x0aa06e,
  gold: 0xffd75e, purple: 0x8a5cf6, cyan: 0x35c8e8,
  crimson: 0xb3123c,
};

const SLOT_RADIANS = (2 * Math.PI);

/** Pure draw-list for the wheel face: one entry per slot in canonical order.
 * Any backend (browser Canvas2D, headless tests) consumes this — the paint
 * function and the data are the same code path (REQ-002). */
export interface WheelDrawOp {
  kind: 'wedge' | 'hub';
  cx: number; cy: number; r: number;
  a0: number; a1: number;
  fill: string;
  stroke?: string;
  text?: string;
  textColor?: string;
}

export const SLOT_PALETTE = PALETTE;

export function wheelFaceDrawList(cfg: WheelConfig, size = 512): WheelDrawOp[] {
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2;
  const n = cfg.numbers.length;
  const specials = new Set(cfg.green_numbers);
  const reds = new Set(cfg.red_numbers);
  const ops: WheelDrawOp[] = [];
  for (let i = 0; i < n; i++) {
    const num = cfg.numbers[i];
    const color: SlotColor = cfg.slot_colors[String(num)]
      ?? (specials.has(num) ? 'green' : reds.has(num) ? 'red' : 'black');
    const a0 = (i / n) * SLOT_RADIANS - Math.PI / 2;
    const a1 = ((i + 1) / n) * SLOT_RADIANS - Math.PI / 2;
    ops.push({
      kind: 'wedge', cx, cy, r, a0, a1,
      fill: `#${new Color(PALETTE[color]).getHexString()}`,
      stroke: '#0a0a0f',
      text: String(num),
      textColor: color === 'black' || color === 'crimson' || color === 'purple' ? '#f0f2f5' : '#0a0a0f',
    });
  }
  ops.push({ kind: 'hub', cx, cy, r: r * 0.18, a0: 0, a1: SLOT_RADIANS, fill: '#0a0a0f' });
  return ops;
}

/** Raster the draw-list onto a canvas (browser runtime only). */
export function paintWheelFace(cfg: WheelConfig, size = 512): HTMLCanvasElement {
  if (typeof document === 'undefined') {
    throw new Error('paintWheelFace requires a DOM; use wheelFaceDrawList headlessly');
  }
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  for (const op of wheelFaceDrawList(cfg, size)) {
    ctx.beginPath();
    ctx.moveTo(op.cx, op.cy);
    ctx.arc(op.cx, op.cy, op.r, op.a0, op.a1);
    ctx.closePath();
    ctx.fillStyle = op.fill;
    ctx.fill();
    if (op.stroke) {
      ctx.strokeStyle = op.stroke;
      ctx.lineWidth = 2;
      ctx.stroke();
    }
    if (op.kind === 'wedge' && op.text) {
      const mid = (op.a0 + op.a1) / 2;
      ctx.save();
      ctx.translate(op.cx + Math.cos(mid) * op.r * 0.78, op.cy + Math.sin(mid) * op.r * 0.78);
      ctx.rotate(mid + Math.PI / 2);
      ctx.fillStyle = op.textColor!;
      ctx.font = `bold ${Math.max(10, size / cfg.numbers.length / 2)}px monospace`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(op.text, 0, 0);
      ctx.restore();
    }
  }
  return canvas;
}

export interface WheelVisual {
  group: Group;
  faceOps: WheelDrawOp[];
  faceTexture: Texture | null;
  /** Slot index → canonical number mapping (from config, never regenerated). */
  slotNumbers: number[];
  /** Set the wheel's rotation from telemetry (radians, slot 0 at angle 0). */
  setWheelAngle(a: number): void;
  /** Set ball positions from telemetry frame. */
  setBallPositions(angles: number[]): void;
  /** Number whose wedge is at the ball reference angle. */
      settledSlot(ballAngle: number, wheelAngle: number): number;
}

export function buildWheelVisual(cfg: WheelConfig): WheelVisual {
  const group = new Group();
  const faceOps = wheelFaceDrawList(cfg);
  const faceTexture: Texture | null = typeof document === 'undefined'
    ? null
    : new CanvasTexture(paintWheelFace(cfg));
  const disc = new Mesh(
    new CircleGeometry(4, cfg.numbers.length),
    new MeshBasicMaterial(faceTexture ? { map: faceTexture } : { color: 0x22242e }),
  );
  disc.name = 'wheel_face';
  disc.rotation.x = -Math.PI / 2;
  const rim = new Mesh(new TorusGeometry(4, 0.3, 12, 48), new MeshBasicMaterial({ color: 0x8a6b2f }));
  rim.rotation.x = Math.PI / 2;
  rim.name = 'wheel_rim';
  const spinGroup = new Group();
  spinGroup.name = 'wheel_spin';
  spinGroup.add(disc);
  group.add(rim, spinGroup);

  const balls: Mesh[] = [];
  for (let i = 0; i < 1; i++) {
    const ball = new Mesh(new SphereGeometry(0.3, 12, 8), new MeshBasicMaterial({ color: 0xf0f2f5 }));
    ball.name = `ball_${i}`;
    balls.push(ball);
    group.add(ball);
  }

  const slotNumbers = [...cfg.numbers];
  const n = slotNumbers.length;
  const slotArc = SLOT_RADIANS / n;

  return {
    group,
    faceOps,
    faceTexture,
    slotNumbers,
    setWheelAngle(a: number): void {
      spinGroup.rotation.y = a;
    },
    setBallPositions(angles: number[]): void {
      for (let i = 0; i < balls.length; i++) {
        const ang = angles[i] ?? 0;
        balls[i].position.set(Math.cos(ang) * 3.6, 0.6, Math.sin(ang) * 3.6);
      }
    },
    /** Number whose wedge is at the ball reference angle, using the engine's
     * §5.1 formula: idx = floor((ball−wheel+half)/slotWidth) mod n (slot 0 is
     * centered on wheel-relative angle 0). TEST-010: the FINAL telemetry
     * frame's slot equals BallLanded. */
    settledSlot(ballAngle: number, wheelAngle: number): number {
      const TAU = 2 * Math.PI;
      const rel = (((ballAngle - wheelAngle) % TAU) + TAU) % TAU;
      const idx = Math.floor((rel + slotArc / 2) / slotArc) % n;
      return slotNumbers[idx];
    },
  };
}

/** Consume spin_telemetry bytes: returns per-frame {wheelAngle, ballAngles} in
 * playback order (120 Hz fixed dt). Returns [] when no frames are available
 * (e.g. no spin has resolved yet). */
export function telemetryFrames(bytes: Uint8Array): Array<{ wheelAngle: number; ballAngles: number[] }> {
  if (bytes.byteLength < 16) return [];
  const { frames } = parseTelemetry(bytes);
  return frames.map((f) => ({
    wheelAngle: f[0],
    ballAngles: Array.from(f.slice(1)),
  }));
}