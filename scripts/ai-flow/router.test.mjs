// Tests of the AI flow router against an in-memory GitHub. Every label the bot sets emits an
// event that is fed back to the router when it passes the same filter as the `if:` of
// .github/workflows/ai-router.yml: whole scenarios run as they would on GitHub.
//
// Run: node --test scripts/ai-flow/

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { loadFlowEnv, loadLabels, parseFlowEnv, parseLabels } from './lib/flow-config.mjs';
import {
  BOT_TRIGGER_LABELS,
  LABEL,
  countLabeledSince,
  createRouter,
  marker,
  parseMarkers,
  slugify,
} from './lib/router.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const CONFIG = loadFlowEnv(ROOT);
const LABELS = loadLabels(ROOT);
const BOT = CONFIG.AI_FLOW_BOT_LOGIN;
const PM = 'getisolated';
const STRANGER = 'someone-else';

// ---- in-memory GitHub ---------------------------------------------------------------------

function createSim({ start = Date.parse('2026-11-02T10:00:00Z'), available = () => true } = {}) {
  let clock = start;
  const tickClock = (ms = 1000) => {
    clock += ms;
    return new Date(clock).toISOString();
  };
  const issues = new Map();
  const branches = new Set(['main', 'integration']);
  const runs = [];
  const pulls = [];
  const releases = [];
  const cancelled = [];
  const queue = [];
  const dispatches = [];

  const issue = (n) => {
    const found = issues.get(n);
    if (!found) throw Object.assign(new Error(`issue ${n} not found`), { status: 404 });
    return found;
  };

  function applyLabel(n, name, actor, action) {
    const target = issue(n);
    const has = target.labels.includes(name);
    if (action === 'labeled' && has) return;
    if (action === 'unlabeled' && !has) throw Object.assign(new Error('label not on issue'), { status: 404 });
    target.labels = action === 'labeled' ? [...target.labels, name] : target.labels.filter((l) => l !== name);
    const at = tickClock();
    target.updatedAt = at;
    target.events.push({ action, label: name, actor, createdAt: at });
    queue.push({ kind: 'issues', action, issueNumber: n, label: name, sender: actor });
  }

  const api = {
    async getIssue(n) {
      const i = issue(n);
      return { ...i, labels: [...i.labels] };
    },
    async listOpenIssues() {
      return [...issues.values()].filter((i) => i.state === 'open').map((i) => ({ ...i, labels: [...i.labels] }));
    },
    async addLabel(n, name) {
      applyLabel(n, name, BOT, 'labeled');
    },
    async removeLabel(n, name) {
      if (issue(n).labels.includes(name)) applyLabel(n, name, BOT, 'unlabeled');
    },
    async comment(n, body) {
      const i = issue(n);
      const at = tickClock();
      i.comments.push({ id: i.comments.length + 1, author: BOT, body, createdAt: at, url: `c${i.comments.length + 1}` });
      i.updatedAt = at;
    },
    async listComments(n) {
      return [...issue(n).comments];
    },
    async listLabelEvents(n) {
      return [...issue(n).events];
    },
    async findBranch(prefix) {
      return [...branches].find((b) => b.startsWith(prefix)) ?? null;
    },
    async createBranch(name, from) {
      assert.ok(branches.has(from), `base branch ${from} exists`);
      branches.add(name);
    },
    async listActiveRuns() {
      return runs.filter((r) => r.active);
    },
    async cancelRun(id) {
      cancelled.push(id);
      const run = runs.find((r) => r.id === id);
      if (run) run.active = false;
    },
    async listOpenPullsByHeadPrefix(prefix) {
      return pulls.filter((p) => p.open && p.head.startsWith(prefix));
    },
    async closePull(number) {
      pulls.find((p) => p.number === number).open = false;
    },
    async listReleases() {
      return [...releases];
    },
    async deleteRelease(id) {
      releases.splice(releases.findIndex((r) => r.id === id), 1);
    },
  };

  const router = createRouter({
    api,
    config: CONFIG,
    labels: LABELS,
    pmLogins: `${PM}, other-pm`,
    runUrl: 'https://example.test/run/1',
    runId: 1,
    isAgentAvailable: available,
    now: () => new Date(clock),
  });

  /** Same filter as the `if:` of the route job in ai-router.yml. */
  function passesWorkflowFilter(event) {
    if (event.kind !== 'issues') return true;
    const isBot = event.sender === BOT;
    switch (event.action) {
      case 'closed':
        return true;
      case 'opened':
        return issue(event.issueNumber).labels.includes(LABEL.DEMANDE);
      case 'labeled':
        return event.label.startsWith('ai:') && (!isBot || BOT_TRIGGER_LABELS.includes(event.label));
      case 'unlabeled':
        return event.label === LABEL.ERROR && !isBot;
      default:
        return false;
    }
  }

  /** Delivers queued events to the router until GitHub is quiet. */
  async function drain() {
    let guard = 0;
    while (queue.length) {
      guard += 1;
      assert.ok(guard < 200, 'event storm: the router keeps waking itself');
      const event = queue.shift();
      if (!passesWorkflowFilter(event)) continue;
      const result = await router.handle(event);
      if (result.dispatch) dispatches.push(result.dispatch);
    }
  }

  return {
    issues,
    branches,
    runs,
    pulls,
    releases,
    cancelled,
    dispatches,
    router,
    advance: (hours) => tickClock(hours * 3600 * 1000),
    async open(number, title, author = PM) {
      const at = tickClock();
      issues.set(number, { number, title, state: 'open', labels: [LABEL.DEMANDE], author, createdAt: at, updatedAt: at, comments: [], events: [] });
      queue.push({ kind: 'issues', action: 'opened', issueNumber: number, label: null, sender: author });
      await drain();
    },
    async label(number, name, actor = PM) {
      applyLabel(number, name, actor, 'labeled');
      await drain();
    },
    async unlabel(number, name, actor = PM) {
      applyLabel(number, name, actor, 'unlabeled');
      await drain();
    },
    /** What an agent workflow does when it finishes: set the next state as the bot. */
    async botSets(number, name) {
      await api.addLabel(number, name);
      for (const l of issue(number).labels) {
        if (l !== name && l.startsWith('ai:')) await api.removeLabel(number, l);
      }
      await drain();
    },
    async userComment(number, body, author = PM) {
      const i = issue(number);
      i.comments.push({ id: i.comments.length + 1, author, body, createdAt: tickClock(), url: `c${i.comments.length + 1}` });
    },
    async close(number) {
      issue(number).state = 'closed';
      queue.push({ kind: 'issues', action: 'closed', issueNumber: number, label: null, sender: PM });
      await drain();
    },
    async tick() {
      const result = await router.handle({ kind: 'tick' });
      if (result.dispatch) dispatches.push(result.dispatch);
      await drain();
    },
    flowLabels: (n) => issue(n).labels.filter((l) => l.startsWith('ai:')),
    lastComment: (n) => issue(n).comments.at(-1)?.body ?? '',
    lastDispatch: () => dispatches.at(-1),
  };
}

