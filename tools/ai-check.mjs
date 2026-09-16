#!/usr/bin/env node
// TASK-005 / GUD-001: the AI-playtest gate command. Boots the real wasm
// engine headlessly and verifies the phase's invariants:
//   1. boot spike + matchers + invariants smoke (TASK-001/002/004)
//   2. random policy completes a run (terminates, invariants hold)
//   3. greedy policy completes a run (terminates, invariants hold)
// Zero exit on success; ≤ 20 lines of output (CON-004 token economy).
// Usage: npm run ai:check   →   node tools/ai-check.mjs [extra ai-play args…]

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const seeds = [1, 2, 3];
let failed = false;

function run(label, cmd, cmdArgs) {
  const r = spawnSync('node', [cmd, ...cmdArgs], { cwd: root, encoding: 'utf8' });
  const out = (r.stdout + r.stderr).trim();
  if (r.status !== 0) {
    failed = true;
    console.log(`FAIL ${label} (exit ${r.status})`);
    console.log(out.split('\n').slice(-12).join('\n'));
    return null;
  }
  return out;
}

// 1. Smoke: boot + matchers + invariants against the real bridge.
run('smoke', 'web/test/node-wasm.mjs', []);

// 2. Policy bots, seeds 1–3.
for (const policy of ['random', 'greedy']) {
  for (const seed of seeds) {
    const out = run(`${policy} s${seed}`, 'tools/ai-play.mjs', [
      '--seed', String(seed),
      '--policy', policy,
      '--json',
      '--difficulty', 'short',
    ]);
    if (!out) continue;
    const summary = out.split('\n').filter(Boolean).pop();
    console.log(`ok ${policy} s${seed}: ${summary}`);
  }
}

if (failed) process.exit(1);
console.log('ai:check PASS');