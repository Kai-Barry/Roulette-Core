/**
 * Minimal virtual-DOM for the DOM UI (Phase 3, TASK-015).
 *
 * Screens render to a plain vnode tree (pure data, PAT-003) that:
 *  - Node tests query directly via data-* attributes (no browser needed);
 *  - the browser mounts via `mount()` below.
 * No CSS-animation timing is ever consulted (headless determinism).
 */

export type VAttrs = Record<string, string | number | boolean | undefined>;
export interface VNode {
  tag: string;
  attrs: VAttrs;
  children: (VNode | string)[];
}

export function h(tag: string, attrs: VAttrs = {}, ...children: unknown[]): VNode {
  const flat: (VNode | string)[] = [];
  const push = (cs: unknown[]) => {
    for (const c of cs) {
      if (Array.isArray(c)) push(c);
      else if (typeof c === 'string') flat.push(c);
      else if (c !== null && c !== undefined && c !== false) flat.push(c as VNode);
    }
  };
  push(children);
  return { tag, attrs, children: flat };
}

/** Depth-first search for the first vnode with `data-test === testId`. */
export function query(root: VNode, testId: string): VNode | null {
  if (root.attrs['data-test'] === testId) return root;
  for (const c of root.children) {
    if (typeof c === 'string') continue;
    const hit = query(c, testId);
    if (hit) return hit;
  }
  return null;
}

export function queryAll(root: VNode, testId: string): VNode[] {
  const out: VNode[] = [];
  walk(root, (v) => {
    if (v.attrs['data-test'] === testId) out.push(v);
  });
  return out;
}

export function walk(v: VNode, fn: (v: VNode) => void): void {
  fn(v);
  for (const c of v.children) if (typeof c !== 'string') walk(c, fn);
}

/** Text content of a subtree (concatenated string children). */
export function text(root: VNode): string {
  let out = '';
  walk(root, (v) => {
    for (const c of v.children) if (typeof c === 'string') out += c;
  });
  return out;
}

/** Query by any single data-* attribute. */
export function byAttr(root: VNode, attr: string, value: string): VNode | null {
  let hit: VNode | null = null;
  walk(root, (v) => {
    if (hit === null && v.attrs[attr] === value) hit = v;
  });
  return hit;
}

/** Browser mount: render a vnode tree into a real DOM element. */
export function mount(v: VNode, el: HTMLElement): void {
  el.replaceChildren(toDom(v));
}

function toDom(v: VNode): HTMLElement | Text {
  if (typeof v === 'string') return document.createTextNode(v);
  const el = document.createElement(v.tag);
  for (const [k, val] of Object.entries(v.attrs)) {
    if (val === undefined || val === false) continue;
    el.setAttribute(k, val === true ? '' : String(val));
  }
  for (const c of v.children) {
    const child = typeof c === 'string' ? document.createTextNode(c) : toDom(c);
    el.appendChild(child);
  }
  return el;
}