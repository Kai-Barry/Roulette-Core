// Invariant suite (TASK-004). Run after every state change in every
// ai:check run; each check is cheap (JSON reads) to keep token burn near zero.

export function checkInvariants(session, label = '') {
  const s = session.state();
  const run = s.run;
  const battle = s.battle;
  const fails = [];

  if (run) {
    if (run.chips < 0) fails.push(`chips negative: ${run.chips}`);
    if (run.hp < 0) fails.push(`hp negative: ${run.hp}`);
    if (run.max_hp && run.hp > run.max_hp) fails.push(`hp ${run.hp} > max ${run.max_hp}`);
    if (run.deck.length > 60) fails.push(`deck over limit: ${run.deck.length}`);
  }
  if (battle) {
    if (battle.bets) {
      const total = battle.bets.reduce((a, b) => a + b.amount, 0);
      if (battle.chips_pool < 0) fails.push(`battle chips_pool negative: ${battle.chips_pool}`);
      if (total && total > (battle.turn_start_pool ?? Infinity) + 1000) fails.push(`bets exceed pool: ${total}`);
    }
    if (battle.hand && battle.hand.length > 8) fails.push(`hand over limit: ${battle.hand.length}`);
    if (battle.round > (battle.max_rounds ?? 1e9) + 5) fails.push(`round ${battle.round} >> max ${battle.max_rounds} (sudden death ok)`);
  }
  if (s.undo_depth > 64) fails.push(`undo depth ${s.undo_depth} > 64`);

  // Event-ordering: spin_resolved requires a prior spin_started + ball_landed per side.
  const events = session.events;
  for (let i = 0; i < events.length; i++) {
    if (events[i].event === 'spin_resolved') {
      const prior = events.slice(0, i);
      const started = prior.findLast((e) => e.event === 'spin_started');
      const landed = prior.findLast((e) => e.event === 'ball_landed');
      if (!started) fails.push(`spin_resolved at ${i} without spin_started`);
      if (!landed) fails.push(`spin_resolved at ${i} without ball_landed`);
    }
  }
  if (fails.length) {
    throw new Error(`INVARIANTS FAILED ${label}:\n  ${fails.join('\n  ')}\nstate: ${JSON.stringify(compactState(s))}`);
  }
  return true;
}

function compactState(s) {
  return {
    game_state: s.game_state,
    chips: s.run?.chips,
    hp: s.run?.hp,
    round: s.battle?.round,
    phase: s.battle?.phase,
    undo_depth: s.undo_depth,
  };
}