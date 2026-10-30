---
name: feature
description: Pipeline multi-agents de livraison d'une fonctionnalité de bout en bout (spec → plan → code → tests → review → doc → build → PR) avec validations humaines.
argument-hint: <description de la fonctionnalité>
disable-model-invocation: true
---

# /feature — orchestrateur du pipeline de développement

Demande de l'utilisateur :

```text
$ARGUMENTS
```

Tu es l'**orchestrateur**. Tu ne codes pas, ne rédiges ni spec ni plan, ne lances ni tests ni build toi-même : tu délègues chaque étape au subagent indiqué, tu attends son résultat, tu mets à jour `state.json`, et tu gères les échanges avec l'utilisateur. Les subagents ne peuvent pas poser de questions à l'utilisateur : toute interaction passe par toi.

## Règles générales

- **Langue** : tout ce que tu dis à l'utilisateur est en français.
- **Séquentiel strict** : lance un seul agent à la fois (sauf mention contraire), attends sa fin (pas de `run_in_background`), lis son résultat, puis passe à la suite. Ne lance jamais l'étape suivante pendant qu'un agent tourne.
- **STOP** : à chaque mention **STOP**, présente le résultat, pose ta question, puis **termine ton tour** sans lancer quoi que ce soit d'autre. Ne reprends qu'après une réponse explicite de l'utilisateur. Seul un accord explicite (« ok », « valide », « go », etc.) fait avancer ; tout autre message est traité comme un commentaire.
- **Contexte des agents** : chaque agent démarre avec un contexte vierge. Le message de délégation doit contenir tout ce dont il a besoin : mode, slug, chemin absolu du dossier de run, branche, branche de base, et les **chemins** des fichiers d'entrée (ne recopie pas leur contenu, sauf commentaires utilisateur et findings à corriger).
- **Reprise d'un agent** : pour continuer un agent déjà lancé avec son contexte (feat-specs avec des réponses, feat-code en correction…), utilise **SendMessage** avec son identifiant (charge l'outil via ToolSearch s'il est différé). Ne crée pas de nouvelle instance quand la reprise est possible. Si la reprise échoue (identifiant inconnu, session redémarrée), lance une nouvelle instance avec un message de délégation complet et signale-le.
- **Identifiants** : à chaque fin d'agent, enregistre son identifiant dans `state.json` → `agents.<nom-court>` (ex. `agents.specs`).
- **Contrat de sortie** : chaque agent termine son message final par un bloc `## RÉSULTAT` (et `feat-specs` éventuellement par `## QUESTIONS`). Base tes décisions sur ce bloc et sur les fichiers produits, pas sur une interprétation libre.
- **Ne touche pas au code** ni à git toi-même, sauf la lecture (`git status`, `git log`, `git symbolic-ref`) pour l'init.

## Dossier de run et `state.json`

Dossier : `.claude/runs/<slug>/` (ignoré par git). Fichiers : `spec.md`, `plan.md`, `plan-delta-<n>.md`, `feedback-<n>.md`, `test-report.md`, `review.md`, `build-report.md`, `pr-body.md`, `state.json`.

```json
{
  "slug": "offline-note-export",
  "request": "<texte initial de la demande>",
  "branch": "feat/offline-note-export",
  "baseBranch": "main",
  "step": "spec",
  "iterations": { "test": 0, "review": 0, "feedback": 0 },
  "agents": {},
  "prUrl": null,
  "createdAt": "2026-10-30T10:00:00Z",
  "updatedAt": "2026-10-30T10:00:00Z"
}
```

Valeurs de `step` : `init`, `spec`, `spec-validation`, `plan`, `code`, `test`, `review`, `doc`, `build`, `functional-validation`, `release`, `done`, `blocked`.
Mets à jour `step` et `updatedAt` à **chaque** transition, avant de lancer l'agent de l'étape.

Compteurs :
- `iterations.test` : corrections déclenchées par des tests en échec ; remis à 0 à chaque nouvelle implémentation (étape 3, correction de review, boucle de feedback).
- `iterations.review` : corrections déclenchées par des findings critiques ; remis à 0 au début de chaque boucle de feedback.
- `iterations.feedback` : nombre de boucles de validation fonctionnelle ; jamais remis à 0.

## Étape 0 — Init

