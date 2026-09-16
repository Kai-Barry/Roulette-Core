/**
 * TASK-023: RenderManager — three.js scene bootstrap + §11.3 nine-camera rig,
 * wired to the virtual ScreenManager via state projections (REQ-002: the
 * render layer invents nothing; every visual parameter comes from engine state).
 *
 * `?no3d=1` (or Node/headless) constructs the full scene graph but skips the
 * WebGLRenderer entirely — the graph is introspectable for gates.
 */

import {
  AmbientLight,
  DirectionalLight,
  Group,
  Mesh,
  OrthographicCamera,
  PerspectiveCamera,
  PlaneGeometry,
  Scene,
  SphereGeometry,
  BoxGeometry,
  MeshBasicMaterial,
  MeshLambertMaterial,
  Vector3,
  WebGLRenderer,
  WebGLRenderTarget,
  type Mesh as TMesh,
  type Object3D,
} from 'three';
import { createPs1Pass, type Ps1Pass } from './PS1Shader.ts';
import { buildWheelVisual, telemetryFrames, type WheelVisual } from './WheelVisual.ts';
import { buildDressing, driftDust, pickAt, type Dressing, type FeltBet, type PickResult } from './dressing.ts';
import { buildFx, setIntent, applyEvents, tickFx, fxSummary, specialBurstFor, type FxObjects } from './fx.ts';
import { combatView } from '../client/projections.ts';
import type { GameStateJson, WheelConfig } from '../engine/schema.ts';

/** Minimal client surface the renderer needs (real GameClient satisfies it). */
export interface GameClientLike {
  state(): GameStateJson;
}

/** §11.3: nine fixed rig angles around the wheel table. */
export const RIG_NAMES = [
  'table_front', 'table_left', 'table_right',
  'wheel_close', 'wheel_top', 'wheel_orbit',
  'board_wide', 'enemy_side', 'dramatic_low',
] as const;
export type RigName = (typeof RIG_NAMES)[number];

export interface Rig {
  name: RigName;
  camera: PerspectiveCamera;
  /** Where this camera looks (table center by default). */
  target: Vector3;
}

export class RenderManager {
  readonly scene = new Scene();
  readonly rig: Rig[] = [];
  readonly ps1: Ps1Pass;
  readonly wheelGroup = new Group();
  readonly ballMesh: TMesh;
  readonly tableMesh: TMesh;
  readonly enemyTableMesh: TMesh;
  active: RigName = 'table_front';
  private client: GameClientLike;
  /** Last landing number forwarded from the engine (never invented). */
  private landedNumber: number | null = null;
  /** Real GL context only when the browser mounts one (never in Node). */
  readonly renderer: WebGLRenderer | null;
  private readonly glTarget: WebGLRenderTarget | null = null;
  private readonly glQuadScene: Scene | null = null;
  private readonly glQuadCam: OrthographicCamera | null = null;
  /** Engine-config wheel visual (TASK-024); null until attachWheel. */
  private wheel: WheelVisual | null = null;
  private wheelCfgKey: string | null = null;
  /** Telemetry playback (TASK-024): frames from spin_telemetry only. */
  private playFrames: Array<{ wheelAngle: number; ballAngles: number[] }> = [];
  private playCursor = 0;
  private playSide: 0 | 1 = 0;
  /** TASK-025 dressing (felt/sectors/bell/chips/croupiers/dust/fog). */
  readonly dressing: Dressing;
  private dustTime = 0;
  /** TASK-026 fx layer (bursts + intent ring). */
  readonly fxGroup = buildFx();
  fx: FxObjects = this.fxGroup.fx;
  /** Last attached wheel config, for fx color derivation (REQ-002). */
  private wheelCfg: WheelConfig | null = null;

