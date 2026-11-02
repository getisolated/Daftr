// Entry point of the router, run by actions/github-script in ai-router.yml:
//
//   const { run } = await import(`${process.env.GITHUB_WORKSPACE}/scripts/ai-flow/router-action.mjs`);
//   await run({ github, context, core });
//
// Turns the GitHub event into a router event, runs it, and exposes the agent to dispatch
// as step outputs: agent, issue_number, branch, run_dir, tour (agent is empty when none).

import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { loadFlowEnv, loadLabels } from './lib/flow-config.mjs';
import { createGithubApi } from './lib/github-api.mjs';
import { createRouter } from './lib/router.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

/** Normalised router event from the GitHub Actions context. */
export function eventFromContext(context) {
  if (context.eventName === 'issues') {
    const { action, issue, label, sender } = context.payload;
    return { kind: 'issues', action, issueNumber: issue.number, label: label?.name ?? null, sender: sender?.login ?? '' };
  }
  return { kind: 'tick' };
}

/** Workflow file of each agent: ai-agent-<name>.yml, and ai-deliver.yml for the delivery. */
export const agentWorkflowFile = (agent) => (agent === 'deliver' ? 'ai-deliver.yml' : `ai-agent-${agent}.yml`);
const agentWorkflowExists = (agent) => existsSync(join(ROOT, '.github', 'workflows', agentWorkflowFile(agent)));

export async function run({ github, context, core, isAgentAvailable = agentWorkflowExists }) {
  const config = loadFlowEnv(ROOT);
  const labels = loadLabels(ROOT);
  const { owner, repo } = context.repo;
  const runUrl = `${context.serverUrl}/${owner}/${repo}/actions/runs/${context.runId}`;

  const api = createGithubApi({ github, owner, repo });
  const router = createRouter({
    api,
    config,
    labels,
    pmLogins: process.env.AI_FLOW_PM_LOGINS ?? '',
    runUrl,
    runId: context.runId,
    isAgentAvailable,
  });

  const event = eventFromContext(context);
  core.info(`Event: ${JSON.stringify(event)}`);

  let result;
  try {
    result = await router.handle(event);
  } catch (error) {
    core.error(error.stack ?? String(error));
    if (event.kind === 'issues') {
      await router.reportFailure(event.issueNumber, error).catch((e) => core.error(`reportFailure: ${e}`));
    }
    core.setFailed(`Router failed: ${error.message ?? error}`);
    return;
  }

  const dispatch = result?.dispatch;
  core.setOutput('agent', dispatch?.agent ?? '');
  for (const key of ['issue_number', 'branch', 'run_dir', 'tour']) {
    core.setOutput(key, dispatch ? String(dispatch[key]) : '');
  }
  core.info(dispatch ? `Dispatch: ${JSON.stringify(dispatch)}` : 'No agent to run.');
}
