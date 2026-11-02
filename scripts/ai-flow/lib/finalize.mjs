// Finalisation of an agent run: turns the agent's structured result into the next state of
// the issue. The agent itself never sets a label (.ai-flow/CLAUDE.md § 1): this module posts
// its comment (with the cost of the run) and sets the next state label as the bot, which
// wakes the router for the next agent. Any failure parks the issue in ai:erreur with the
// failed step, so that removing the label relaunches it.

import { LABEL, applyPosition, flowLabelNames, marker, parseMarkers } from './router.mjs';

/** Next state for each (agent, status) pair handled so far. */
const TRANSITIONS = {
  specs: { QUESTIONS: LABEL.QUESTION, SPEC_FINALE: LABEL.PLAN },
  plan: { DONE: LABEL.TESTS },
  tests: { DONE: LABEL.CODE },
  code: { DONE: LABEL.REVIEW },
  review: { APPROVED: LABEL.DOCS, CHANGES_REQUESTED: LABEL.CODE },
};

/** Run files that must exist for a successful (agent, status); the first one is linked. */
const REQUIRED_FILES = {
  'specs:SPEC_FINALE': ['spec.md'],
  'plan:DONE': ['plan.md'],
  'tests:DONE': ['tests.md'],
  'review:APPROVED': ['rapport-pm.md', 'review.md'],
  'review:CHANGES_REQUESTED': ['review.md'],
};

const SEVERITY_LABEL = { critical: 'Critique', important: 'Important', suggestion: 'Suggestion' };

/** "[Demande] Compteur de mots" -> "Compteur de mots" */
export function requestTitle(title) {
  return title.replace(/^\s*\[demande\]\s*/i, '').trim() || title;
}

/** First lines of a fast-check report (the result table), for an issue comment. */
function checksTable(report) {
  const lines = (report ?? '').split('\n');
  const end = lines.findIndex((line) => line.startsWith('## '));
  return lines.slice(0, end === -1 ? lines.length : end).filter((line) => !line.startsWith('# ')).join('\n').trim();
}

const usd = (value) => `${value.toFixed(2).replace('.', ',')} $`;

/**
 * Human explanation of a failed Claude run, from the last "result" message of the execution
 * file ({ subtype, is_error, result, num_turns, total_cost_usd }), or null if there is none.
 */
export function diagnose(execution) {
  if (!execution) {
    return "l'agent n'a pas pu s'exécuter (échec du workflow avant ou pendant le lancement de Claude, voir le run).";
  }
  const text = `${execution.result ?? ''} ${execution.subtype ?? ''}`.toLowerCase();
  if (/\b401\b|unauthori[sz]ed|invalid.{0,20}(token|api key|bearer)|oauth token|authentication[_ ]error|token.{0,20}expired/.test(text)) {
    return 'le token Claude Max est invalide ou expiré. Régénérez-le (voir `docs/ai-flow/TOKEN.md`), puis retirez `ai:erreur`.';
  }
  if (/rate[_ -]?limit|usage limit|limit reached|quota|\b429\b|overloaded/.test(text)) {
    return 'quota Claude Max atteint ou service saturé. Attendez la fin de la fenêtre de 5 h (ou de la limite hebdomadaire), puis retirez `ai:erreur`.';
  }
  if (execution.subtype === 'error_max_turns') {
    return `budget de tours épuisé (${execution.num_turns ?? '?'} tours) avant la fin de la mission. Retirer \`ai:erreur\` relance l'étape avec le même budget ; s'il ne suffit pas, augmentez-le dans \`.ai-flow/flow.env\`.`;
  }
  const detail = (execution.result ?? execution.subtype ?? 'erreur inconnue').toString().replace(/\s+/g, ' ').slice(0, 300);
  return `l'agent a échoué : « ${detail} ».`;
}

/** Sum of the costs recorded in the bot's comments of the issue. */
export function recordedCost(comments, botLogin) {
  let total = 0;
  for (const comment of comments) {
    if ((comment.author ?? '').toLowerCase() !== botLogin) continue;
    for (const found of parseMarkers(comment.body)) {
      if (found.kind === 'cost') total += Number(found.fields.usd) || 0;
    }
  }
  return total;
}