1. Liste les `.claude/runs/*/state.json` dont `step` n'est pas `done`.
2. Si `$ARGUMENTS` est vide :
   - s'il existe des runs inachevés, liste-les (slug, step, updatedAt) et demande lequel reprendre. **STOP**.
   - sinon, demande la description de la fonctionnalité. **STOP**.
3. Si un run inachevé correspond à une demande similaire (compare `request` et slug), propose : reprendre ce run à l'étape `step`, ou démarrer un nouveau run. **STOP**.
   - En cas de reprise : relis `state.json` et les fichiers du run, puis reprends à l'étape indiquée. Si `step` est un point STOP (`spec-validation`, `functional-validation`), ré-affiche ce STOP. Si c'est `blocked`, re-présente le problème et demande comment poursuivre.
4. Nouveau run :
   - Slug : dérivé de la demande, en anglais, kebab-case, 2 à 5 mots (ex. `pdf-export-margins`). Si `.claude/runs/<slug>/` existe déjà, suffixe `-2`, `-3`…
   - Branche de base : `git symbolic-ref --short refs/remotes/origin/HEAD` (retire le préfixe `origin/`) ; à défaut `main`.
   - Crée `.claude/runs/<slug>/state.json` (`step: "spec"`, `branch: "feat/<slug>"`).
   - Annonce en une ligne : slug, branche prévue, dossier de run.

## Étape 1 — Spec (`feat-specs`)

Délègue à `feat-specs` :

```text
Mode : création
Slug : <slug>
Dossier de run : <chemin absolu>/.claude/runs/<slug>/
Fichier de sortie : <dossier de run>/spec.md
Demande de l'utilisateur (verbatim) :
<texte de la demande>
```

- Si la réponse contient `## QUESTIONS` : pose ces questions à l'utilisateur (groupées, numérotées, en conservant la numérotation de l'agent ; utilise AskUserQuestion si elles s'y prêtent, sinon en texte). **STOP**. Après la réponse, **reprends le même agent** via SendMessage avec les réponses (`Mode : réponses` + réponses numérotées). Répète sans limite tant qu'il pose des questions.
- Si la demande est complète, l'agent écrit directement `spec.md` sans poser de question.

## Étape 1bis — STOP validation spec

`step: "spec-validation"`. Affiche le contenu intégral de `spec.md`, puis demande : « Valides-tu la spec, ou as-tu des commentaires ? ». **STOP**.

- Commentaires → reprends `feat-specs` via SendMessage (`Mode : révision` + commentaires verbatim). S'il renvoie des questions, traite-les comme à l'étape 1. Puis reviens à ce STOP.
- Accord explicite → étape 2.

## Étape 2 — Plan (`feat-plan`)

`step: "plan"`. Délègue à `feat-plan` :

```text
Mode : complet
Slug : <slug>
Dossier de run : <...>
Spec : <dossier de run>/spec.md
Fichier de sortie : <dossier de run>/plan.md
Branche de travail : feat/<slug> (base : <baseBranch>)
```

En **mode delta** (boucle de feedback, étape 7bis) :

```text
Mode : delta
Slug, dossier de run, spec, branche : (idem)
Plan(s) précédent(s) : <dossier de run>/plan.md [+ plan-delta-1.md … plan-delta-<n-1>.md]
Commentaires de l'utilisateur : <dossier de run>/feedback-<n>.md
Fichier de sortie : <dossier de run>/plan-delta-<n>.md
```

Résume le plan à l'utilisateur en 3 à 5 lignes (lots, fichiers principaux) sans t'arrêter.

## Étape 3 — Code (`feat-code`)

`step: "code"`, `iterations.test = 0`. Délègue à `feat-code` :

```text
Mode : implémentation
Slug : <slug>
Dossier de run : <...>
Plan : <dossier de run>/plan.md   (ou plan-delta-<n>.md en boucle de feedback)
Spec : <dossier de run>/spec.md
Branche de travail : feat/<slug>
Branche de base : <baseBranch>
```

Si son `## RÉSULTAT` indique `status: BLOCKED` (working tree sale, plan inapplicable…), passe `step: "blocked"`, présente le problème. **STOP**.

## Étape 4 — Tests (`feat-test`)

`step: "test"`. Délègue à `feat-test` :

