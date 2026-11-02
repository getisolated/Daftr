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
  const issue = { number: 5, state: 'open', labels: [...labels] };
  return {
    issue,
    comments,
    async getIssue() {
      return { ...issue, labels: [...issue.labels] };
    },
    async addLabel(_, name) {
      if (!issue.labels.includes(name)) issue.labels.push(name);
    },
    async removeLabel(_, name) {
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

const base = { issueNumber: 5, tour: 1, runDir: '.ai-flow/runs/5', execution: { type: 'result', subtype: 'success', total_cost_usd: 0.25 } };
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
