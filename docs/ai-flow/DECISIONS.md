# Flux IA — décisions de mise en œuvre

Réponses aux questions du cahier des charges (« Flux de développement par agents IA », 2 nov. 2026), et contraintes découvertes en analysant le repo. Elles complètent le cahier ; en cas de doute, ce fichier fait foi pour l'implémentation.

## Accès

| Sujet | Décision |
| --- | --- |
| Token modèles | Secret `CLAUDE_CODE_OAUTH_TOKEN` (compte Max du propriétaire). L'App officielle « claude » n'est pas utilisée. |
| GitHub App du flux | `daftr-ai-flow` (id `5579737`), créée sur le compte `getisolated`, installée sur ce seul repo. Secrets `AI_FLOW_APP_ID`, `AI_FLOW_APP_PRIVATE_KEY`. Permissions : Actions, Checks, Contents, Issues, Pull requests en écriture. |
| Logins PM | Variable `AI_FLOW_PM_LOGINS` = `getisolated`. |
| Scripts de setup | Lancés en local par le propriétaire (Git Bash sous Windows) avec `gh` : `bash scripts/ai-flow/setup.sh`. |
| GitHub Projects | **Aucun en phase 1** : `getisolated` est un compte personnel et une GitHub App ne peut pas modifier un Project v2 personnel. Suivi par les labels et une recherche d'issues sauvegardée « Action PM attendue ». `setup-project.sh` et les champs Projects sont retirés du périmètre ; l'état reste lisible par l'API (labels, `runs/`, releases) pour l'UI de la phase 2. |

## CI et version de test

| Sujet | Décision |
| --- | --- |
| CI du flux | Fichier séparé `ai-ci.yml` ; `ci.yml` et `release.yml` restent intacts. Check rapide (job `ai-ci-check`) sur ubuntu : Daftr n'y a jamais été compilé, le minimum sera corrigé côté Rust si besoin. |
| Cibles de test | Windows x64 (installeur NSIS `.exe`) et macOS Apple Silicon (`.dmg`). Pas de Linux pour le PM. |
| App de test | « Daftr Test », identifiant `com.daftr.app.test` : s'installe à côté de l'app réelle avec ses propres données ; mise à jour automatique et artefacts de l'updater désactivés. |
| Secrets de build | Aucun. |
| Rétention | Pre-releases `test-*` supprimées à `ai:livré` ou `ai:stop` ; artifacts 30 jours. |

## Agents

| Sujet | Décision |
| --- | --- |
| Langue | Code, commentaires de code, commits, documents d'agent à agent : anglais. Tout ce que lit le PM : français. |
| Pipeline local `/feature` | Conservé ; ses consignes servent de base aux prompts `.ai-flow/prompts/`. |
| Budgets | specs 15, plan 25, tests 40, code 80, review 30, docs 25, deploy 15 (`.ai-flow/flow.env`). |
| Modèles | `opus` pour plan, code, review ; `sonnet` pour specs, tests, docs, deploy. |
| Plafonds | 3 tours de questions, 3 boucles review → code, 3 retours PM, puis `ai:escalade`. |
| Dépendances | Autorisées si justifiées dans `code-notes.md` ; vérifiées par la review. |
| Agent docs | `README.md`, `docs/`, et un `CHANGELOG.md` à créer. |
| Zones interdites | Flux et CI, chaîne de release et updater, numéros de version, `LICENSE`, `.claude/` (liste exacte : `.ai-flow/CLAUDE.md` § 7). |
| Rapport PM | Gabarit du cahier tel quel. |

## Issues

| Sujet | Décision |
| --- | --- |
| Formulaire | `demande.yml` ajouté (objectif, contexte utilisateur, critères d'acceptation, maquettes/liens, priorité, OS concernés). `feature.yml` et `bug.yml` restent, hors flux. Pas de formulaire bug dans le flux. |
| Démarrage | Uniquement quand un login PM pose le label `ai:go` (aussi pour les issues ouvertes par des tiers, ignorées sinon). |

## Branches et livraison

| Sujet | Décision |
| --- | --- |
| `main` | Ruleset « ai-flow: main » : PR obligatoire, CI `ci.yml` verte (Windows et macOS), pas de push direct, de force-push ni de suppression. Bypass : admins du repo (dont `RELEASE_TOKEN`, qui doit appartenir à un admin). |
| `integration` | Ruleset « ai-flow: integration » : seuls l'App (via PR) et les admins la mettent à jour ; check `ai-ci-check` requis ; pas de force-push ni de suppression. |
| `integration` → `main` | PR ouverte à la main par le propriétaire quand il veut publier ; la release automatique existante (`release.yml`) suffit. |
| PR des lots | Titre `ci(ai-flow): lot N [skip release]`, merge en squash : aucune release publiée. |

## Test de bout en bout

Fonctionnalité de test : **compteur de mots et temps de lecture de la note ouverte**. Le PM (propriétaire) est disponible pour dérouler la procédure.

## Contraintes techniques relevées

- `claude-code-action` refuse par défaut d'être déclenchée par un bot : les runs déclenchés par les labels de l'App devront passer `allowed_bots` avec le login de l'App (`daftr-ai-flow[bot]`), et le routeur vérifiera lui-même l'auteur des labels PM.
- Sorties utiles de `claude-code-action@v1` : `conclusion`, `execution_file` (coût : `total_cost_usd` du message `result`), `session_id`.
- `cargo test` régénère les bindings TypeScript (`frontend/src/app/core/bindings/`) : les agents doivent les commiter.
- `tauri.conf.json` active `createUpdaterArtifacts` : le build de test doit le désactiver (pas de clé de signature minisign dans le flux).