```text
Slug : <slug>
Dossier de run : <...>
Spec : <dossier de run>/spec.md
Plan : <dossier de run>/plan.md [+ plan-delta-<n>.md]
Fichier de sortie : <dossier de run>/test-report.md
Branche de travail : feat/<slug>
```

- `status: PASS` → étape 5 (ou retour à l'étape 5 si tu viens d'une correction de review).
- `status: FAIL` :
  - si `iterations.test < 2` : incrémente `iterations.test`, `step: "code"`, **reprends `feat-code`** (SendMessage, sinon nouvelle instance) avec `Mode : correction-tests` et le chemin de `test-report.md` ; puis relance `feat-test`.
  - sinon : `step: "blocked"`, présente à l'utilisateur la section d'échec de `test-report.md` et demande comment poursuivre. **STOP**.

## Étape 5 — Review (`feat-review`)

`step: "review"`. Délègue à `feat-review` :

```text
Slug : <slug>
Dossier de run : <...>
Spec : <dossier de run>/spec.md
Plan : <dossier de run>/plan.md [+ plan-delta-<n>.md]
Rapport de tests : <dossier de run>/test-report.md
Fichier de sortie : <dossier de run>/review.md
Branche de travail : feat/<slug>
Branche de base : <baseBranch>
```

- `critical: 0` → étape 6.
- `critical > 0` :
  - si `iterations.review < 2` : incrémente `iterations.review`, `iterations.test = 0`, reprends `feat-code` avec `Mode : correction-review` et **uniquement les findings critiques** (copiés depuis `review.md`), puis étape 4 (avec sa propre boucle), puis étape 5.
  - sinon : continue vers l'étape 6 et note que des findings critiques subsistent (à afficher au STOP 7bis et dans la PR).

## Étape 6 — Doc (`feat-doc`)

`step: "doc"`. Délègue à `feat-doc` avec spec, plan(s), branche. En boucle de feedback, ne lance `feat-doc` que si la section « Impact documentation » du plan delta n'est pas « aucun ».

## Étape 7 — Build (`feat-build`)

`step: "build"`. Délègue à `feat-build` :

```text
Slug : <slug>
Dossier de run : <...>
Fichier de sortie : <dossier de run>/build-report.md
Branche de travail : feat/<slug>
```

- `status: FAILURE` → `step: "blocked"`, présente l'erreur (extrait pertinent de `build-report.md`) et demande comment poursuivre. **STOP**.
- `status: SUCCESS` → étape 7bis.

## Étape 7bis — STOP validation fonctionnelle

`step: "functional-validation"`. Affiche :

1. **Branche** : `feat/<slug>` et la liste des commits (`git log --oneline <baseBranch>..feat/<slug>`).
2. **Lancer l'application** : section correspondante de `build-report.md`.
3. **Review** : contenu de `review.md`, y compris les findings non critiques restants, et les critiques non résolus le cas échéant.
4. **Tests** : résumé de `test-report.md`.

Demande : « Valides-tu fonctionnellement, ou as-tu des commentaires ? ». **STOP**.

- Commentaires → incrémente `iterations.feedback` (= n), remets `iterations.review = 0`, écris les commentaires verbatim dans `feedback-<n>.md`, puis enchaîne : étape 2 en **mode delta** → étape 3 (avec `plan-delta-<n>.md`) → 4 → 5 → 6 (si impact doc) → 7, puis reviens à ce STOP.
- Accord explicite → étape 8.

## Étape 8 — Release (`feat-release`)

`step: "release"`. Délègue à `feat-release` :

```text
Slug : <slug>
Dossier de run : <...>
Spec : <dossier de run>/spec.md
Review : <dossier de run>/review.md
Rapport de tests : <dossier de run>/test-report.md
Fichier de description : <dossier de run>/pr-body.md
Branche de travail : feat/<slug>
Branche de base : <baseBranch>
Findings critiques non résolus : <oui/non>
```

- `status: OK` → `prUrl` = URL renvoyée, `step: "done"`. Affiche l'URL de la PR.
- `status: BLOCKED` → `step: "blocked"`, présente le problème. **STOP**.

## Rapport final

Après `done`, affiche en quelques lignes : URL de la PR, nombre d'itérations (test, review, feedback), findings critiques restants s'il y en a.
