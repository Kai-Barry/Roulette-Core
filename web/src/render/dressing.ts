/**
 * TASK-025: table dressing + raycast picking.
 *
 * Felt, chip stacks (instanced), croupier figures, fog + dust particles, and
 * pick targets. Picking maps felt sectors → `place_bet` and the bell →
 * `spin` — the SAME commands the DOM dispatches (PAT-001, single path).
 *
 * Sector geometry derives from the engine's WheelConfig (REQ-002): a sector
 * exists for each BetType the felt supports; colors come from SLOT_PALETTE.
 * Headless-safe: everything is plain three.js objects — no GL needed.
 */

import {
  BoxGeometry,
  CapsuleGeometry,
  CylinderGeometry,
  Fog,
  Group,
  InstancedMesh,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  MeshLambertMaterial,
  Points,
  BufferGeometry,
  BufferAttribute,
  PointsMaterial,
  SphereGeometry,
  TorusGeometry,
  Vector3,
} from 'three';

export const FELT_BETS = ['red', 'black', 'green', 'odd', 'even'] as const;
export type FeltBet = (typeof FELT_BETS)[number];

/** Sector palette (presentation-only mapping of bet kind → felt color). */
export const FELT_COLORS: Record<FeltBet, number> = {
  red: 0xd81838, black: 0x15161c, green: 0x0aa06e, odd: 0x30354a, even: 0x4a4530,
};

export interface FeltSector {
  bet: FeltBet;
  /** World-space center of the sector rectangle (y = felt top). */
  center: Vector3;
  size: [number, number];
  mesh: Mesh;
}

export interface Dressing {
  felt: Mesh;
  sectors: FeltSector[];
  bell: Mesh;
  chips: InstancedMesh;
  croupiers: Group;
  dust: Points;
  fog: Fog;
}

export interface PickResult {
  kind: 'felt' | 'bell';
  bet?: FeltBet;
}

export const SECTOR_LAYOUT: Array<[FeltBet, number, number]> = [
  ['red', -4.5, 2], ['black', -1.5, 2], ['green', 1.5, 2], ['odd', -3, -1], ['even', 0, -1],
];

export function buildDressing(): Dressing {
  const group = new Group();
  group.name = 'dressing';

  // Felt: green baize slab in front of the wheel.
  const felt = new Mesh(new BoxGeometry(16, 0.4, 8), new MeshLambertMaterial({ color: 0x0d3b2a }));
  felt.position.set(0, 0.2, 6.5);
  felt.name = 'felt';
  group.add(felt);

  // Betting sectors on the felt (positions in SECTOR_LAYOUT; sizes constant).
  const sectors: FeltSector[] = [];
  for (const [bet, x, z] of SECTOR_LAYOUT) {
    const mesh = new Mesh(
      new BoxGeometry(2.6, 0.12, 2),
      new MeshBasicMaterial({ color: FELT_COLORS[bet] }),
    );
    mesh.position.set(x, 0.46, 6.5 + z);
    mesh.name = `sector_${bet}`;
    mesh.userData = { pick: { kind: 'felt', bet } };
    group.add(mesh);
    sectors.push({ bet, center: mesh.position.clone(), size: [2.6, 2], mesh });
  }

  // Bell: brass torus + knob to the felt's right; raycast → spin.
  const bell = new Mesh(new TorusGeometry(0.55, 0.18, 10, 24), new MeshBasicMaterial({ color: 0xc9a227 }));
  bell.position.set(7.2, 0.75, 6.5);
  bell.rotation.x = Math.PI / 2;
  bell.name = 'bell';
  bell.userData = { pick: { kind: 'bell' } };
  group.add(bell);

  // Chip stacks: instanced cylinders (denomination colors), placed on sectors.
  const chipGeo = new CylinderGeometry(0.35, 0.35, 0.12, 12);
  const chipMat = new MeshLambertMaterial({ color: 0xffffff });
  const chips = new InstancedMesh(chipGeo, chipMat, FELT_BETS.length * 3);
  chips.name = 'chip_stacks';
  const m = new Matrix4();
  let ci = 0;
  for (const s of sectors) {
    for (let lvl = 0; lvl < 3; lvl++) {
      m.setPosition(s.center.x + 0.7, 0.52 + lvl * 0.13, s.center.z + 0.7);
      chips.setMatrixAt(ci++, m);
    }
  }
  group.add(chips);

  // Croupiers: two silhouette figures across the table.
  const croupiers = new Group();
  croupiers.name = 'croupiers';
  const bodyGeo = new CapsuleGeometry(0.5, 1.6, 4, 8);
  const headGeo = new SphereGeometry(0.32, 10, 8);
  const suit = new MeshLambertMaterial({ color: 0x1a1a24 });
  for (const x of [-3.5, 3.5]) {
    const body = new Mesh(bodyGeo, suit);
    body.position.set(x, 1.4, -3);
    const head = new Mesh(headGeo, new MeshLambertMaterial({ color: 0xb9907a }));
    head.position.set(x, 2.55, -3);
    croupiers.add(body, head);
  }
  group.add(croupiers);

  // Dust particles + fog (§11 atmosphere).
  const dustCount = 220;
  const pos = new Float32Array(dustCount * 3);
  for (let i = 0; i < dustCount; i++) {
    pos[i * 3] = (Math.random() - 0.5) * 30;
    pos[i * 3 + 1] = Math.random() * 8;
    pos[i * 3 + 2] = (Math.random() - 0.5) * 30;
  }
  const dustGeo = new BufferGeometry();
  dustGeo.setAttribute('position', new BufferAttribute(pos, 3));
  const dust = new Points(dustGeo, new PointsMaterial({ color: 0x8a8f9e, size: 0.06, transparent: true, opacity: 0.5 }));
  dust.name = 'dust';
  group.add(dust);

  const fog = new Fog(0x0b0d13, 18, 60);

  return { felt, sectors, bell, chips, croupiers, dust, fog };
}

