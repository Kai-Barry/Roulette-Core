// Golden event-tail matchers (TASK-002, PAT-002). Assertions match the tail
// of the serialized event log field-wise — never screenshots, never timing.

/** Assert the event log ENDS with the given sequence (subset match per event). */
export function expectEvents(log, expected, label = 'event tail') {
  const tail = log.slice(-expected.length);
  if (tail.length < expected.length) {
    throw new Error(`${label}: log has only ${log.length} events, need ${expected.length}\n${diffLog(tail, expected)}`);
  }
  for (let i = 0; i < expected.length; i++) {
    const got = tail[i];
    const want = expected[i];
    for (const [k, v] of Object.entries(want)) {
      if (!deepEq(got[k], v)) {
        throw new Error(`${label}[${i}]: expected ${k}=${JSON.stringify(v)}, got ${JSON.stringify(got)}\ncontext: ${JSON.stringify(log.slice(-expected.length - 2).map(compactEvent))}`);
      }
    }
  }
  return true;
}

/** Assert at least one event in the log matches (subset match). */
export function expectEvent(log, want, label = 'event') {
  const hit = log.find((e) => Object.entries(want).every(([k, v]) => deepEq(e[k], v)));
  if (!hit) {
    throw new Error(`${label}: no event matches ${JSON.stringify(want)} in ${log.length} events`);
  }
  return hit;
}

export function expect(cond, label) {
  if (!cond) throw new Error(`expect: ${label}`);
  return true;
}

function diffLog(tail, expected) {
  return `tail: ${tail.map(compactEvent).join(' | ')}\nwant: ${expected.map((e) => JSON.stringify(e)).join(' | ')}`;
}

function compactEvent(e) {
  return JSON.stringify(e).slice(0, 90);
}

export function deepEq(a, b) {
  if (a === b) return true;
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  if (ka.length !== kb.length) return false;
  return ka.every((k) => deepEq(a[k], b[k]));
}