// ---- configuration --------------------------------------------------------------------------

test('flow.env and labels.txt parse like the bash scripts', () => {
  assert.deepEqual(parseFlowEnv('# c\nA=1\nB="x y"\r\n\nC=\'[z]\'\n'), { A: '1', B: 'x y', C: '[z]' });
  assert.deepEqual(parseLabels('# c\n ai:x | 0969da | state | Une | deux\n'), [
    { name: 'ai:x', color: '0969da', role: 'state', description: 'Une | deux' },
  ]);
  assert.equal(BOT, 'daftr-ai-flow[bot]');
});

test('every label the router uses exists in labels.txt', () => {
  const names = new Set(LABELS.map((l) => l.name));
  for (const name of Object.values(LABEL)) assert.ok(names.has(name), `${name} in labels.txt`);
  for (const label of LABELS) assert.ok(label.description.length <= 100, `${label.name}: description <= 100 chars`);
});

test('ai-router.yml filters events with the same bot login and trigger labels as the router', () => {
  const workflow = readFileSync(join(ROOT, '.github', 'workflows', 'ai-router.yml'), 'utf8');
  assert.ok(workflow.includes(`'${BOT}'`), 'bot login of flow.env appears in the workflow filter');
  const triggers = JSON.parse(workflow.match(/fromJSON\('(\[[^\]]*\])'\)/)[1]);
  assert.deepEqual([...triggers].sort(), [...BOT_TRIGGER_LABELS].sort());
});

