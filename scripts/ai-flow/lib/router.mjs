// Router of the AI flow: the state machine carried by the ai:* labels of an issue.
//
// One call to `handle(event)` per GitHub event. The router reads the issue, applies the
// transition (labels, comments, branch, cleanup) through the injected `api`, and returns
// the agent to run, if any. It holds no state of its own: everything lives in the labels,
// the label history of the issue and a few hidden markers in the bot's comments, so a run
// can be replayed and the future web UI (phase 2) can read the same state from the API.
//
// Rules (see docs/ai-flow/DECISIONS.md and the specification):
// - one position label per issue: a state set by the bot, or a PM decision being handled;
// - only one active flow at a time; the others wait in ai:file-d-attente;
// - PM decisions are accepted only from AI_FLOW_PM_LOGINS, machine labels only from the bot;
// - the bot's own labels wake the router only for agent states and ai:livré.

export const LABEL = Object.freeze({
  DEMANDE: 'demande',
  GO: 'ai:go',
  QUEUED: 'ai:file-d-attente',
  SPECS: 'ai:specs',
  QUESTION: 'ai:question',
  ANSWERED: 'ai:réponse-donnée',
  PLAN: 'ai:plan',
  TESTS: 'ai:tests',
  CODE: 'ai:code',
  REVIEW: 'ai:review',
  DOCS: 'ai:docs',
  DEPLOY: 'ai:deploy',
  TO_VALIDATE: 'ai:à-valider',
  VALIDATED: 'ai:validé',
  PROBLEM: 'ai:problème',
  ESCALATED: 'ai:escalade',
  FORCE: 'ai:validé-quand-même',
  ERROR: 'ai:erreur',
  DELIVERED: 'ai:livré',
  STOP: 'ai:stop',
});

/** Agent run for each agent state label. */
export const AGENT_BY_STATE = Object.freeze({
  [LABEL.SPECS]: 'specs',
  [LABEL.PLAN]: 'plan',
  [LABEL.TESTS]: 'tests',
  [LABEL.CODE]: 'code',
  [LABEL.REVIEW]: 'review',
  [LABEL.DOCS]: 'docs',
  [LABEL.DEPLOY]: 'deploy',
});

/** Labels set by the bot that wake the router. Must match the `if:` of ai-router.yml. */
export const BOT_TRIGGER_LABELS = Object.freeze([...Object.keys(AGENT_BY_STATE), LABEL.DELIVERED]);

/** States waiting for the PM: reminded after REMINDER_HOURS without activity. */
export const WAITING_STATES = Object.freeze([LABEL.QUESTION, LABEL.TO_VALIDATE, LABEL.ESCALATED, LABEL.ERROR]);

/** Positions that do not hold the lock. */
const INACTIVE_POSITIONS = new Set([LABEL.QUEUED, LABEL.DELIVERED, LABEL.STOP]);

const REMINDER_HOURS = 48;
/** An agent state with no router run for this long is considered interrupted. */
const STALL_MINUTES = 20;

const MARKER_RE = /<!-- ai-flow:(\w[\w-]*)((?:\s+[\w-]+=\S+)*)\s*-->/g;

/** Hidden marker embedded in a bot comment: <!-- ai-flow:error step=ai:code --> */
export function marker(kind, fields = {}) {
  const attrs = Object.entries(fields).map(([k, v]) => ` ${k}=${String(v).replace(/\s+/g, '_')}`).join('');
  return `<!-- ai-flow:${kind}${attrs} -->`;
}

export function parseMarkers(body) {
  const found = [];
  for (const match of body.matchAll(MARKER_RE)) {
    const fields = {};
    for (const pair of match[2].trim().split(/\s+/).filter(Boolean)) {
      const eq = pair.indexOf('=');
      fields[pair.slice(0, eq)] = pair.slice(eq + 1);
    }
    found.push({ kind: match[1], fields });
  }
  return found;
}

/** Names of the flow labels (state and decision roles) of a label catalog. */
export function flowLabelNames(labels) {
  return new Set(labels.filter((l) => l.role === 'state' || l.role === 'decision').map((l) => l.name));
}

/**
 * Makes `label` the only position of an issue: adds it first, so the lock never lapses,
 * then removes every other flow label. Shared by the router and the agent finalisation.
 */
export async function applyPosition(api, flowLabels, number, label) {
  const issue = await api.getIssue(number);
  if (!issue.labels.includes(label)) await api.addLabel(number, label);
  for (const name of issue.labels) {
    if (name !== label && flowLabels.has(name)) await api.removeLabel(number, name);
  }
}

