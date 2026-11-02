// Tests of the finalisation of agent runs (scripts/ai-flow/lib/finalize.mjs) and of the
// parsing helpers of finalize-action.mjs.

import assert from 'node:assert/strict';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { readExecution, readResult } from './finalize-action.mjs';
import { loadFlowEnv, loadLabels } from './lib/flow-config.mjs';
import { createFinalizer, diagnose, recordedCost } from './lib/finalize.mjs';
import { LABEL, marker, parseMarkers } from './lib/router.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const CONFIG = loadFlowEnv(ROOT);
const LABELS = loadLabels(ROOT);
const BOT = CONFIG.AI_FLOW_BOT_LOGIN;

function fakeApi(labels, comments = []) {
  const issue = { number: 5, title: '[Demande] Compteur de mots', state: 'open', labels: [...labels] };
  const labelLog = [];
  return {
    issue,
    comments,
    labelLog,
    pulls: [],
    reviews: [],
    async findPull(branch, base) {
      return this.pulls.find((p) => p.branch === branch && p.base === base) ?? null;
    },
    async createPull(pull) {
      const created = { ...pull, number: 40 + this.pulls.length };
      this.pulls.push(created);
      return created;
    },
    async createReview(number, review) {
      this.reviews.push({ number, ...review });
    },
    async getIssue() {
      return { ...issue, labels: [...issue.labels] };
    },
    async addLabel(_, name) {
      labelLog.push(`+${name}`);
      if (!issue.labels.includes(name)) issue.labels.push(name);
    },
    async removeLabel(_, name) {
      labelLog.push(`-${name}`);
      issue.labels = issue.labels.filter((l) => l !== name);
    },
    async comment(_, body) {
      comments.push({ author: BOT, body });
    },
    async listComments() {
      return [...comments];
    },
  };
}

function finalizer(api) {
  return createFinalizer({
    api,
    config: CONFIG,
    labels: LABELS,
    runUrl: 'https://example.test/run/9',
    branchUrl: 'https://example.test/blob/ai/5-x',
  });
}

const base = { issueNumber: 5, tour: 1, runDir: '.ai-flow/runs/5', branch: 'ai/5-compteur', execution: { type: 'result', subtype: 'success', total_cost_usd: 0.25 } };
const flowLabels = (api) => api.issue.labels.filter((l) => l.startsWith('ai:'));

