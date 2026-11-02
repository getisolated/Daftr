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
};

/** Run file that must exist for a successful (agent, status). */
const REQUIRED_FILE = {
  'specs:SPEC_FINALE': 'spec.md',
  'plan:DONE': 'plan.md',
};

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

    const requiredFile = REQUIRED_FILE[`${agent}:${result.status}`];
    const fileContent = requiredFile ? input.files?.[requiredFile]?.trim() : null;
    if (requiredFile && !fileContent) {
      return fail(`l'agent annonce sa sortie mais \`${input.runDir}/${requiredFile}\` est absent ou vide.`);
    }
    const fileLink = requiredFile && branchUrl ? `[\`${requiredFile}\`](${branchUrl}/${input.runDir}/${requiredFile})` : `\`${requiredFile}\``;

    let body;
    if (agent === 'specs' && result.status === 'QUESTIONS') {
      body = `## Questions de l'agent specs (tour ${tour})\n\n${result.comment.trim()}\n\n${marker('questions', { round: tour })}`;
    } else if (agent === 'specs') {
      body = `## Spec finale\n\n${result.comment.trim()}\n\n---\n\n${withoutTitle(fileContent)}\n\n<sub>Source : ${fileLink}</sub>`;
    } else if (agent === 'plan') {
      body = `## Plan de développement (tour ${tour})\n\n${result.comment.trim()}\n\nPlan détaillé pour les agents : ${fileLink}. La suite s'enchaîne automatiquement : tests, code, review.`;
    } else {
      body = result.comment.trim();
    }

    await api.comment(issueNumber, `${body}${footer}`);
    await applyPosition(api, flowLabels, issueNumber, next);
    return { outcome: 'ok', next };
  }

  return { finalize };
}