  constructor(client: GameClientLike, opts?: { no3d?: boolean; webgl?: boolean; canvas?: HTMLCanvasElement }) {
    this.client = client;
    this.ps1 = createPs1Pass(true);

    // Table: a dark box the wheel sits on.
    this.tableMesh = new Mesh(new BoxGeometry(30, 1, 18), new MeshLambertMaterial({ color: 0x14161f }));
    this.tableMesh.position.set(0, -0.5, 0);
    this.enemyTableMesh = new Mesh(new BoxGeometry(30, 1, 18), new MeshLambertMaterial({ color: 0x1a0d12 }));
    this.enemyTableMesh.position.set(0, -0.5, -26);

    // Wheel placeholder (real geometry: TASK-024 WheelVisual).
    this.wheelGroup.position.set(0, 2, 0);
    const wheelDisc = new Mesh(new SphereGeometry(4, 24, 12), new MeshBasicMaterial({ color: 0x22242e }));
    wheelDisc.scale.y = 0.25;
    this.wheelGroup.add(wheelDisc);
    this.ballMesh = new Mesh(new SphereGeometry(0.35, 12, 8), new MeshBasicMaterial({ color: 0xf0f2f5 }));
    this.ballMesh.position.set(0, 2.6, 0);

    this.scene.add(new AmbientLight(0xffffff, 0.55));
    const key = new DirectionalLight(0xffffff, 1.1);
    key.position.set(6, 14, 8);
    this.scene.add(key, this.tableMesh, this.enemyTableMesh, this.wheelGroup, this.ballMesh);

    // TASK-025: felt, sectors, bell, chip stacks, croupiers, dust, fog.
    this.dressing = buildDressing();
    this.scene.add(this.dressing.felt.parent ?? this.dressing.felt);
    if (!this.scene.fog) this.scene.fog = this.dressing.fog;

    // TASK-026: fx layer.
    this.scene.add(this.fxGroup.group);

    this.buildRig();
    if (opts?.webgl && opts.canvas && !opts.no3d && typeof document !== 'undefined') {
      this.renderer = new WebGLRenderer({ canvas: opts.canvas, antialias: false });
      this.renderer.setPixelRatio(1);
      this.renderer.setSize(opts.canvas.clientWidth || 640, opts.canvas.clientHeight || 360, false);
      this.glTarget = new WebGLRenderTarget(320, 180, { depthBuffer: true });
      this.ps1.setResolution(320, 180);
      this.ps1.material.uniforms.tDiffuse.value = this.glTarget.texture;
      this.glQuadScene = new Scene();
      this.glQuadScene.add(new Mesh(new PlaneGeometry(2, 2), this.ps1.material));
      this.glQuadCam = new OrthographicCamera(-1, 1, 1, -1, 0, 1);
    } else {
      this.renderer = null;
    }
  }

  /** §11.3 nine-camera rig: deterministic positions looking at the table. */
  private buildRig(): void {
    const center = new Vector3(0, 2, 0);
    const defs: Array<[RigName, Vector3]> = [
      ['table_front', new Vector3(0, 8, 16)],
      ['table_left', new Vector3(-16, 8, 8)],
      ['table_right', new Vector3(16, 8, 8)],
      ['wheel_close', new Vector3(0, 6, 7)],
      ['wheel_top', new Vector3(0, 18, 0.001)],
      ['wheel_orbit', new Vector3(10, 10, 10)],
      ['board_wide', new Vector3(0, 22, 24)],
      ['enemy_side', new Vector3(0, 7, -14)],
      ['dramatic_low', new Vector3(4, 1.2, 9)],
    ];
    for (const [name, pos] of defs) {
      const camera = new PerspectiveCamera(name === 'wheel_close' ? 45 : 55, 16 / 9, 0.1, 200);
      camera.position.copy(pos);
      camera.lookAt(center);
      this.rig.push({ name, camera, target: center.clone() });
    }
  }

  camera(name: RigName): PerspectiveCamera {
    const r = this.rig.find((x) => x.name === name);
    if (!r) throw new Error(`unknown rig camera: ${name}`);
    return r.camera;
  }

  /** Engine-forwarded landing (from BallLanded) — render never computes its own. */
  onBallLanded(n: number): void {
    this.landedNumber = n;
  }

  get lastLanded(): number | null {
    return this.landedNumber;
  }

  /** TASK-024: mount a wheel visual built from the engine's WheelConfig
   * (REQ-002 — slots come only from state). Rebuilds only when the config
   * actually changes (forge ops mutate slot colors). */
  attachWheel(cfg: WheelConfig): void {
    const key = JSON.stringify(cfg);
    if (this.wheel && this.wheelCfgKey === key) return;
    this.wheelCfgKey = key;
    this.wheelCfg = cfg;
    if (this.wheel) {
      this.wheelGroup.remove(this.wheel.group);
    }
    this.wheel = buildWheelVisual(cfg);
    this.wheel.group.position.set(0, 2, 0);
    this.wheelGroup.add(this.wheel.group);
  }

  get wheelAttached(): boolean {
    return this.wheel !== null;
  }

  get wheelSlotCount(): number | null {
    return this.wheel?.slotNumbers.length ?? null;
  }

  /** Direct handle for gates asserting the settled slot (TEST-010). */
  wheelVisual(): WheelVisual | null {
    return this.wheel;
  }

  /** TASK-024 playback: consume spin_telemetry bytes for a spin that just
   * resolved. Frames play back at fixed dt (1/120 s per frame); the LAST
   * frame equals BallLanded by construction (TEST-010). */
  playSpin(side: 0 | 1, bytes: Uint8Array): void {
    const frames = telemetryFrames(bytes);
    if (frames.length === 0) return;
    this.playFrames = frames;
    this.playCursor = 0;
    this.playSide = side;
  }

  get playbackActive(): boolean {
    return this.playCursor < this.playFrames.length;
  }

  get playbackFrame(): number {
    return this.playCursor;
  }

