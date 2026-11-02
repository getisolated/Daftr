// Wiring tests: router-action.mjs and github-api.mjs against a fake Octokit client, as
// actions/github-script provides it. The state machine itself is tested in router.test.mjs.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { createGithubApi } from './lib/github-api.mjs';
import { agentWorkflowFile, eventFromContext, run } from './router-action.mjs';

const BOT = 'daftr-ai-flow[bot]';

/** Fake Octokit: `routes` maps "issues.get" to a handler; every call is recorded. */
function fakeOctokit(routes) {
  const calls = [];
  const rest = new Proxy(
    {},
    {
      get: (_, namespace) =>
        new Proxy(
          {},
          {
            get: (__, method) => {
              const key = `${String(namespace)}.${String(method)}`;
              const fn = async (params) => {
                calls.push({ key, params });
                const handler = routes[key];
                if (!handler) throw new Error(`unexpected call ${key}`);
                return { data: await handler(params) };
              };
              fn.key = key;
              return fn;
            },
          },
        ),
    },
  );
  return {
    calls,
    rest,
    paginate: async (fn, params) => (await fn(params)).data,
  };
}

function notFound() {
  return Object.assign(new Error('Not Found'), { status: 404 });
}

test('eventFromContext normalises issue events and treats the rest as a tick', () => {
  const context = {
    eventName: 'issues',
    payload: { action: 'labeled', issue: { number: 3 }, label: { name: 'ai:go' }, sender: { login: 'getisolated' } },
  };
  assert.deepEqual(eventFromContext(context), { kind: 'issues', action: 'labeled', issueNumber: 3, label: 'ai:go', sender: 'getisolated' });
  assert.deepEqual(eventFromContext({ eventName: 'schedule', payload: {} }), { kind: 'tick' });
});

test('github-api maps Octokit data and tolerates the expected errors', async () => {
  const github = fakeOctokit({
    'issues.get': () => ({ number: 4, title: 'T', state: 'open', labels: [{ name: 'ai:go' }, 'demande'], user: { login: 'pm' }, created_at: 'c', updated_at: 'u' }),
    'issues.listForRepo': () => [{ number: 4, labels: [], state: 'open' }, { number: 5, labels: [], state: 'open', pull_request: {} }],
    'issues.removeLabel': () => {
      throw notFound();
    },
    'git.listMatchingRefs': () => [{ ref: 'refs/heads/ai/4-titre' }],
    'pulls.list': () => [
      { number: 8, head: { ref: 'ai/4-titre', repo: { full_name: 'o/r' } } },
      { number: 9, head: { ref: 'ai/4-titre', repo: { full_name: 'fork/r' } } },
      { number: 10, head: { ref: 'feature/x', repo: { full_name: 'o/r' } } },
    ],
    'repos.deleteRelease': () => ({}),
    'git.deleteRef': () => {
      throw Object.assign(new Error('Unprocessable'), { status: 422 });
    },
    'actions.listWorkflowRuns': ({ status }) => (status === 'in_progress' ? [{ id: 1, display_title: 'ai-flow #4 · labeled ai:plan' }] : []),
  });
  const api = createGithubApi({ github, owner: 'o', repo: 'r' });

  assert.deepEqual((await api.getIssue(4)).labels, ['ai:go', 'demande']);
  assert.deepEqual((await api.listOpenIssues()).map((i) => i.number), [4], 'pull requests are not issues');
  await api.removeLabel(4, 'ai:absent');
  assert.equal(await api.findBranch('ai/4-'), 'ai/4-titre');
  assert.deepEqual(await api.listOpenPullsByHeadPrefix('ai/4-'), [{ number: 8, draft: undefined }], 'forks and other branches ignored');
  await api.deleteRelease(1, 'test-issue-4-tour-1');
  assert.deepEqual(await api.listActiveRuns(), [{ id: 1, title: 'ai-flow #4 · labeled ai:plan' }]);
  assert.equal(github.calls.find((c) => c.key === 'actions.listWorkflowRuns').params.workflow_id, 'ai-router.yml');
});

test('run() routes a bot label to an agent and sets the step outputs', async () => {
  const labels = ['ai:plan'];
  const github = fakeOctokit({
    'issues.get': () => ({ number: 6, title: '[Demande] Compteur', state: 'open', labels: labels.map((name) => ({ name })), created_at: 'c', updated_at: 'u' }),
    'issues.listEvents': () => [
      { event: 'labeled', label: { name: 'ai:specs' }, actor: { login: BOT }, created_at: '1' },
      { event: 'labeled', label: { name: 'ai:plan' }, actor: { login: BOT }, created_at: '2' },
    ],
    'git.listMatchingRefs': () => [],
    'git.getRef': ({ ref }) => {
      assert.equal(ref, 'heads/integration');
      return { object: { sha: 'abc' } };
    },
    'git.createRef': ({ ref, sha }) => {
      assert.deepEqual([ref, sha], ['refs/heads/ai/6-compteur', 'abc']);
      return {};
    },
  });
  const outputs = {};
  const core = { info() {}, error() {}, setFailed: (m) => assert.fail(m), setOutput: (k, v) => (outputs[k] = v) };
  const context = {
    eventName: 'issues',
    payload: { action: 'labeled', issue: { number: 6 }, label: { name: 'ai:plan' }, sender: { login: BOT } },
    repo: { owner: 'o', repo: 'r' },
    serverUrl: 'https://github.com',
    runId: 77,
  };

  await run({ github, context, core, isAgentAvailable: () => true });
  assert.deepEqual(outputs, { agent: 'plan', issue_number: '6', branch: 'ai/6-compteur', run_dir: '.ai-flow/runs/6', tour: '1' });
});

test('run() reports a failure on the issue and fails the step', async () => {
  let failed = '';
  const comments = [];
  const github = fakeOctokit({
    'issues.get': () => ({ number: 6, title: 'T', state: 'open', labels: [{ name: 'ai:plan' }], created_at: 'c', updated_at: 'u' }),
    'issues.listEvents': () => {
      throw new Error('API down');
    },
    'issues.addLabels': () => ({}),
    'issues.removeLabel': () => ({}),
    'issues.createComment': ({ body }) => comments.push(body),
  });
  const core = { info() {}, error() {}, setFailed: (m) => (failed = m), setOutput() {} };
  const context = {
    eventName: 'issues',
    payload: { action: 'labeled', issue: { number: 6 }, label: { name: 'ai:plan' }, sender: { login: BOT } },
    repo: { owner: 'o', repo: 'r' },
    serverUrl: 'https://github.com',
    runId: 78,
  };

  await run({ github, context, core, isAgentAvailable: () => true });
  assert.match(failed, /API down/);
  assert.match(comments.at(-1), /ai-flow:error step=ai:plan/);
});

test('agent workflow files follow the names of the specification', () => {
  assert.equal(agentWorkflowFile('specs'), 'ai-agent-specs.yml');
  assert.equal(agentWorkflowFile('deliver'), 'ai-deliver.yml');
});