/** "# Spec — Titre\n..." -> "..." : the issue comment already has its own heading. */
function withoutTitle(markdown) {
  return markdown.replace(/^\s*# [^\n]*\n+/, '').trim();
}

/**
 * @param {object} deps
 * @param {object} deps.api     same contract as the router (github-api.mjs)
 * @param {object} deps.config  values of .ai-flow/flow.env
 * @param {Array}  deps.labels  catalog of .ai-flow/labels.txt
 * @param {string} deps.runUrl  URL of the agent run
 * @param {string} [deps.branchUrl] URL of the flow branch (links to the run files)
 */
export function createFinalizer({ api, config, labels, runUrl, branchUrl }) {
  const flowLabels = flowLabelNames(labels);
  const botLogin = (config.AI_FLOW_BOT_LOGIN ?? '').toLowerCase();

  /**
   * @param {object} input
   * @param {string} input.agent
   * @param {number} input.issueNumber
   * @param {number} input.tour
   * @param {string} input.runDir        e.g. .ai-flow/runs/12
   * @param {object|null} input.result   structured output { status, comment, summary }
   * @param {object|null} input.execution last "result" message of the execution file
   * @param {string[]} [input.forbidden] paths changed outside the agent's allowed zone
   * @param {string}   [input.pushError] error of the commit / push step
   * @param {object}   [input.files]     content of run files: { 'spec.md': '...' }
   * @returns {Promise<{ outcome: string, next?: string }>}
   */
  async function finalize(input) {
    const { agent, issueNumber, tour } = input;
    const stateLabel = `ai:${agent}`;
    const issue = await api.getIssue(issueNumber);
    if (issue.state !== 'open' || !issue.labels.includes(stateLabel)) {
      return { outcome: 'stale' }; // stopped, closed or moved on while the agent was running
    }

    const cost = Number(input.execution?.total_cost_usd) || 0;
    const previous = recordedCost(await api.listComments(issueNumber), botLogin);
    const footer =
      `\n\n<sub>Agent **${agent}** · tour ${tour} · coût équivalent ${usd(cost)} ` +
      `(cumul de la demande : ${usd(previous + cost)})${runUrl ? ` · [run](${runUrl})` : ''}</sub>` +
      `\n\n${marker('cost', { agent, usd: cost.toFixed(4) })}`;

    async function fail(reason) {
      await applyPosition(api, flowLabels, issueNumber, LABEL.ERROR);
      await api.comment(
        issueNumber,
        `❌ **Erreur** pendant l'étape \`${stateLabel}\` : ${reason}\n\nPour relancer l'étape, retirez le label \`${LABEL.ERROR}\`.` +
          `\n\n${marker('error', { step: stateLabel })}${footer}`,
      );
      return { outcome: 'error', next: LABEL.ERROR };
    }

    if (input.forbidden?.length) {
      const list = input.forbidden.slice(0, 10).map((path) => `\`${path}\``).join(', ');
      return fail(`l'agent a modifié des fichiers hors de sa zone autorisée (${list}). Rien n'a été poussé.`);
    }
    if (input.pushError) return fail(`impossible d'enregistrer le travail sur la branche (${input.pushError}).`);

    const result = input.result;
    if (!result?.status) return fail(diagnose(input.execution));
    if (result.status === 'BLOCKED') {
      return fail(
        `l'agent est bloqué.\n\n> ${(result.comment ?? '').trim().replace(/\n/g, '\n> ')}\n\n` +
          `Répondez en commentaire si vous pouvez lever le blocage, puis retirez \`${LABEL.ERROR}\` ; sinon posez \`${LABEL.STOP}\`.`,
      );
    }

    const next = TRANSITIONS[agent]?.[result.status];
    if (!next) return fail(`statut « ${result.status} » inattendu pour l'agent ${agent}.`);

    const required = REQUIRED_FILES[`${agent}:${result.status}`] ?? [];
    const missing = required.find((name) => !input.files?.[name]?.trim());
    if (missing) {
      return fail(`l'agent annonce sa sortie mais \`${input.runDir}/${missing}\` est absent ou vide.`);
    }
    const link = (name) => (branchUrl ? `[\`${name}\`](${branchUrl}/${input.runDir}/${name})` : `\`${name}\``);
    const requiredFile = required[0];
    const fileContent = requiredFile ? input.files[requiredFile].trim() : null;
    const fileLink = requiredFile ? link(requiredFile) : '';

    // The code agent is done only when the workflow's own fast check passes.
    if (agent === 'code') {
      const checks = input.checks ?? {};
      if (checks.status !== 'pass' && checks.status !== 'fail') {
        return fail("le check rapide n'a pas pu être lancé après l'agent code (voir le run).");
      }
      if (checks.status === 'fail') {
        await api.comment(
          issueNumber,
          `🔁 **Check rapide en échec** après l'agent code : il est relancé avec le rapport (${link('checks.md')}).\n\n${checksTable(checks.report)}${footer}`,
        );
        await api.removeLabel(issueNumber, LABEL.CODE); // re-adding the same label wakes the router
        await api.addLabel(issueNumber, LABEL.CODE);
        return { outcome: 'retry', next: LABEL.CODE };
      }
    }

    let body;
    if (agent === 'specs' && result.status === 'QUESTIONS') {
      body = `## Questions de l'agent specs (tour ${tour})\n\n${result.comment.trim()}\n\n${marker('questions', { round: tour })}`;
    } else if (agent === 'specs') {
      body = `## Spec finale\n\n${result.comment.trim()}\n\n---\n\n${withoutTitle(fileContent)}\n\n<sub>Source : ${fileLink}</sub>`;
    } else if (agent === 'plan') {
      body = `## Plan de développement (tour ${tour})\n\n${result.comment.trim()}\n\nPlan détaillé pour les agents : ${fileLink}. La suite s'enchaîne automatiquement : tests, code, review.`;
    } else if (agent === 'tests') {
      body = `## Tests écrits (tour ${tour})\n\n${result.comment.trim()}\n\nDétail : ${fileLink}. Ces tests échouent tant que le code n'existe pas : c'est attendu.`;
    } else if (agent === 'code') {
      const pull = await ensurePull(issue, input);
      body =
        `## Code prêt pour la review (tour ${tour})\n\n${result.comment.trim()}\n\n` +
        `Check rapide ✅ (${link('checks.md')}) · pull request : #${pull.number}`;
    } else if (agent === 'review') {
      const pull = await api.findPull(input.branch, config.AI_FLOW_INTEGRATION_BRANCH ?? 'integration');
      const posted = pull ? await postFindings(pull.number, result) : 0;
      const where = posted ? ` · ${posted} remarque(s) sur la pull request #${pull.number}` : '';
      body =
        result.status === 'APPROVED'
          ? `## Review : approuvé ✅ (tour ${tour})\n\n${result.comment.trim()}\n\nRapport pour le PM rédigé (${link('rapport-pm.md')}) : il sera publié avec la version de test. Review : ${link('review.md')}${where}.`
          : `## Review : retour au code 🔁 (tour ${tour})\n\n${result.comment.trim()}\n\nDétail : ${link('review.md')}${where}.`;
    } else {
      body = result.comment.trim();
    }

    await api.comment(issueNumber, `${body}${footer}`);
    await applyPosition(api, flowLabels, issueNumber, next);
    return { outcome: 'ok', next };
  }

  /** Draft pull request of the flow branch towards integration, created on first need. */
  async function ensurePull(issue, input) {
    const base = config.AI_FLOW_INTEGRATION_BRANCH ?? 'integration';
    const existing = await api.findPull(input.branch, base);
    if (existing) return existing;
    const file = (name) => (branchUrl ? `[${name}](${branchUrl}/${input.runDir}/${name})` : name);
    return api.createPull({
      branch: input.branch,
      base,
      title: `[#${issue.number}] ${requestTitle(issue.title)}`,
      body:
        `Closes #${issue.number}\n\n` +
        `Pull request ouverte par le flux IA. Elle reste en brouillon jusqu'à la version de test, ` +
        `puis est fusionnée dans \`${base}\` quand le PM pose \`${LABEL.VALIDATED}\`.\n\n` +
        `- Spec : ${file('spec.md')}\n- Plan : ${file('plan.md')}\n- Tests : ${file('tests.md')}\n- Notes du code : ${file('code-notes.md')}`,
      draft: true,
    });
  }

  /** Review findings as line comments of a pull request review; returns how many were posted. */
  async function postFindings(pullNumber, result) {
    const findings = (result.findings ?? []).filter((f) => f.path && Number.isInteger(f.line) && f.line > 0 && f.body);
    if (!findings.length) return 0;
    const verdict = result.status === 'APPROVED' ? 'APPROUVÉ' : 'RETOUR CODE';
    await api.createReview(pullNumber, {
      body: `Review du flux IA : **${verdict}**.`,
      comments: findings.map((f) => ({
        path: f.path,
        line: f.line,
        body: `**${SEVERITY_LABEL[f.severity] ?? f.severity}** — ${f.body}`,
      })),
    });
    return findings.length;
  }

  return { finalize };
}