test('slugify makes short ASCII branch slugs', () => {
  assert.equal(slugify('[Demande] Compteur de mots & temps de lecture'), 'compteur-de-mots-temps-de-lecture');
  assert.equal(slugify('Élément très « spécial » !!!'), 'element-tres-special');
  assert.equal(slugify('???'), 'demande');
  assert.ok(slugify('a'.repeat(80)).length <= 40);
});

test('markers round-trip', () => {
  const body = `texte\n\n${marker('error', { step: LABEL.CODE })}`;
  assert.deepEqual(parseMarkers(body), [{ kind: 'error', fields: { step: 'ai:code' } }]);
});

test('countLabeledSince resets at the anchor', () => {
  const ev = (label) => ({ action: 'labeled', label });
  const events = [ev('ai:code'), ev('ai:code'), ev('ai:plan'), ev('ai:code'), { action: 'unlabeled', label: 'ai:plan' }, ev('ai:code')];
  assert.equal(countLabeledSince(events, 'ai:code', 'ai:plan'), 2);
});

// ---- lot 2 exit test ------------------------------------------------------------------------

test('lot 2: second issue queues, ai:stop on the first starts the second', async () => {
  const sim = createSim();
  await sim.open(1, '[Demande] Compteur de mots');
  await sim.open(2, '[Demande] Raccourci clavier');
  assert.match(sim.lastComment(1), /ai:go/);
  assert.deepEqual(sim.flowLabels(1), [], 'nothing starts without ai:go');

  await sim.label(1, LABEL.GO);
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
  assert.ok(sim.branches.has('ai/1-compteur-de-mots'));
  assert.deepEqual(sim.lastDispatch(), { agent: 'specs', issue_number: 1, branch: 'ai/1-compteur-de-mots', run_dir: '.ai-flow/runs/1', tour: 1 });

  await sim.label(2, LABEL.GO);
  assert.deepEqual(sim.flowLabels(2), [LABEL.QUEUED]);
  assert.match(sim.lastComment(2), /file d'attente, position 1/);
  assert.equal(sim.dispatches.length, 1, 'no agent for the queued issue');

  sim.runs.push({ id: 41, title: 'ai-flow #1 · labeled ai:specs', active: true }, { id: 42, title: 'ai-flow #12 · labeled ai:plan', active: true });
  await sim.label(1, LABEL.STOP);
  assert.deepEqual(sim.flowLabels(1), [LABEL.STOP]);
  assert.deepEqual(sim.cancelled, [41], 'only the runs of #1 are cancelled');
  assert.deepEqual(sim.flowLabels(2), [LABEL.SPECS]);
  assert.match(sim.lastComment(2), /C'est votre tour/);
  assert.equal(sim.lastDispatch().issue_number, 2);
});

test('queue order follows the time of entry in the queue', async () => {
  const sim = createSim();
  for (const n of [1, 2, 3]) await sim.open(n, `Demande ${n}`);
  await sim.label(1, LABEL.GO);
  await sim.label(3, LABEL.GO);
  await sim.label(2, LABEL.GO);
  await sim.label(1, LABEL.STOP);
  assert.deepEqual(sim.flowLabels(3), [LABEL.SPECS], 'issue 3 entered the queue first');
  assert.deepEqual(sim.flowLabels(2), [LABEL.QUEUED]);
});

// ---- access control -------------------------------------------------------------------------

test('a PM decision from a non-PM login is removed with an explanation', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.VALIDATED, STRANGER);
  assert.deepEqual(sim.flowLabels(1), []);
  assert.match(sim.lastComment(1), new RegExp(`@${STRANGER}, seul le PM`));
});

