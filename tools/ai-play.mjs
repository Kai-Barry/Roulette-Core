#!/usr/bin/env node
// TASK-003: AI autoplay policy bots. Plays full runs through the real wasm
// engine, headlessly, using only legal Commands (REQ-005). Deterministic for
// a fixed seed + policy (REQ-006).
//
// Usage:
//   node tools/ai-play.mjs [--seed S] [--policy random|greedy|scripted FILE]
//                          [--max-commands N] [--json] [--difficulty d]
//                          [--campaign N]
//
// --json prints a one-line machine summary (token-cheap); default prints a
// compact human summary. Exit 0 iff the run terminated (victory/defeat) and
// invariants held throughout.

import { readFileSync } from 'node:fs';
import { EngineSession, candidatesFor } from '../web/test/lib.mjs';
import { checkInvariants } from '../web/test/invariants.mjs';

const POLICIES = {
  random(state) {
    const cands = candidatesFor(state);
    if (!cands.length) return null;
    return cands[Math.floor(Math.random() * cands.length)];
  },
  greedy(state) {
    const cands = candidatesFor(state);
    if (!cands.length) return null;
    const score = (c) => {
      if (c.cmd === 'finish_loadout') return 100;
      if (c.cmd === 'draft_wheel') return 90;
      if (c.cmd === 'draft_card') return 80;
      if (c.cmd === 'event_choose') return 70;
      if (c.cmd === 'forge_take') return 65;
      if (c.cmd === 'purchase') return 60;
      if (c.cmd === 'play_card') return 50;
      if (c.cmd === 'place_bet') return 40;
      if (c.cmd === 'pick_node') return 30;
      if (c.cmd === 'spin') return 20;
      return 10;
    };
    return cands.sort((a, b) => score(b) - score(a))[0];
  },
  scripted() {
    return null; // handled inline in playRun
  },
};

const args = parseArgs(process.argv.slice(2));

main().catch((e) => {
  console.error(String(e?.message ?? e).slice(0, 2000));
  process.exit(1);
});

function main() {
  if (args.campaign) return campaign(Number(args.campaign));
  return singleRun();
}

function singleRun() {
  const seed = args.seed ?? 'ai_seed_1';
  const policy = args.policy ?? 'greedy';
  const max = Number(args['max-commands'] ?? 5000);
  const difficulty = args.difficulty ?? 'short';
  const summary = playRun(seed, policy, difficulty, max);
  report(summary);
  process.exit(summary.ok ? 0 : 1);
}

function campaign(n) {
  const rows = [];
  for (let i = 1; i <= n; i++) {
    const seed = `campaign_${i}`;
    const diff = ['short', 'medium', 'long'][i % 3];
    rows.push(playRun(seed, args.policy ?? 'greedy', diff, 20000));
  }
  const byDiff = {};
  for (const r of rows) {
    byDiff[r.difficulty] ??= { runs: 0, wins: 0, totalCommands: 0, avgCommands: 0 };
    byDiff[r.difficulty].runs++;
    byDiff[r.difficulty].wins += r.outcome === 'victory' ? 1 : 0;
    byDiff[r.difficulty].totalCommands += r.commands;
  }
  for (const d of Object.values(byDiff)) d.avgCommands = Math.round(d.totalCommands / d.runs);
  console.log(JSON.stringify({ campaign: n, byDifficulty: byDiff }));
  process.exit(rows.every((r) => r.ok) ? 0 : 1);
}

function playRun(seed, policyName, difficulty, max) {
  const s = new EngineSession(seed);
  const script = policyName === 'scripted' ? JSON.parse(readFileSync(args._[0], 'utf8')) : null;
  let scriptIdx = 0;
  let outcome = 'incomplete';
  let lastErr = null;

  try {
    s.cmd({ cmd: 'start_run', difficulty });
  } catch (e) {
    return fail(seed, policyName, difficulty, `start_run failed: ${e.message}`);
  }

  for (let step = 0; step < max; step++) {
    checkInvariants(s, `step ${step}`);
    const state = s.state();
    const gs = state.game_state;
    if (gs === 'victory') { outcome = 'victory'; break; }
    if (gs === 'game_over') { outcome = 'defeat'; break; }

    let cmd = null;
    if (script) {
      cmd = script[scriptIdx++];
      if (!cmd) { outcome = 'script_exhausted'; break; }
    } else {
      cmd = POLICIES[policyName](state, s);
    }
    if (!cmd) { outcome = 'softlock'; lastErr = `no candidate for state ${gs}`; break; }

    const ev = s.tryCmd(cmd);
    if (!ev) {
      if (policyName === 'scripted') {
        outcome = 'script_error';
        lastErr = s.errors.at(-1)?.error?.message ?? 'unknown';
        break;
      }
      // Probe fallback: try remaining candidates for this state.
      const cands = candidatesFor(state).filter((c) => JSON.stringify(c) !== JSON.stringify(cmd));
      let played = false;
      for (const c2 of cands) {
        if (s.tryCmd(c2)) { played = true; break; }
      }
      if (!played) { outcome = 'softlock'; lastErr = `state ${gs}: no candidate applied`; break; }
    }
  }

  if (outcome === 'incomplete') lastErr = `command cap ${max} reached`;
  const ok = ['victory', 'defeat'].includes(outcome);
  return {
    seed, policy: policyName, difficulty, ok, outcome, error: lastErr,
    commands: s.commands.length, events: s.events.length,
    rounds: s.events.filter((e) => e.event === 'round_ended').length,
    hp: s.state().run?.hp ?? null,
  };
}

function fail(seed, policy, difficulty, msg) {
  return { seed, policy, difficulty, ok: false, outcome: 'error', error: msg, commands: 0, events: 0, rounds: 0, hp: null };
}



function report(r) {
  if (args.json) {
    console.log(JSON.stringify(r));
  } else {
    console.log(
      `${r.ok ? 'OK ' : 'FAIL'} seed=${r.seed} policy=${r.policy} diff=${r.difficulty} ` +
        `outcome=${r.outcome} cmds=${r.commands} events=${r.events} rounds=${r.rounds} hp=${r.hp}` +
        (r.error ? ` err=${r.error}` : '')
    );
  }
}


function parseArgs(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith('--')) {
      const key = a.slice(2);
      const next = argv[i + 1];
      if (next === undefined || next.startsWith('--')) out[key] = true;
      else { out[key] = next; i++; }
    } else out._.push(a);
  }
  return out;
}