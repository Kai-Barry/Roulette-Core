/**
 * TASK-022: Settings (§12.9) — three volume buses persisted to storage;
 * headless mode never touches audio (no AudioContext is constructed —
 * audio recipes are verified by existence + dispatch in Phase 4, TASK-028).
 */

import { h, type VNode } from './vscreen.ts';

export type Bus = 'music' | 'drone' | 'sfx';
export const BUSES: readonly Bus[] = ['music', 'drone', 'sfx'] as const;

/** §13.4 defaults from the blueprint. */
export const DEFAULT_VOLUMES: Record<Bus, number> = { music: 0.55, drone: 0.15, sfx: 0.8 };

const KEY = 'rt.settings.v1';

export interface SettingsJson {
  volumes: Record<Bus, number>;
  mobile: boolean;
}

export class SettingsStore {
  private storage: { getItem(k: string): string | null; setItem(k: string, v: string): void } | null;
  private data: SettingsJson;

  constructor(storage?: { getItem(k: string): string | null; setItem(k: string, v: string): void } | null) {
    this.storage = storage ?? null;
    this.data = { volumes: { ...DEFAULT_VOLUMES }, mobile: false };
    const raw = this.storage?.getItem(KEY) ?? null;
    if (raw) {
      try {
        const parsed = JSON.parse(raw) as Partial<SettingsJson>;
        if (parsed.volumes) {
          for (const b of BUSES) {
            const v = parsed.volumes[b];
            if (typeof v === 'number' && v >= 0 && v <= 1) this.data.volumes[b] = v;
          }
        }
        if (typeof parsed.mobile === 'boolean') this.data.mobile = parsed.mobile;
      } catch { /* corrupt settings fall back to defaults */ }
    }
  }

  volume(bus: Bus): number {
    return this.data.volumes[bus];
  }

  setVolume(bus: Bus, v: number): void {
    this.data.volumes[bus] = Math.min(1, Math.max(0, v));
    this.persist();
  }

  isMobile(): boolean {
    return this.data.mobile;
  }

  setMobile(m: boolean): void {
    this.data.mobile = m;
    this.persist();
  }

  private persist(): void {
    this.storage?.setItem(KEY, JSON.stringify(this.data));
  }
}

/** Settings panel vnode; bus steppers are button-based so headless audits
 * never need CSS/input timing. */
export function settingsScreen(settings: SettingsStore, dispatch: (b: Bus, delta: number) => void): VNode {
  return h('section', { 'data-screen': 'settings', 'data-test': 'screen-settings' },
    BUSES.map((bus) =>
      h('div', { 'data-test': `bus-${bus}`, 'data-volume': settings.volume(bus) },
        h('button', {
          'data-test': `${bus}-down`,
          onclick: () => dispatch(bus, -0.05),
        }, `${bus} −`),
        `${Math.round(settings.volume(bus) * 100)}%`,
        h('button', {
          'data-test': `${bus}-up`,
          onclick: () => dispatch(bus, 0.05),
        }, `${bus} +`))),
    h('button', {
      'data-test': 'toggle-mobile',
      'data-mobile': settings.isMobile(),
      onclick: () => settings.setMobile(!settings.isMobile()),
    }, settings.isMobile() ? 'mobile ON' : 'mobile OFF'),
  );
}