// Entry point of the finalisation step of ai-agent-run.yml (actions/github-script):
//
//   const { run } = await import(`${process.env.GITHUB_WORKSPACE}/scripts/ai-flow/finalize-action.mjs`);
//   await run({ github, context, core });
//
// Inputs come from the environment (never interpolated into the script): AI_FLOW_AGENT,
// AI_FLOW_ISSUE, AI_FLOW_TOUR, AI_FLOW_RUN_DIR, AI_FLOW_BRANCH, STRUCTURED_OUTPUT (output of
// claude-code-action), EXECUTION_FILE, FORBIDDEN (one path per line), PUSH_ERROR, CHECKS_STATUS
// (pass | fail | empty: outcome of the fast check re-run after the code agent).

import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { loadFlowEnv, loadLabels } from './lib/flow-config.mjs';
import { createFinalizer } from './lib/finalize.mjs';
import { createGithubApi } from './lib/github-api.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
/** Checkout of the flow branch (run files); the flow's own files come from ROOT. */
const WORKDIR = process.env.AI_FLOW_WORKDIR || ROOT;
const RUN_FILES = ['spec.md', 'plan.md', 'tests.md', 'code-notes.md', 'checks.md', 'review.md', 'rapport-pm.md'];
/** Written by an agent when structured output is unavailable (same schema). */
const FALLBACK_RESULT = '/tmp/ai-flow-result.json';

function parseJson(text) {
  if (!text?.trim()) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** Last "result" message of the execution file (a JSON array, or one message per line). */
export function readExecution(text) {
  if (!text?.trim()) return null;
  let messages = parseJson(text);
  if (!Array.isArray(messages)) {
    messages = text
      .split(/\r?\n/)
      .map(parseJson)
      .filter(Boolean);
  }
  return messages.filter((m) => m?.type === 'result').at(-1) ?? null;
}

/** Structured result: claude-code-action output first, then the fallback file. */
export function readResult(structuredOutput, fallbackText) {
  for (const candidate of [parseJson(structuredOutput), parseJson(fallbackText)]) {
    if (candidate && typeof candidate === 'object' && typeof candidate.status === 'string') return candidate;
  }
  return null;
}

const readIfExists = (path) => (path && existsSync(path) ? readFileSync(path, 'utf8') : '');

export async function run({ github, context, core }) {
  const env = process.env;
  const config = loadFlowEnv(ROOT);
  const labels = loadLabels(ROOT);
  const { owner, repo } = context.repo;
  const runDir = env.AI_FLOW_RUN_DIR;

  const files = {};
  for (const name of RUN_FILES) {
    const content = readIfExists(join(WORKDIR, runDir, name));
    if (content) files[name] = content;
  }

  const finalizer = createFinalizer({
    api: createGithubApi({ github, owner, repo }),
    config,
    labels,
    runUrl: `${context.serverUrl}/${owner}/${repo}/actions/runs/${context.runId}`,
    branchUrl: `${context.serverUrl}/${owner}/${repo}/blob/${env.AI_FLOW_BRANCH}`,
  });

  const input = {
    agent: env.AI_FLOW_AGENT,
    issueNumber: Number(env.AI_FLOW_ISSUE),
    tour: Number(env.AI_FLOW_TOUR),
    runDir,
    branch: env.AI_FLOW_BRANCH,
    checks: { status: env.CHECKS_STATUS?.trim() || null, report: files['checks.md'] ?? '' },
    result: readResult(env.STRUCTURED_OUTPUT, readIfExists(FALLBACK_RESULT)),
    execution: readExecution(readIfExists(env.EXECUTION_FILE)),
    forbidden: (env.FORBIDDEN ?? '').split('\n').map((s) => s.trim()).filter(Boolean),
    pushError: env.PUSH_ERROR?.trim() || null,
    files,
  };
  core.info(`Result: ${JSON.stringify(input.result)}`);
  core.info(`Execution: ${JSON.stringify({ ...input.execution, result: undefined })}`);

  const outcome = await finalizer.finalize(input);
  core.info(`Outcome: ${JSON.stringify(outcome)}`);
  core.setOutput('outcome', outcome.outcome);
  if (outcome.outcome === 'error') core.setFailed('The agent run ended in ai:erreur (see the issue comment).');
}