test('a machine label set by a human is removed and does not run an agent', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.PLAN, PM);
  assert.deepEqual(sim.flowLabels(1), []);
  assert.equal(sim.dispatches.length, 0);
  assert.match(sim.lastComment(1), /réservé au flux/);
});

test('PM logins are case-insensitive and a second PM is accepted', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande', STRANGER);
  await sim.label(1, LABEL.GO, 'Other-PM');
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
});

test('ai:go on an issue already in the flow is removed', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await sim.label(1, LABEL.GO);
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
  assert.match(sim.lastComment(1), /déjà engagé/);
});

test('a decision that is not expected in the current state is removed', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await sim.label(1, LABEL.VALIDATED);
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
  assert.match(sim.lastComment(1), /n'est pas attendu/);
});

// ---- specs loop -----------------------------------------------------------------------------

test('answers relaunch specs with the next round, escalation past the limit', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  for (let round = 2; round <= 3; round += 1) {
    await sim.botSets(1, LABEL.QUESTION);
    await sim.label(1, LABEL.ANSWERED);
    assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
    assert.equal(sim.lastDispatch().tour, round);
  }
  await sim.botSets(1, LABEL.QUESTION);
  await sim.label(1, LABEL.ANSWERED);
  assert.deepEqual(sim.flowLabels(1), [LABEL.ESCALATED]);

  await sim.label(1, LABEL.FORCE);
  assert.deepEqual(sim.flowLabels(1), [LABEL.PLAN]);
  assert.equal(sim.lastDispatch().agent, 'plan');
});

// ---- full chain, review loop, PM returns ----------------------------------------------------

async function toReview(sim, n) {
  await sim.botSets(n, LABEL.PLAN);
  await sim.botSets(n, LABEL.TESTS);
  await sim.botSets(n, LABEL.CODE);
  await sim.botSets(n, LABEL.REVIEW);
}

test('agents chain on the bot labels with the right tours', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await toReview(sim, 1);
  assert.deepEqual(sim.dispatches.map((d) => d.agent), ['specs', 'plan', 'tests', 'code', 'review']);
  await sim.botSets(1, LABEL.CODE);
  await sim.botSets(1, LABEL.REVIEW);
  assert.deepEqual(sim.dispatches.slice(-2).map((d) => [d.agent, d.tour]), [['code', 2], ['review', 2]]);
});

test('review sends code back at most 3 times, then escalates; forcing goes to docs', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await toReview(sim, 1);
  for (let i = 0; i < 3; i += 1) {
    await sim.botSets(1, LABEL.CODE);
    assert.deepEqual(sim.flowLabels(1), [LABEL.CODE]);
    await sim.botSets(1, LABEL.REVIEW);
  }
  await sim.botSets(1, LABEL.CODE);
  assert.deepEqual(sim.flowLabels(1), [LABEL.ESCALATED]);
  assert.match(sim.lastComment(1), /code a été repris 3 fois/);

  await sim.label(1, LABEL.FORCE);
  assert.deepEqual(sim.flowLabels(1), [LABEL.DOCS]);
  assert.equal(sim.lastDispatch().agent, 'docs');
});