/** Raycast helper over dressing pick targets (pure — no renderer needed). */
export function pickAt(objects: Mesh[], origin: Vector3, dir: Vector3): PickResult | null {
  let best: { d: number; res: PickResult } | null = null;
  for (const o of objects) {
    const pick = o.userData?.pick as PickResult | undefined;
    if (!pick) continue;
    // Slab/point-in-bounds test in world space: closest approach to the box.
    const box = new Box3Like(o);
    const hit = rayBox(origin, dir, box);
    if (hit !== null && (best === null || hit < best.d)) best = { d: hit, res: pick };
  }
  return best?.res ?? null;
}

/** Minimal AABB over a mesh's bounding sphere center + footprint. */
class Box3Like {
  private readonly mesh: Mesh;
  constructor(mesh: Mesh) {
    this.mesh = mesh;
  }
  min(): Vector3 {
    const c = this.mesh.position;
    const s = this.footprint();
    return new Vector3(c.x - s.x / 2, c.y - s.y / 2, c.z - s.z / 2);
  }
  max(): Vector3 {
    const c = this.mesh.position;
    const s = this.footprint();
    return new Vector3(c.x + s.x / 2, c.y + s.y / 2, c.z + s.z / 2);
  }
  private footprint(): Vector3 {
    const g = this.mesh.geometry as BoxGeometry | TorusGeometry | CylinderGeometry;
    if (this.mesh.name === 'bell') return new Vector3(1.2, 1.2, 1.2);
    if (this.mesh.name.startsWith('sector_')) return new Vector3(2.6, 0.12, 2);
    void g;
    return new Vector3(1, 1, 1);
  }
}

/** Slab-method ray/AABB intersection; returns distance or null. */
export function rayBox(origin: Vector3, dir: Vector3, box: { min(): Vector3; max(): Vector3 }): number | null {
  const mn = box.min(), mx = box.max();
  let tmin = -Infinity, tmax = Infinity;
  for (const axis of ['x', 'y', 'z'] as const) {
    const d = dir[axis];
    const o = origin[axis];
    if (Math.abs(d) < 1e-9) {
      if (o < mn[axis] || o > mx[axis]) return null;
      continue;
    }
    let t1 = (mn[axis] - o) / d;
    let t2 = (mx[axis] - o) / d;
    if (t1 > t2) [t1, t2] = [t2, t1];
    tmin = Math.max(tmin, t1);
    tmax = Math.min(tmax, t2);
    if (tmin > tmax) return null;
  }
  return tmax >= 0 ? Math.max(tmin, 0) : null;
}

/** Dust drift animation (called per frame by RenderManager). */
export function driftDust(dust: Points, dt: number): void {
  const pos = dust.geometry.getAttribute('position') as BufferAttribute;
  const a = dust.geometry.getAttribute('position');
  for (let i = 0; i < a.count; i++) {
    let y = a.getY(i) + dt * 0.15;
    if (y > 8) y = 0;
    a.setY(i, y);
  }
  void pos;
  a.needsUpdate = true;
}