/** ASCII slug of an issue title, for the branch name ai/<n>-<slug>. */
export function slugify(title) {
  const slug = title
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/^\s*\[demande\]\s*/, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40)
    .replace(/-+$/g, '');
  return slug || 'demande';
}

export function parseLogins(value) {
  return (value ?? '')
    .split(',')
    .map((login) => login.trim().toLowerCase())
    .filter(Boolean);
}

/**
 * @param {object} deps
 * @param {object} deps.api      GitHub operations (see github-api.mjs for the contract)
 * @param {object} deps.config   values of .ai-flow/flow.env
 * @param {Array}  deps.labels   catalog parsed from .ai-flow/labels.txt
 * @param {string} deps.pmLogins value of the AI_FLOW_PM_LOGINS variable
 * @param {string} deps.runUrl   URL of the current Actions run (for comments)
 * @param {number} [deps.runId]  id of the current run (never cancelled by ai:stop)
 * @param {(agent: string) => boolean} [deps.isAgentAvailable] whether the workflow of an
 *        agent exists yet (the flow is delivered batch by batch)
 * @param {() => Date} [deps.now]
 */
export function createRouter({
  api,
  config,
  labels,
  pmLogins,
  runUrl,
  runId,
  isAgentAvailable = () => true,
  now = () => new Date(),
}) {
  const catalog = new Map(labels.map((label) => [label.name, label]));
  const flowLabels = flowLabelNames(labels);
  const pms = parseLogins(pmLogins);
  const botLogin = (config.AI_FLOW_BOT_LOGIN ?? '').toLowerCase();
  const prefix = config.AI_FLOW_BRANCH_PREFIX ?? 'ai/';
  const runsDir = config.AI_FLOW_RUNS_DIR ?? '.ai-flow/runs';
  const limits = {
    specs: Number(config.AI_FLOW_MAX_SPEC_ROUNDS ?? 3),
    review: Number(config.AI_FLOW_MAX_REVIEW_LOOPS ?? 3),
    pm: Number(config.AI_FLOW_MAX_PM_RETURNS ?? 3),
  };

  const isBot = (login) => (login ?? '').toLowerCase() === botLogin;
  const isPm = (login) => pms.includes((login ?? '').toLowerCase());
  const mention = () => pms.map((login) => `@${login}`).join(' ');
  const footer = runUrl ? `\n\n<sub>[Run du routeur](${runUrl})</sub>` : '';
  const none = { dispatch: null };

  /** Position labels of an issue (every flow label except ai:go). */
  const positionsOf = (issue) => issue.labels.filter((name) => flowLabels.has(name) && name !== LABEL.GO);
  const isActive = (issue) =>
    issue.state === 'open' && positionsOf(issue).some((name) => !INACTIVE_POSITIONS.has(name));

  async function say(number, text, mark = '') {
    await api.comment(number, `${text}${mark ? `\n\n${mark}` : ''}${footer}`);
  }

  /** Sets `label` as the only position of the issue: add first (the lock never lapses), then clean. */
  const setPosition = (number, label) => applyPosition(api, flowLabels, number, label);

  async function setError(number, step, reason) {
    await setPosition(number, LABEL.ERROR);
    await say(
      number,
      `❌ **Erreur** pendant l'étape \`${step}\` : ${reason}\n\nPour relancer l'étape, retirez le label \`${LABEL.ERROR}\`.`,
      marker('error', { step }),
    );
  }

  async function escalate(number, from, reason) {
    await setPosition(number, LABEL.ESCALATED);
    await say(
      number,
      `🧭 **Escalade** : ${reason}\n\n${mention()} à vous de trancher : \`${LABEL.FORCE}\` pour continuer quand même, ou \`${LABEL.PROBLEM}\` avec un commentaire qui explique quoi changer.`,
      marker('escalade', { from }),
    );
  }

  async function activeIssues(excludeNumber) {
    return (await api.listOpenIssues()).filter((issue) => issue.number !== excludeNumber && isActive(issue));
  }

  /** Queued issues, oldest entry in the queue first. */
  async function queuedIssues() {
    const queued = (await api.listOpenIssues()).filter((issue) => issue.labels.includes(LABEL.QUEUED));
    const withDate = await Promise.all(
      queued.map(async (issue) => {
        const events = await api.listLabelEvents(issue.number);
        const since = lastLabeledAt(events, LABEL.QUEUED) ?? issue.createdAt ?? '';
        return { issue, since };
      }),
    );
    return withDate
      .sort((a, b) => (a.since < b.since ? -1 : a.since > b.since ? 1 : a.issue.number - b.issue.number))
      .map((entry) => entry.issue);
  }

  async function ensureBranch(issue) {
    const existing = await api.findBranch(`${prefix}${issue.number}-`);
    if (existing) return existing;
    const name = `${prefix}${issue.number}-${slugify(issue.title)}`;
    await api.createBranch(name, config.AI_FLOW_INTEGRATION_BRANCH ?? 'integration');
    return name;
  }

  async function startFlow(issue, intro) {
    const branch = await ensureBranch(issue);
    await setPosition(issue.number, LABEL.SPECS);
    await say(
      issue.number,
      `${intro}\n\nL'agent **specs** analyse la demande ; ses questions arriveront ici. Branche de travail : \`${branch}\`.`,
    );
  }

  /** Gives the lock to the oldest queued issue, if the lock is free. */
  async function promoteNext(finishedNumber) {
    if ((await activeIssues()).length > 0) return;
    const [next] = await queuedIssues();
    if (!next) return;
    const why = finishedNumber ? ` : la demande #${finishedNumber} est terminée` : '';
    await startFlow(next, `🚀 **C'est votre tour**${why}. Le flux IA démarre.`);
  }

  /** Cancels the runs of the issue, closes its PR and deletes its test pre-releases. */
  async function cleanup(number) {
    const pattern = new RegExp(`#${number}(?!\\d)`);
    let cancelled = 0;
    for (const run of await api.listActiveRuns()) {
      if (run.id !== runId && pattern.test(run.title)) {
        await api.cancelRun(run.id);
        cancelled += 1;
      }
    }
    const pulls = await api.listOpenPullsByHeadPrefix(`${prefix}${number}-`);
    for (const pull of pulls) await api.closePull(pull.number);
    let releases = 0;
    for (const release of await api.listReleases()) {
      if (release.tag.startsWith(`test-issue-${number}-`)) {
        await api.deleteRelease(release.id, release.tag);
        releases += 1;
      }
    }
    return { cancelled, pulls: pulls.map((p) => p.number), releases };
  }

  function cleanupSummary({ cancelled, pulls, releases }) {
    const parts = [`runs annulés : ${cancelled}`];
    parts.push(pulls.length ? `PR fermée : ${pulls.map((n) => `#${n}`).join(', ')}` : 'aucune PR ouverte');
    parts.push(`versions de test supprimées : ${releases}`);
    return parts.join(' · ');
  }

  /**
   * Label history of the current flow only: after the last accepted ai:stop (set by a PM)
   * or ai:livré (set by the bot). A flow restarted with ai:go starts its counters at zero.
   */
  function currentFlow(events) {
    let start = 0;
    events.forEach((event, index) => {
      if (event.action !== 'labeled') return;
      if ((event.label === LABEL.STOP && isPm(event.actor)) || (event.label === LABEL.DELIVERED && isBot(event.actor))) {
        start = index + 1;
      }
    });
    return events.slice(start);
  }

  async function latestMarker(number, kind) {
    const comments = await api.listComments(number);
    for (let i = comments.length - 1; i >= 0; i -= 1) {
      if (!isBot(comments[i].author)) continue;
      const found = parseMarkers(comments[i].body).find((m) => m.kind === kind);
      if (found) return found.fields;
    }
    return null;
  }

  async function notInstalled(number, agent) {
    await say(number, `⚙️ L'agent **${agent}** n'est pas encore installé dans ce dépôt : le flux attend ici. Posez \`${LABEL.STOP}\` pour libérer la place.`);
    return none;
  }

  async function dispatchAgent(issue, label) {
    const agent = AGENT_BY_STATE[label];
    if (!isAgentAvailable(agent)) return notInstalled(issue.number, agent);
    const events = currentFlow(await api.listLabelEvents(issue.number));
    const planTour = Math.max(1, countLabeled(events, LABEL.PLAN));
    let tour;
    switch (label) {
      case LABEL.SPECS:
        tour = countLabeled(events, LABEL.SPECS);
        if (tour > limits.specs) {
          await escalate(issue.number, 'specs', `la spec n'est pas finalisée après ${limits.specs} tours de questions.`);
          return none;
        }
        break;
      case LABEL.CODE:
      case LABEL.REVIEW:
        tour = countLabeledSince(events, label, LABEL.PLAN);
        if (label === LABEL.CODE && tour - 1 > limits.review) {
          await escalate(
            issue.number,
            'review',
            `la review a renvoyé le code ${limits.review} fois sans être satisfaite.`,
          );
          return none;
        }
        break;
      default:
        tour = planTour;
    }
    const branch = await ensureBranch(issue);
    return {
      dispatch: {
        agent,
        issue_number: issue.number,
        branch,
        run_dir: `${runsDir}/${issue.number}`,
        tour: Math.max(1, tour),
      },
    };
  }

  async function deliver(issue) {
    if (!isAgentAvailable('deliver')) return notInstalled(issue.number, 'deliver');
    const branch = await ensureBranch(issue);
    const events = currentFlow(await api.listLabelEvents(issue.number));
    return {
      dispatch: {
        agent: 'deliver',
        issue_number: issue.number,
        branch,
        run_dir: `${runsDir}/${issue.number}`,
        tour: Math.max(1, countLabeled(events, LABEL.PLAN)),
      },
    };
  }

  // ---- PM decisions -------------------------------------------------------------------

  async function rejectDecision(issue, label, text) {
    await api.removeLabel(issue.number, label);
    await say(issue.number, text);
    return none;
  }

  async function onGo(issue) {
    const positions = positionsOf(issue);
    const current = positions.find((name) => name !== LABEL.STOP && name !== LABEL.DELIVERED);
    if (current) {
      return rejectDecision(issue, LABEL.GO, `ℹ️ Le flux est déjà engagé sur cette demande (état \`${current}\`) : \`${LABEL.GO}\` retiré.`);
    }
    const others = await activeIssues(issue.number);
    if (others.length > 0) {
      await setPosition(issue.number, LABEL.QUEUED);
      const queue = await queuedIssues();
      const rank = queue.findIndex((q) => q.number === issue.number) + 1 || queue.length;
      await say(
        issue.number,
        `⏳ Une autre demande est en cours (#${others[0].number}). Celle-ci est **en file d'attente, position ${rank}** : elle démarrera automatiquement.`,
      );
      return none;
    }
    await startFlow(issue, '🚀 **Flux IA démarré.**');
    return none;
  }

  async function onAnswered(issue, current) {
    if (current !== LABEL.QUESTION) {
      return rejectDecision(issue, LABEL.ANSWERED, unexpected(LABEL.ANSWERED, current));
    }
    await setPosition(issue.number, LABEL.SPECS);
    return none;
  }

  async function onValidated(issue, current) {
    if (current !== LABEL.TO_VALIDATE) {
      return rejectDecision(issue, LABEL.VALIDATED, unexpected(LABEL.VALIDATED, current));
    }
    await setPosition(issue.number, LABEL.VALIDATED);
    await say(issue.number, '✅ Validé par le PM : fusion dans la branche `integration` en cours.');
    return deliver(issue);
  }

  async function onForce(issue, current) {
    if (current !== LABEL.ESCALATED) {
      return rejectDecision(issue, LABEL.FORCE, unexpected(LABEL.FORCE, current));
    }
    const from = (await latestMarker(issue.number, 'escalade'))?.from;
    if (from === 'review') {
      await setPosition(issue.number, LABEL.DOCS);
      await say(issue.number, '➡️ Le PM continue malgré la review : passage à la documentation.');
      return none;
    }
    if (from === 'specs') {
      await setPosition(issue.number, LABEL.PLAN);
      await say(issue.number, '➡️ Le PM continue avec la spec en l\'état : passage au plan.');
      return none;
    }
    if (from === 'deploy') {
      await setPosition(issue.number, LABEL.VALIDATED);
      await say(issue.number, '✅ Le PM accepte la version : fusion dans la branche `integration` en cours.');
      return deliver(issue);
    }
    return rejectDecision(issue, LABEL.FORCE, '⚠️ Origine de l\'escalade introuvable : label retiré. Utilisez `ai:problème` avec un commentaire.');
  }

  async function onProblem(issue, current) {
    if (current !== LABEL.TO_VALIDATE && current !== LABEL.ESCALATED) {
      return rejectDecision(issue, LABEL.PROBLEM, unexpected(LABEL.PROBLEM, current));
    }
    const events = currentFlow(await api.listLabelEvents(issue.number));
    const since = lastLabeledAt(events, current) ?? '';
    const comments = await api.listComments(issue.number);
    const feedback = comments.filter((c) => isPm(c.author) && c.createdAt >= since).at(-1);
    if (!feedback) {
      return rejectDecision(
        issue,
        LABEL.PROBLEM,
        `✍️ Décrivez d'abord le problème dans un commentaire (ce que vous avez fait, ce que vous attendiez, ce qui s'est passé), puis reposez \`${LABEL.PROBLEM}\`.`,
      );
    }
    const returns = Math.max(0, countLabeled(events, LABEL.PLAN) - 1);
    if (current === LABEL.TO_VALIDATE && returns >= limits.pm) {
      await escalate(
        issue.number,
        'deploy',
        `${returns} retours PM déjà faits sur cette demande (maximum ${limits.pm}). Votre dernier commentaire est conservé.`,
      );
      return none;
    }
    await setPosition(issue.number, LABEL.PLAN);
    await say(
      issue.number,
      `🔁 Retour au **plan** (tour ${returns + 2}) avec votre commentaire : l'agent plan l'ajoute à \`plan.md\`, puis tests, code et review ajustent l'existant.`,
      marker('pm-return', { tour: returns + 1, comment: feedback.url ?? feedback.id }),
    );
    return none;
  }

  async function onStop(issue) {
    const wasActive = isActive(issue);
    const summary = await cleanup(issue.number);
    await setPosition(issue.number, LABEL.STOP);
    await say(issue.number, `🛑 **Flux arrêté** par le PM (${cleanupSummary(summary)}). Pour reprendre plus tard, posez \`${LABEL.GO}\`.`);
    if (wasActive) await promoteNext(issue.number);
    return none;
  }

  function unexpected(label, current) {
    return `ℹ️ \`${label}\` n'est pas attendu à l'état actuel (\`${current ?? 'aucun'}\`) : label retiré.`;
  }

  // ---- event handlers -----------------------------------------------------------------

  async function onLabeled(issue, label, sender) {
    const definition = catalog.get(label);
    if (!definition || !label.startsWith('ai:')) return none;

    if (isBot(sender)) {
      if (!issue.labels.includes(label)) return none; // stale: the label is already gone
      if (AGENT_BY_STATE[label]) return dispatchAgent(issue, label);
      if (label === LABEL.DELIVERED) {
        await cleanup(issue.number);
        await promoteNext(issue.number);
      }
      return none;
    }

    if (!issue.labels.includes(label)) return none; // stale: removed since the event

    if (definition.role !== 'decision') {
      return rejectDecision(
        issue,
        label,
        `⛔ \`${label}\` est réservé au flux et ne peut pas être posé à la main : label retiré. Les labels du PM sont \`${LABEL.GO}\`, \`${LABEL.ANSWERED}\`, \`${LABEL.VALIDATED}\`, \`${LABEL.PROBLEM}\`, \`${LABEL.FORCE}\` et \`${LABEL.STOP}\`.`,
      );
    }
    if (!isPm(sender)) {
      return rejectDecision(issue, label, `⛔ @${sender}, seul le PM peut poser \`${label}\` : label retiré.`);
    }
    if (issue.state !== 'open') {
      return rejectDecision(issue, label, `ℹ️ La demande est fermée : \`${label}\` retiré.`);
    }

    const current = positionsOf(issue).find((name) => name !== label);
    switch (label) {
      case LABEL.GO:
        return onGo(issue);
      case LABEL.ANSWERED:
        return onAnswered(issue, current);
      case LABEL.VALIDATED:
        return onValidated(issue, current);
      case LABEL.FORCE:
        return onForce(issue, current);
      case LABEL.PROBLEM:
        return onProblem(issue, current);
      case LABEL.STOP:
        return onStop(issue);
      default:
        return none;
    }
  }

  async function onErrorCleared(issue, sender) {
    if (!isPm(sender)) {
      await api.addLabel(issue.number, LABEL.ERROR);
      await say(issue.number, `⛔ @${sender}, seul le PM peut relancer une étape en erreur : label remis.`);
      return none;
    }
    if (issue.state !== 'open' || positionsOf(issue).length > 0) return none;
    const step = (await latestMarker(issue.number, 'error'))?.step;
    if (step && AGENT_BY_STATE[step]) {
      await say(issue.number, `🔁 Relance de l'étape \`${step}\`.`);
      await setPosition(issue.number, step); // the bot's label wakes the router for the agent
      return none;
    }
    if (step === LABEL.VALIDATED) {
      await say(issue.number, '🔁 Relance de la fusion dans `integration`.');
      await setPosition(issue.number, LABEL.VALIDATED);
      return deliver(issue);
    }
    await setPosition(issue.number, LABEL.ERROR);
    await say(issue.number, `⚠️ Étape en erreur introuvable : impossible de relancer automatiquement. Posez \`${LABEL.STOP}\` puis \`${LABEL.GO}\` pour repartir de zéro.`);
    return none;
  }

  async function onClosed(issue) {
    const positions = positionsOf(issue);
    if (positions.includes(LABEL.QUEUED)) {
      await api.removeLabel(issue.number, LABEL.QUEUED);
      return none;
    }
    if (!positions.some((name) => !INACTIVE_POSITIONS.has(name))) return none;
    const summary = await cleanup(issue.number);
    await setPosition(issue.number, LABEL.STOP);
    await say(issue.number, `🛑 Demande fermée : flux arrêté (${cleanupSummary(summary)}).`);
    await promoteNext(issue.number);
    return none;
  }

  /** Hourly housekeeping: interrupted agents, PM reminders, a free lock with a queue. */
  async function tick() {
    const issues = await api.listOpenIssues();
    const runs = await api.listActiveRuns();
    const nowMs = now().getTime();

    for (const issue of issues) {
      const position = positionsOf(issue)[0];
      if (!position) continue;

      if (AGENT_BY_STATE[position]) {
        if (!isAgentAvailable(AGENT_BY_STATE[position])) continue;
        const running = runs.some((run) => new RegExp(`#${issue.number}(?!\\d)`).test(run.title));
        const idleMinutes = (nowMs - Date.parse(issue.updatedAt)) / 60000;
        if (!running && idleMinutes > STALL_MINUTES) {
          await setError(issue.number, position, 'l\'étape ne tourne plus (run perdu, annulé ou expiré).');
        }
        continue;
      }

      if (WAITING_STATES.includes(position)) {
        const [comments, events] = await Promise.all([api.listComments(issue.number), api.listLabelEvents(issue.number)]);
        const last = [...comments.map((c) => c.createdAt), ...events.map((e) => e.createdAt), issue.createdAt ?? '']
          .filter(Boolean)
          .sort()
          .at(-1);
        if (last && (nowMs - Date.parse(last)) / 3600000 >= REMINDER_HOURS) {
          await say(issue.number, `⏰ ${mention()} rappel : cette demande attend votre action (\`${position}\`) depuis plus de ${REMINDER_HOURS} h.`);
        }
      }
    }

    await promoteNext();
    return none;
  }

  async function handle(event) {
    if (event.kind !== 'issues') return tick();
    const issue = await api.getIssue(event.issueNumber);
    switch (event.action) {
      case 'opened':
        if (issue.labels.includes(LABEL.DEMANDE)) {
          await say(issue.number, `📥 Demande reçue. Le flux IA démarrera quand le PM posera le label \`${LABEL.GO}\`.`);
        }
        return none;
      case 'labeled':
        return onLabeled(issue, event.label, event.sender);
      case 'unlabeled':
        if (event.label === LABEL.ERROR && !isBot(event.sender)) return onErrorCleared(issue, event.sender);
        return none;
      case 'closed':
        return onClosed(issue);
      default:
        return none;
    }
  }

  /** Called when handle() throws: report on the issue and park it in ai:erreur. */
  async function reportFailure(issueNumber, error) {
    const issue = await api.getIssue(issueNumber);
    if (issue.state !== 'open') return;
    const step = positionsOf(issue).find((name) => AGENT_BY_STATE[name] || name === LABEL.VALIDATED);
    if (!step) {
      await say(issueNumber, `❌ Le routeur du flux IA a rencontré une erreur : ${error.message ?? error}`);
      return;
    }
    await setError(issueNumber, step, `le routeur a rencontré une erreur (${error.message ?? error}).`);
  }

  return { handle, reportFailure };
}

// ---- label history helpers (events in chronological order) ------------------------------

export function countLabeled(events, label) {
  return events.filter((e) => e.action === 'labeled' && e.label === label).length;
}

/** Number of times `label` was added since the last time `anchor` was added. */
export function countLabeledSince(events, label, anchor) {
  let count = 0;
  for (const event of events) {
    if (event.action !== 'labeled') continue;
    if (event.label === anchor) count = 0;
    else if (event.label === label) count += 1;
  }
  return count;
}

export function lastLabeledAt(events, label) {
  return events.filter((e) => e.action === 'labeled' && e.label === label).at(-1)?.createdAt ?? null;
}