test('ai:problème needs a PM comment, goes back to plan, escalates after 3 returns', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await toReview(sim, 1);

  for (let returns = 0; returns < 3; returns += 1) {
    await sim.botSets(1, LABEL.TO_VALIDATE);
    await sim.label(1, LABEL.PROBLEM);
    assert.deepEqual(sim.flowLabels(1), [LABEL.TO_VALIDATE], 'rejected without comment');
    assert.match(sim.lastComment(1), /Décrivez d'abord le problème/);

    await sim.userComment(1, 'Le compteur ne se met pas à jour quand je tape.');
    await sim.label(1, LABEL.PROBLEM);
    assert.deepEqual(sim.flowLabels(1), [LABEL.PLAN]);
    assert.deepEqual([sim.lastDispatch().agent, sim.lastDispatch().tour], ['plan', returns + 2]);
    assert.equal(parseMarkers(sim.lastComment(1))[0].kind, 'pm-return');
    await sim.botSets(1, LABEL.TESTS);
    await sim.botSets(1, LABEL.CODE);
    assert.equal(sim.lastDispatch().tour, 1, 'review loop counter restarts after a new plan');
    await sim.botSets(1, LABEL.REVIEW);
  }

  await sim.botSets(1, LABEL.TO_VALIDATE);
  await sim.userComment(1, 'Toujours pas bon.');
  await sim.label(1, LABEL.PROBLEM);
  assert.deepEqual(sim.flowLabels(1), [LABEL.ESCALATED]);

  await sim.label(1, LABEL.FORCE);
  assert.deepEqual(sim.flowLabels(1), [LABEL.VALIDATED]);
  assert.equal(sim.lastDispatch().agent, 'deliver');
});

test('ai:problème from an escalation goes back to plan', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await toReview(sim, 1);
  for (let i = 0; i < 3; i += 1) {
    await sim.botSets(1, LABEL.CODE);
    await sim.botSets(1, LABEL.REVIEW);
  }
  await sim.botSets(1, LABEL.CODE);
  assert.deepEqual(sim.flowLabels(1), [LABEL.ESCALATED]);
  await sim.userComment(1, 'Simplifiez : pas besoin du temps de lecture.');
  await sim.label(1, LABEL.PROBLEM);
  assert.deepEqual(sim.flowLabels(1), [LABEL.PLAN]);
});

// ---- delivery -------------------------------------------------------------------------------

test('ai:validé dispatches delivery; ai:livré cleans up and starts the next issue', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande 1');
  await sim.open(2, 'Demande 2');
  await sim.label(1, LABEL.GO);
  await sim.label(2, LABEL.GO);
  await toReview(sim, 1);
  await sim.botSets(1, LABEL.DOCS);
  await sim.botSets(1, LABEL.DEPLOY);
  await sim.botSets(1, LABEL.TO_VALIDATE);
  sim.releases.push({ id: 7, tag: 'test-issue-1-tour-1' }, { id: 8, tag: 'test-issue-10-tour-1' }, { id: 9, tag: 'v1.5.0' });

  await sim.label(1, LABEL.VALIDATED);
  assert.deepEqual(sim.flowLabels(1), [LABEL.VALIDATED]);
  assert.equal(sim.lastDispatch().agent, 'deliver');
  assert.deepEqual(sim.flowLabels(2), [LABEL.QUEUED], 'still locked while delivering');

  await sim.botSets(1, LABEL.DELIVERED);
  assert.deepEqual(sim.releases.map((r) => r.tag), ['test-issue-10-tour-1', 'v1.5.0']);
  assert.deepEqual(sim.flowLabels(2), [LABEL.SPECS]);
});

// ---- errors, closing, housekeeping ----------------------------------------------------------

test('removing ai:erreur relaunches the failed step', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await sim.botSets(1, LABEL.PLAN);
  await sim.botSets(1, LABEL.ERROR);
  await sim.issues.get(1).comments.push({ id: 99, author: BOT, body: `Erreur\n${marker('error', { step: LABEL.PLAN })}`, createdAt: sim.advance(0), url: 'c99' });

  await sim.unlabel(1, LABEL.ERROR, STRANGER);
  assert.deepEqual(sim.flowLabels(1), [LABEL.ERROR], 'a non-PM cannot relaunch');

  await sim.unlabel(1, LABEL.ERROR);
  assert.deepEqual(sim.flowLabels(1), [LABEL.PLAN]);
  assert.deepEqual([sim.lastDispatch().agent, sim.lastDispatch().tour], ['plan', 2]);
});

