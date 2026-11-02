// GitHub operations used by the router, on top of the Octokit client that
// actions/github-script provides (authenticated with the flow's App token).
//
// This is the contract that the router relies on; the tests use an in-memory fake with the
// same methods (scripts/ai-flow/router.test.mjs).

const ACTIVE_RUN_STATUSES = ['queued', 'in_progress', 'waiting', 'requested', 'pending'];

function issueOf(data) {
  return {
    number: data.number,
    title: data.title,
    state: data.state,
    labels: (data.labels ?? []).map((label) => (typeof label === 'string' ? label : label.name)),
    author: data.user?.login ?? '',
    createdAt: data.created_at,
    updatedAt: data.updated_at,
  };
}

function ignore(...statuses) {
  return (error) => {
    if (!statuses.includes(error.status)) throw error;
  };
}

/**
 * @param {object} deps
 * @param {object} deps.github   Octokit client (github-script's `github`)
 * @param {string} deps.owner
 * @param {string} deps.repo
 * @param {string} deps.workflow file name of the router workflow (runs to cancel)
 */
export function createGithubApi({ github, owner, repo, workflow = 'ai-router.yml' }) {
  const base = { owner, repo };

  return {
    async getIssue(number) {
      const { data } = await github.rest.issues.get({ ...base, issue_number: number });
      return issueOf(data);
    },

    async listOpenIssues() {
      const items = await github.paginate(github.rest.issues.listForRepo, { ...base, state: 'open', per_page: 100 });
      return items.filter((item) => !item.pull_request).map(issueOf);
    },

    async addLabel(number, name) {
      await github.rest.issues.addLabels({ ...base, issue_number: number, labels: [name] });
    },

    async removeLabel(number, name) {
      await github.rest.issues.removeLabel({ ...base, issue_number: number, name }).catch(ignore(404));
    },

    async comment(number, body) {
      await github.rest.issues.createComment({ ...base, issue_number: number, body });
    },

    async listComments(number) {
      const items = await github.paginate(github.rest.issues.listComments, { ...base, issue_number: number, per_page: 100 });
      return items.map((c) => ({ id: c.id, author: c.user?.login ?? '', body: c.body ?? '', createdAt: c.created_at, url: c.html_url }));
    },

    async listLabelEvents(number) {
      const items = await github.paginate(github.rest.issues.listEvents, { ...base, issue_number: number, per_page: 100 });
      return items
        .filter((e) => (e.event === 'labeled' || e.event === 'unlabeled') && e.label)
        .map((e) => ({ action: e.event, label: e.label.name, actor: e.actor?.login ?? '', createdAt: e.created_at }));
    },

    async findBranch(prefix) {
      const { data } = await github.rest.git.listMatchingRefs({ ...base, ref: `heads/${prefix}` });
      return data[0]?.ref.replace(/^refs\/heads\//, '') ?? null;
    },

    async createBranch(name, from) {
      const { data } = await github.rest.git.getRef({ ...base, ref: `heads/${from}` });
      await github.rest.git.createRef({ ...base, ref: `refs/heads/${name}`, sha: data.object.sha });
    },

    async listActiveRuns() {
      const runs = [];
      for (const status of ACTIVE_RUN_STATUSES) {
        const items = await github.paginate(github.rest.actions.listWorkflowRuns, { ...base, workflow_id: workflow, status, per_page: 100 });
        runs.push(...items.map((run) => ({ id: run.id, title: run.display_title ?? run.name ?? '' })));
      }
      return runs;
    },

    async cancelRun(id) {
      await github.rest.actions.cancelWorkflowRun({ ...base, run_id: id }).catch(ignore(409));
    },

    async listOpenPullsByHeadPrefix(prefix) {
      const items = await github.paginate(github.rest.pulls.list, { ...base, state: 'open', per_page: 100 });
      return items
        .filter((pull) => pull.head?.ref?.startsWith(prefix) && pull.head?.repo?.full_name === `${owner}/${repo}`)
        .map((pull) => ({ number: pull.number, draft: pull.draft }));
    },

    async closePull(number) {
      await github.rest.pulls.update({ ...base, pull_number: number, state: 'closed' });
    },

    async listReleases() {
      const items = await github.paginate(github.rest.repos.listReleases, { ...base, per_page: 100 });
      return items.map((release) => ({ id: release.id, tag: release.tag_name, prerelease: release.prerelease }));
    },

    async deleteRelease(id, tag) {
      await github.rest.repos.deleteRelease({ ...base, release_id: id }).catch(ignore(404));
      await github.rest.git.deleteRef({ ...base, ref: `tags/${tag}` }).catch(ignore(404, 422));
    },
  };
}