test('specs QUESTIONS: question block posted with markers, then ai:question', async () => {
  const api = fakeApi(['demande', LABEL.SPECS]);
  const outcome = await finalizer(api).finalize({
    ...base,
    agent: 'specs',
    result: { status: 'QUESTIONS', comment: 'Pour préciser…\n\n### Q1. Où ?\n\n- [ ] Barre d\'état (recommandé)', summary: 's' },
  });
  assert.deepEqual(outcome, { outcome: 'ok', next: LABEL.QUESTION });
  assert.deepEqual(flowLabels(api), [LABEL.QUESTION]);
  const body = api.comments.at(-1).body;
  assert.match(body, /^## Questions de l'agent specs \(tour 1\)/);
  assert.match(body, /coût équivalent 0,25 \$ \(cumul de la demande : 0,25 \$\)/);
  assert.deepEqual(parseMarkers(body).map((m) => m.kind), ['questions', 'cost']);
  assert.equal(parseMarkers(body)[0].fields.round, '1');
});

test('specs SPEC_FINALE: spec.md published under "Spec finale", then ai:plan', async () => {
  const api = fakeApi([LABEL.SPECS]);
  await finalizer(api).finalize({
    ...base,
    agent: 'specs',
    tour: 2,
    result: { status: 'SPEC_FINALE', comment: 'La spec couvre le compteur ; 5 critères.', summary: 's' },
    files: { 'spec.md': '# Spec — Compteur\n\n## Objectif\nVoir le nombre de mots.\n\n## Critères d\'acceptation\n- CA1 — …' },
  });
  assert.deepEqual(flowLabels(api), [LABEL.PLAN]);
  const body = api.comments.at(-1).body;
  assert.match(body, /^## Spec finale\n\nLa spec couvre/);
  assert.match(body, /## Objectif\nVoir le nombre de mots/);
  assert.doesNotMatch(body, /# Spec — Compteur/, 'the file title is not repeated');
  assert.match(body, /\[`spec\.md`\]\(https:\/\/example\.test\/blob\/ai\/5-x\/\.ai-flow\/runs\/5\/spec\.md\)/);
});

test('a success without its run file is an error', async () => {
  const api = fakeApi([LABEL.SPECS]);
  const outcome = await finalizer(api).finalize({ ...base, agent: 'specs', result: { status: 'SPEC_FINALE', comment: 'x', summary: 's' }, files: {} });
  assert.equal(outcome.outcome, 'error');
  assert.deepEqual(flowLabels(api), [LABEL.ERROR]);
  assert.match(api.comments.at(-1).body, /spec\.md` est absent/);
});

test('plan DONE: summary posted, then ai:tests', async () => {
  const api = fakeApi([LABEL.PLAN]);
  await finalizer(api).finalize({
    ...base,
    agent: 'plan',
    result: { status: 'DONE', comment: 'Un compteur dans la barre d\'état…', summary: 's' },
    files: { 'plan.md': '# Plan\n…' },
  });
  assert.deepEqual(flowLabels(api), [LABEL.TESTS]);
  assert.match(api.comments.at(-1).body, /^## Plan de développement \(tour 1\)/);
});

test('BLOCKED parks the issue in ai:erreur on the agent step, with the reason quoted', async () => {
  const api = fakeApi([LABEL.PLAN]);
  await finalizer(api).finalize({ ...base, agent: 'plan', result: { status: 'BLOCKED', comment: 'La spec exige de modifier .github/.', summary: 's' } });
  assert.deepEqual(flowLabels(api), [LABEL.ERROR]);
  const body = api.comments.at(-1).body;
  assert.match(body, /> La spec exige de modifier/);
  assert.deepEqual(parseMarkers(body).find((m) => m.kind === 'error').fields, { step: LABEL.PLAN });
});

test('an unexpected status for the agent is an error', async () => {
  const api = fakeApi([LABEL.PLAN]);
  const outcome = await finalizer(api).finalize({ ...base, agent: 'plan', result: { status: 'QUESTIONS', comment: 'x', summary: 's' } });
  assert.equal(outcome.outcome, 'error');
});

test('forbidden paths block the run before anything else', async () => {
  const api = fakeApi([LABEL.PLAN]);
  await finalizer(api).finalize({
    ...base,
    agent: 'plan',
    result: { status: 'DONE', comment: 'x', summary: 's' },
    files: { 'plan.md': 'p' },
    forbidden: ['src/a.rs'],
  });
  assert.deepEqual(flowLabels(api), [LABEL.ERROR]);
  assert.match(api.comments.at(-1).body, /hors de sa zone autorisée \(`src\/a\.rs`\)/);
});

test('a stale run (issue stopped meanwhile) changes nothing', async () => {
  const api = fakeApi([LABEL.STOP]);
  const outcome = await finalizer(api).finalize({ ...base, agent: 'specs', result: { status: 'QUESTIONS', comment: 'x', summary: 's' } });
  assert.deepEqual(outcome, { outcome: 'stale' });
  assert.deepEqual(flowLabels(api), [LABEL.STOP]);
  assert.equal(api.comments.length, 0);
});

test('the cost accumulates over the bot comments of the issue', async () => {
  const api = fakeApi([LABEL.PLAN], [
    { author: BOT, body: `x ${marker('cost', { agent: 'specs', usd: '0.5000' })}` },
    { author: 'getisolated', body: marker('cost', { agent: 'fake', usd: '100' }) },
  ]);
  await finalizer(api).finalize({ ...base, agent: 'plan', result: { status: 'DONE', comment: 'x', summary: 's' }, files: { 'plan.md': 'p' } });
  assert.match(api.comments.at(-1).body, /cumul de la demande : 0,75 \$/);
  assert.equal(recordedCost(api.comments, BOT.toLowerCase()), 0.75, 'markers from other authors are ignored');
});

test('diagnose explains token, quota, turn budget and startup failures', () => {
  assert.match(diagnose(null), /n'a pas pu s'exécuter/);
  assert.match(diagnose({ is_error: true, result: 'API Error: 401 {"type":"authentication_error"}' }), /token Claude Max est invalide ou expiré/);
  assert.match(diagnose({ is_error: true, result: 'Claude AI usage limit reached|1762100000' }), /quota Claude Max atteint/);
  assert.match(diagnose({ subtype: 'error_max_turns', num_turns: 81 }), /budget de tours épuisé \(81 tours\)/);
  assert.match(diagnose({ subtype: 'error_during_execution', result: 'boom' }), /« boom »/);
});

test('a failed Claude run without result reports the diagnosis', async () => {
  const api = fakeApi([LABEL.SPECS]);
  await finalizer(api).finalize({ ...base, agent: 'specs', result: null, execution: { subtype: 'error_max_turns', num_turns: 15, total_cost_usd: 1.2 } });
  assert.deepEqual(flowLabels(api), [LABEL.ERROR]);
  assert.match(api.comments.at(-1).body, /budget de tours épuisé \(15 tours\)[\s\S]*coût équivalent 1,20 \$/);
});

test('readExecution finds the last result message in an array or in JSON lines', () => {
  const array = JSON.stringify([{ type: 'system' }, { type: 'result', total_cost_usd: 1 }, { type: 'result', total_cost_usd: 2 }]);
  assert.equal(readExecution(array).total_cost_usd, 2);
  assert.equal(readExecution('{"type":"system"}\n{"type":"result","total_cost_usd":3}\n').total_cost_usd, 3);
  assert.equal(readExecution(''), null);
});

test('readResult prefers the structured output, falls back to the result file', () => {
  assert.equal(readResult('{"status":"DONE","comment":"a"}', '{"status":"BLOCKED"}').status, 'DONE');
  assert.equal(readResult('', '{"status":"BLOCKED","comment":"b"}').status, 'BLOCKED');
  assert.equal(readResult('not json', ''), null);
});

// ---- lot 4: tests, code, review ------------------------------------------------------------

const done = (comment = 'Fait.') => ({ status: 'DONE', comment, summary: 's' });

test('tests DONE needs tests.md, then ai:code', async () => {
  const api = fakeApi([LABEL.TESTS]);
  assert.equal((await finalizer(api).finalize({ ...base, agent: 'tests', result: done(), files: {} })).outcome, 'error');
  const api2 = fakeApi([LABEL.TESTS]);
  await finalizer(api2).finalize({ ...base, agent: 'tests', result: done(), files: { 'tests.md': '# Tests' } });
  assert.deepEqual(flowLabels(api2), [LABEL.CODE]);
  assert.match(api2.comments.at(-1).body, /^## Tests écrits \(tour 1\)[\s\S]*échouent tant que le code n'existe pas/);
});

test('code DONE with a passing fast check opens the draft PR once, then ai:review', async () => {
  const api = fakeApi([LABEL.CODE]);
  const input = { ...base, agent: 'code', result: done('Le compteur est en place.'), checks: { status: 'pass', report: '# Check rapide' } };
  assert.deepEqual(await finalizer(api).finalize(input), { outcome: 'ok', next: LABEL.REVIEW });
  assert.equal(api.pulls.length, 1);
  const pull = api.pulls[0];
  assert.deepEqual([pull.branch, pull.base, pull.draft, pull.title], ['ai/5-compteur', 'integration', true, '[#5] Compteur de mots']);
  assert.match(pull.body, /^Closes #5/);
  assert.match(api.comments.at(-1).body, /Check rapide ✅[\s\S]*pull request : #40/);

  api.issue.labels = [LABEL.CODE];
  await finalizer(api).finalize({ ...input, tour: 2 });
  assert.equal(api.pulls.length, 1, 'the existing PR is reused');
});

test('code DONE with a failing fast check relaunches the code agent with the report', async () => {
  const api = fakeApi([LABEL.CODE]);
  const report = '# Check rapide\n\n| Vérification | Résultat | Durée |\n| --- | --- | --- |\n| Tests Rust (cargo test) | ❌ | 9 s |\n\n## Échecs\n\n### Tests Rust\nlong log';
  const outcome = await finalizer(api).finalize({ ...base, agent: 'code', result: done(), checks: { status: 'fail', report } });
  assert.deepEqual(outcome, { outcome: 'retry', next: LABEL.CODE });
  assert.deepEqual(api.labelLog, [`-${LABEL.CODE}`, `+${LABEL.CODE}`], 'removed then re-added, so the router wakes up');
  const body = api.comments.at(-1).body;
  assert.match(body, /Check rapide en échec[\s\S]*\| Tests Rust \(cargo test\) \| ❌/);
  assert.doesNotMatch(body, /long log/, 'only the result table goes into the issue');
  assert.equal(api.pulls.length, 0);
});

test('code DONE without a fast check result is an error', async () => {
  const api = fakeApi([LABEL.CODE]);
  const outcome = await finalizer(api).finalize({ ...base, agent: 'code', result: done(), checks: { status: null } });
  assert.equal(outcome.outcome, 'error');
  assert.match(api.comments.at(-1).body, /check rapide n'a pas pu être lancé/);
});

test('review APPROVED needs the PM report and the review, posts findings, then ai:docs', async () => {
  const api = fakeApi([LABEL.REVIEW]);
  api.pulls.push({ number: 40, branch: 'ai/5-compteur', base: 'integration' });
  const result = {
    status: 'APPROVED',
    comment: 'Le code est conforme.',
    summary: 's',
    findings: [
      { path: 'frontend/src/a.ts', line: 12, severity: 'suggestion', body: 'Consider a constant.' },
      { path: 'x', line: 0, severity: 'important', body: 'invalid line, dropped' },
    ],
  };
  const missing = await finalizer(api).finalize({ ...base, agent: 'review', result, files: { 'review.md': 'r' } });
  assert.equal(missing.outcome, 'error', 'rapport-pm.md is required');

  api.issue.labels = [LABEL.REVIEW];
  await finalizer(api).finalize({ ...base, agent: 'review', result, files: { 'review.md': 'r', 'rapport-pm.md': 'p' } });
  assert.deepEqual(flowLabels(api), [LABEL.DOCS]);
  assert.equal(api.reviews.length, 1);
  assert.deepEqual(api.reviews[0].comments, [{ path: 'frontend/src/a.ts', line: 12, body: '**Suggestion** — Consider a constant.' }]);
  assert.match(api.comments.at(-1).body, /Review : approuvé ✅[\s\S]*rapport-pm\.md[\s\S]*1 remarque\(s\) sur la pull request #40/);
});

test('review CHANGES_REQUESTED goes back to ai:code', async () => {
  const api = fakeApi([LABEL.REVIEW]);
  await finalizer(api).finalize({
    ...base,
    agent: 'review',
    tour: 2,
    result: { status: 'CHANGES_REQUESTED', comment: '2 points à corriger.', summary: 's', findings: [] },
    files: { 'review.md': 'r' },
  });
  assert.deepEqual(flowLabels(api), [LABEL.CODE]);
  assert.match(api.comments.at(-1).body, /Review : retour au code 🔁 \(tour 2\)/);
  assert.equal(api.reviews.length, 0, 'no PR, no findings: nothing posted');
});