test('closing an active issue stops its flow and frees the lock', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande 1');
  await sim.open(2, 'Demande 2');
  await sim.label(1, LABEL.GO);
  await sim.label(2, LABEL.GO);
  sim.pulls.push({ number: 30, head: 'ai/1-demande-1', open: true });
  await sim.close(1);
  assert.deepEqual(sim.flowLabels(1), [LABEL.STOP]);
  assert.equal(sim.pulls[0].open, false);
  assert.deepEqual(sim.flowLabels(2), [LABEL.SPECS]);
});

test('closing a queued issue takes it out of the queue', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande 1');
  await sim.open(2, 'Demande 2');
  await sim.label(1, LABEL.GO);
  await sim.label(2, LABEL.GO);
  await sim.close(2);
  assert.deepEqual(sim.flowLabels(2), []);
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
});

test('ai:go after ai:stop restarts the flow on the same branch', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await sim.label(1, LABEL.STOP);
  await sim.label(1, LABEL.GO);
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
  assert.deepEqual([...sim.branches].filter((b) => b.startsWith('ai/1-')), ['ai/1-demande']);
});

test('tick parks an interrupted agent in ai:erreur, then relaunching works', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  sim.runs.push({ id: 5, title: 'ai-flow #1 · labeled ai:specs', active: true });
  sim.advance(1);
  await sim.tick();
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS], 'a running agent is left alone');

  sim.runs[0].active = false;
  await sim.tick();
  assert.deepEqual(sim.flowLabels(1), [LABEL.ERROR]);
  await sim.unlabel(1, LABEL.ERROR);
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
});

test('tick reminds the PM after 48 h of silence, not before', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await sim.botSets(1, LABEL.QUESTION);
  const before = sim.issues.get(1).comments.length;
  sim.advance(47);
  await sim.tick();
  assert.equal(sim.issues.get(1).comments.length, before);
  sim.advance(2);
  await sim.tick();
  assert.match(sim.lastComment(1), /@getisolated @other-pm rappel/);
  await sim.tick();
  assert.equal(sim.issues.get(1).comments.length, before + 1, 'one reminder per 48 h');
});

test('tick starts a queued issue when the lock is free', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande 1');
  await sim.open(2, 'Demande 2');
  await sim.label(1, LABEL.GO);
  await sim.label(2, LABEL.GO);
  sim.issues.get(1).state = 'closed'; // closed while the router was down: no event
  await sim.tick();
  assert.deepEqual(sim.flowLabels(2), [LABEL.SPECS]);
});

test('a router failure parks the issue in ai:erreur on the current step', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await sim.router.reportFailure(1, new Error('boom'));
  assert.deepEqual(sim.flowLabels(1), [LABEL.ERROR]);
  assert.deepEqual(parseMarkers(sim.lastComment(1)), [{ kind: 'error', fields: { step: LABEL.SPECS } }]);
});

test('an agent whose workflow is not installed yet is not dispatched, nor declared interrupted', async () => {
  const sim = createSim({ available: (agent) => agent !== 'specs' });
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
  assert.equal(sim.dispatches.length, 0);
  assert.match(sim.lastComment(1), /agent \*\*specs\*\* n'est pas encore installé/);
  sim.advance(3);
  await sim.tick();
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS], 'the watchdog leaves it alone');
  await sim.label(1, LABEL.STOP);
  assert.deepEqual(sim.flowLabels(1), [LABEL.STOP]);
});

test('a flow restarted after ai:stop starts its counters at zero', async () => {
  const sim = createSim();
  await sim.open(1, 'Demande');
  await sim.label(1, LABEL.GO);
  await sim.botSets(1, LABEL.QUESTION);
  await sim.label(1, LABEL.ANSWERED);
  assert.equal(sim.lastDispatch().tour, 2);
  await sim.label(1, LABEL.STOP, STRANGER); // refused: does not reset anything
  assert.deepEqual(sim.flowLabels(1), [LABEL.SPECS]);
  await sim.label(1, LABEL.STOP);
  await sim.label(1, LABEL.GO);
  assert.deepEqual([sim.lastDispatch().agent, sim.lastDispatch().tour], ['specs', 1]);
});