  /** Advance telemetry playback; dt seconds → fixed-dt frame steps.
   * onPlaybackFrame (TASK-029) fires once per advanced frame index so
   * SoundManager can sync the §5.4 click/bounce track to the cursor. */
  onPlaybackFrame: ((frame: number) => void) | null = null;
  private lastPlaybackFrame = -1;
  tick(dt: number): void {
    if (!this.playbackActive) return;
    const steps = Math.max(1, Math.round(dt * 120));
    this.playCursor = Math.min(this.playFrames.length, this.playCursor + steps);
    const f = this.playFrames[Math.max(0, this.playCursor - 1)];
    const wv = this.wheel;
    if (wv) {
      wv.setWheelAngle(f.wheelAngle);
      wv.setBallPositions(f.ballAngles);
    }
    // Placeholder ball mirrors the primary ball while the wheel is unmounted.
    if (f.ballAngles.length > 0) {
      const a = f.ballAngles[0];
      this.ballMesh.position.set(Math.cos(a) * 3.6, 2.6, Math.sin(a) * 3.6);
    }
    if (this.playCursor !== this.lastPlaybackFrame) {
      this.lastPlaybackFrame = this.playCursor;
      this.onPlaybackFrame?.(this.playCursor);
    }
  }

  /** TASK-025: raycast the dressing from an active camera ray (screen NDC).
   * Returns the same `Command` inputs the DOM would dispatch (PAT-001). */
  pick(kind: 'felt' | 'bell', bet?: FeltBet, origin?: Vector3, dir?: Vector3): PickResult | null {
    void kind; void bet;
    const pickables = [...this.dressing.sectors.map((s) => s.mesh), this.dressing.bell];
    if (origin && dir) return pickAt(pickables, origin, dir);
    return null;
  }

  /** Direct sector lookup for tests/headless audits. */
  sector(bet: FeltBet): PickResult | null {
    const s = this.dressing.sectors.find((x) => x.bet === bet);
    return s ? { kind: 'felt', bet } : null;
  }

  /** Atmosphere update (dust drift). Cheap; called from the RAF loop. */
  private animate(dt: number): void {
    this.dustTime += dt;
    void this.dustTime;
    driftDust(this.dressing.dust, dt);
  }

  /** Render one frame: low-res scene → PS1 pass → canvas (TASK-023). */
  render(): void {
    if (!this.renderer || !this.glTarget || !this.glQuadScene || !this.glQuadCam) return;
    const cam = this.camera(this.active);
    this.renderer.setRenderTarget(this.glTarget);
    this.renderer.render(this.scene, cam);
    this.renderer.setRenderTarget(null);
    this.renderer.render(this.glQuadScene, this.glQuadCam);
  }

  /** Per-frame update from the RAF loop: playback + atmosphere + fx decay. */
  update(dt: number): void {
    this.tick(dt);
    this.animate(dt);
    tickFx(this.fx, dt);
    this.render();
  }

  /** TASK-026: feed an event batch to the fx layer (bursts from special
   * colors; intent cleared on execution). Requires an attached wheel config. */
  onEvents(events: Array<Record<string, unknown>>): void {
    if (!this.wheelCfg) return;
    applyEvents(this.fx, events as never, this.wheelCfg, (m) => this.fxGroup.group.add(m));
  }

  /** TASK-026: sync the intent telegraph from battle state (REQ-002: the
   * label is the engine's enemy_intent verbatim; null removes the ring). */
  syncIntent(): void {
    const b = this.client.state().battle;
    setIntent(this.fx, b?.enemy_intent ?? null, (m) => this.fxGroup.group.add(m));
  }

  /** Headless fx audit (gates read data, not pixels). */
  fxState(): { burst: string | null; intent: string | null } {
    return fxSummary(this.fx);
  }

  /** Pure helper exposed for tests: would this landing burst? */
  wouldBurst(side: string, number: number): boolean {
    return this.wheelCfg ? specialBurstFor({ side, number }, this.wheelCfg) !== null : false;
  }

  /** Sync visuals from current engine state (combat view). */
  syncFromState(): void {
    const state = this.client.state();
    if (state.game_state !== 'combat') return;
    const v = combatView(state);
    // Ball sits at rim while betting; parked at center after landing.
    if (this.landedNumber === null) {
      this.ballMesh.position.set(0, 2.6, 3.6);
    }
    void v;
  }

  /** Flat, AI-queriable summary of the render graph (gates, PAT-003). */
  graph(): {
    rig: RigName[]; meshes: number; landed: number | null; active: RigName;
    wheelSlots: number | null; playback: { active: boolean; frame: number; side: 0 | 1 };
    dressing: { sectors: FeltBet[]; bell: boolean; chipInstances: number; croupiers: number };
    fx: { burst: string | null; intent: string | null };
  } {
    let meshes = 0;
    const count = (o: Object3D) => {
      if ((o as Mesh).isMesh) meshes++;
      o.children.forEach(count);
    };
    count(this.scene);
    return {
      rig: this.rig.map((r) => r.name),
      meshes,
      landed: this.landedNumber,
      active: this.active,
      wheelSlots: this.wheelSlotCount,
      playback: { active: this.playbackActive, frame: this.playbackFrame, side: this.playSide },
      dressing: {
        sectors: this.dressing.sectors.map((s) => s.bet),
        bell: true, chipInstances: this.dressing.chips.count, croupiers: this.dressing.croupiers.children.length,
      },
      fx: this.fxState(),
    };
  }
}