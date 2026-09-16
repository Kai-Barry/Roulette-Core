/**
 * Browser entry (three.js client over the WASM engine).
 *
 * Loads `roulette_wasm_bg.wasm` (web-bindgen pkg, `telemetry` default feature),
 * boots the headless app shell (`boot()`), mounts the phase-3 DOM UI, and
 * attaches the phase-4 RenderManager. `?legacy=1` skips this module's mounts
 * so the standalone inline harness below keeps its behavior; `?no3d=1`
 * disables the 3D renderer (renderer escape hatch, TASK-023).
 *
 * Seed is fixed unless `?seed=` is provided — dev sessions are reproducible
 * (REQ-006 determinism starts at the entry point).
 */
import init, { new_engine } from '../../crates/roulette-wasm/pkg-web/roulette_wasm.js';
import { boot, installRtWindow, type RtApi } from './main.ts';
import { RenderManager } from './render/RenderManager.ts';
import { mount } from './ui/vscreen.ts';
import type { WheelConfig } from './engine/schema.ts';
import { SoundManager } from './audio/SoundManager.ts';
import { EncounterMusic } from './audio/music.ts';
import { Vector3 } from 'three';

async function main(): Promise<void> {
  const params = new URLSearchParams(window.location.search);
  await init();
  const rt: RtApi = boot({
    seed: params.get('seed') ?? 'demo',
    headless: params.has('headless'),
    // Explicit `?seed=` asks for a fresh reproducible run; otherwise resume
    // the autosave (command replay + state-hash verification, REQ-006).
    resume: !params.has('seed'),
    // The web pkg exports module-level fns, not a namespace object.
    wasm: { new_engine },
  });
  installRtWindow(rt);

  if (params.has('legacy')) return;

  // The module client owns the screen; the legacy inline harness stays beneath
  // (reachable only via ?legacy=1) and must not swallow pointer events.
  const style = document.createElement('style');
  style.textContent = [
    '#rt-shell { position: fixed; inset: 0; z-index: 5000; display: flex; flex-direction: column;',
    '  background: #0b0d13; color: #e8eaf2; font: 14px/1.45 system-ui, sans-serif; overflow: auto; }',
    '#rt-canvas { position: fixed; inset: 0; z-index: 4990; width: 100%; height: 100%; }',
    '#rt-err { position: fixed; bottom: 8px; left: 8px; z-index: 6000; margin: 0; padding: 8px;',
    '  color: #ff8a80; background: #1a0d12; border: 1px solid #ff8a80; font: 12px/1.4 monospace; }',
    '#rt-badge { position: fixed; top: 4px; right: 4px; z-index: 6001; color: #7ee787;',
    '  background: #10231a; border: 1px solid #7ee787; padding: 2px 6px; font: 11px monospace; }',
    '#rt-ui { margin: 0 auto; padding: 24px; max-width: 720px; width: 100%; }',
    '#rt-ui h1 { font-size: 28px; letter-spacing: 2px; margin: 0 0 4px; color: #ffd700;',
    '  text-shadow: 0 0 18px #ff2a4b66; }',
    '#rt-ui h2 { font-size: 20px; letter-spacing: 1px; margin: 0 0 12px; color: #e8eaf2; }',
    '#rt-ui section { display: flex; flex-direction: column; gap: 10px; }',
    '#rt-ui button { cursor: pointer; color: #e8eaf2; background: #161823; border: 1px solid #ffffff1a;',
    '  border-radius: 8px; padding: 8px 14px; font: 600 14px system-ui, sans-serif; text-align: left; }',
    '#rt-ui button:hover:not(:disabled) { border-color: #ff3b5c; background: #202434; }',
    '#rt-ui button:disabled { opacity: 0.45; cursor: not-allowed; }',
    '#rt-ui ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }',
    '#rt-ui li { display: flex; align-items: center; justify-content: space-between; gap: 10px;',
    '  background: #10121b; border: 1px solid #ffffff14; border-radius: 8px; padding: 8px 12px; }',
    '#rt-ui .rt-hud { display: flex; gap: 14px; flex-wrap: wrap; font: 12px/1.4 monospace; color: #8e95a5; }',
    '#rt-ui .rt-hud span { background: #10121b; border: 1px solid #ffffff14; border-radius: 6px; padding: 4px 8px; }',
    '#rt-ui .rt-map { position: relative; width: 100%; max-width: 560px; min-height: 380px;',
    '  background: #0e1018; border: 1px solid #ffffff14; border-radius: 12px; }',
    '#rt-ui .rt-map-node { position: absolute; transform: translate(-50%, -50%); text-align: center;',
    '  font: 11px monospace; padding: 6px 9px; }',
    '#rt-ui .rt-map-node[data-pickable="true"] { border-color: #ff3b5c; background: #3a1420;',
    '  box-shadow: 0 0 10px #ff3b5c44; }',
    '#rt-ui .rt-map-node[data-completed="true"] { border-color: #00e676; background: #0f2318; color: #b9f6ca; }',
    '#rt-ui .rt-map-node[data-pickable="false"] { opacity: 0.55; }',
    '#rt-ui [data-test="spin-report"] { font: 12px/1.5 monospace; color: #ffe57f;',
    '  background: #10121b; border: 1px solid #ffd70033; border-radius: 8px; padding: 8px 12px; }',
    '#rt-ui [data-test="prediction"] { font: 12px/1.5 monospace; color: #b388ff; }',
    '#rt-ui [data-test="combat-hud"], #rt-ui [data-test="hand"], #rt-ui [data-test="bet-board"] {',
    '  display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }',
  ].join('\n');
  document.head.appendChild(style);

  const badge = document.createElement('div');
  badge.id = 'rt-badge';
  badge.textContent = 'RT-MODULE-UI';
  document.body.appendChild(badge);

  window.onerror = (msg, _src, _line, _col, err) => {
    const el = document.createElement('pre');
    el.id = 'rt-boot-err';
    el.style.cssText = 'position:fixed;top:24px;right:4px;z-index:6001;margin:0;padding:6px;color:#ff8a80;background:#1a0d12;border:1px solid #ff8a80;font:11px/1.3 monospace;max-width:60vw;white-space:pre-wrap;';
    el.textContent = `window error: ${err?.stack ?? msg}`;
    document.body.appendChild(el);
  };

  // REQ-004 harness console: `__rt_eval(expr)` runs in page context and the
  // result lands in #rt-eval-out (browser gates read this, not screenshots).
  const evalOut = document.createElement('pre');
  evalOut.id = 'rt-eval-out';
  evalOut.style.cssText = 'position:fixed;bottom:4px;left:4px;z-index:6001;margin:0;padding:6px;max-width:70vw;max-height:30vh;overflow:auto;color:#b9f6ca;background:#0d1a12;border:1px solid #00e67655;font:11px/1.3 monospace;white-space:pre-wrap;display:none;';
  document.body.appendChild(evalOut);
  (window as unknown as Record<string, unknown>).__rt_eval = (expr: string): string => {
    try {
      const result = (0, eval)(expr);
      evalOut.style.display = 'block';
      evalOut.textContent = `RT> ${String(result)}`;
      return String(result);
    } catch (err) {
      evalOut.style.display = 'block';
      evalOut.textContent = `RT-ERR> ${String(err)}`;
      return `ERR: ${String(err)}`;
    }
  };
  // `?eval=` gate: run once at boot and surface the result in #rt-eval-out
  // (lets AI browser gates query the harness without a JS console).
  const evalParam = params.get('eval');
  if (evalParam) {
    setTimeout(() => {
      const fn = (window as unknown as Record<string, unknown>).__rt_eval as (e: string) => string;
      if (fn) fn(evalParam);
    }, 300);
  }

  const canvas = document.createElement('canvas');
  canvas.id = 'rt-canvas';
  document.body.appendChild(canvas);

  const shell = document.createElement('div');
  shell.id = 'rt-shell';
  document.body.appendChild(shell);

  const uiRoot = document.createElement('div');
  uiRoot.id = 'rt-ui';
  shell.appendChild(uiRoot);

  const renderUi = () => {
    try {
      mount(rt.tree(), uiRoot);
    } catch (err) {
      const el = document.createElement('pre');
      el.id = 'rt-err';
      el.textContent = `render error: ${String(err)}`;
      document.body.appendChild(el);
    }
  };
  rt.client.on((ev) => {
    renderUi();
    if (ev.kind === 'error') {
      let el = document.getElementById('rt-err');
      if (!el) {
        el = document.createElement('pre');
        el.id = 'rt-err';
        document.body.appendChild(el);
      }
      el.textContent = `dispatch error: ${ev.error.message ?? JSON.stringify(ev.error)}`;
    }
  });
  renderUi();

  const rm = new RenderManager(rt.client, params.has('no3d') ? { no3d: true } : { webgl: true, canvas });
  // Expose the render manager for browser-side gates (REQ-004 harness
  // surface): graph()/fxState() are queryable without screenshots.
  (rt as unknown as Record<string, unknown>).render = rm;

  // TASK-028: audio — three buses driven by the persisted settings store.
  // Headless-safe by construction (no AudioContext → recorded diagnostics).
  const sound = new SoundManager(rt.settings);
  const music = new EncounterMusic(sound);
  (rt as unknown as Record<string, unknown>).sound = sound;
  (rt as unknown as Record<string, unknown>).music = music;
  // First gesture unlocks autoplay-suspended contexts (§12.9).
  window.addEventListener('pointerdown', () => sound.unlock(), { once: true });
  // TASK-029: the renderer fires the click/bounce track as the playback
  // cursor crosses each sim event's frame (§5.4 sync).
  rm.onPlaybackFrame = (frame) => sound.tickPlayback(frame);

  // TASK-030: last engine-declared encounter tier (from battle_started).
  let lastBattleTier: string = 'normal';

  rt.client.on((ev) => {
    if (ev.kind === 'events') {
      let landed = false;
      let playerLanded = false;
      // TASK-030: encounter tier derives from the engine's own battle_started
      // event (REQ-002 analogue — no invented audio state).
      for (const e of ev.events) {
        if (e.event === 'battle_started') lastBattleTier = e.tier;
      }
      for (const e of ev.events) {
        if (e.event === 'ball_landed' && e.side === 'player' && typeof e.number === 'number') {
          rm.onBallLanded(e.number);
          landed = true;
          playerLanded = true;
        } else if (e.event === 'ball_landed' && e.side === 'enemy') {
          landed = true;
        }
      }
      // TASK-024: animate the resolved spin from the telemetry side channel
      // (REQ-008 — the event log itself stays float-free). TASK-029: queue
      // the timed sim events so clicks/bounces sync to the same playback
      // cursor. The enemy wheel reuses the shared visual playback (one
      // RenderManager playback at a time), so only side 0 is queued.
      if (landed && playerLanded &&
          typeof (rt.client as { spinTelemetry?: unknown }).spinTelemetry === 'function') {
        rm.playSpin(0, rt.client.spinTelemetry(0));
        sound.queueSimEvents(rt.client.spinSimEvents(0));
      }
      // TASK-028: §13.4 recipes react to the same event batch.
      sound.onEvents(ev.events);
      // TASK-030: encounter music keyed by the current battle.
      const battle = rt.client.state().battle;
      if (battle && !music.playing) {
        const boss = lastBattleTier === 'boss';
        const tier = lastBattleTier === 'elite' ? 2 : boss ? 4 : 0;
        music.start({ tier, boss });
      } else if (!battle && music.playing) {
        music.stop();
      }
      // TASK-026: fx react to the same event batch (special-color bursts).
      rm.onEvents(ev.events as never);
    }
    // TASK-024: mount the wheel straight from engine state (REQ-002 —
    // rebuilds automatically when forge ops change the config).
    const wheel = rt.client.state().battle?.player_wheel ?? rt.client.state().run?.player_wheel;
    if (wheel) rm.attachWheel(wheel as WheelConfig);
    // §11.3: camera keyed by game state (data-driven presentation).
    const gs = rt.client.state().game_state;
    rm.active = gs === 'combat' ? 'table_front' : gs === 'map' ? 'board_wide' : 'wheel_orbit';
    // TASK-026: intent telegraph tracks battle state.
    rm.syncIntent();
  });
  rm.syncFromState();
  const wheel0 = rt.client.state().battle?.player_wheel ?? rt.client.state().run?.player_wheel;
  if (wheel0) rm.attachWheel(wheel0 as WheelConfig);

  if (rm.renderer) {
    let last = performance.now();
    const frame = (now: number) => {
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      rm.update(dt);
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);

    // TASK-025 raycast picking: felt sectors → place_bet, bell → spin — the
    // same commands the DOM dispatches (PAT-001). Camera-space ray from NDC.
    let downNdc: { x: number; y: number } | null = null;
    const ndcOf = (e: PointerEvent) => ({
      x: (e.clientX / window.innerWidth) * 2 - 1,
      y: -(e.clientY / window.innerHeight) * 2 + 1,
    });
    const rayAt = (ndc: { x: number; y: number }) => {
      const cam = rm.camera(rm.active);
      const origin = cam.position.clone();
      const dir = new Vector3(ndc.x, ndc.y, 0.5).unproject(cam).sub(origin).normalize();
      return { origin, dir };
    };
    canvas.addEventListener('pointerdown', (e) => { downNdc = ndcOf(e); });
    canvas.addEventListener('pointerup', (e) => {
      if (!downNdc) return;
      const up = ndcOf(e);
      const moved = Math.hypot(up.x - downNdc.x, up.y - downNdc.y);
      downNdc = null;
      if (moved > 0.04) return; // drag, not a pick
      const { origin, dir } = rayAt(up);
      const hit = rm.pick('felt', undefined, origin, dir);
      if (!hit) return;
      if (hit.kind === 'felt' && hit.bet && rt.client.state().game_state === 'combat') {
        rt.dispatch({ cmd: 'place_bet', bet: hit.bet, amount: 1 });
      } else if (hit.kind === 'bell' && rt.client.state().game_state === 'combat') {
        rt.dispatch({ cmd: 'spin' });
      }
    });
  }
}

main().catch((e) => {
  console.error('[rt] boot failed:', e);
  document.body.insertAdjacentHTML(
    'beforeend',
    `<pre id="rt-boot-error">boot failed: ${String(e)}</pre>`,
  );
});