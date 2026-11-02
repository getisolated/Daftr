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

## Routeur (lot 2)

- **Un seul label de position par issue.** Le routeur pose le nouveau label avant de retirer l'ancien : le verrou ne se libère jamais par accident. Une décision du PM acceptée devient la position le temps de son traitement (ex. `ai:validé` pendant la fusion).
- **Démarrage par `ai:go` seulement.** Une issue créée avec le formulaire reçoit un commentaire d'accueil, puis attend `ai:go`. `ai:go` après `ai:stop` relance le flux sur la même branche.
- **Verrou** : une issue est active si elle est ouverte et porte une position autre que `ai:file-d-attente`, `ai:livré` ou `ai:stop` ; les attentes du PM (`ai:question`, `ai:à-valider`, `ai:escalade`, `ai:erreur`) gardent le verrou. La file d'attente est servie dans l'ordre d'entrée.
- **Labels du bot** : seuls les états d'agent (`ai:specs` … `ai:deploy`) et `ai:livré` réveillent le routeur ; ses autres événements sont filtrés par le `if:` du job, avant le groupe de concurrence (GitHub ne garde qu'un run en attente par groupe).
- **Compteurs** déduits de l'historique des labels, sans fichier d'état : tours de specs = nombre de `ai:specs` ; tour de plan = nombre de `ai:plan` (retours PM + 1) ; boucles review → code = nombre de `ai:code` depuis le dernier `ai:plan`.
- **Marqueurs cachés** dans les commentaires du bot (`<!-- ai-flow:error step=ai:code -->`, `escalade from=review`, `pm-return`, `questions`) : retirer `ai:erreur` relance l'étape notée, `ai:validé-quand-même` reprend selon l'origine de l'escalade.
- **`ai:problème`** exige un commentaire du PM postérieur à l'entrée dans l'état courant.
- **`ai:stop`** et la fermeture d'une issue active : annulation des runs dont le titre porte `#<n>`, fermeture de la PR `ai/<n>-*`, suppression des pre-releases `test-issue-<n>-*`, puis la file d'attente avance.
- **Tick horaire** : une étape d'agent sans run depuis 20 min passe en `ai:erreur` (run perdu) ; rappel au PM après 48 h sans activité sur une attente ; la file avance si le verrou est libre.
- **Agents livrés par lots** : un état dont le workflow d'agent n'existe pas encore est signalé dans l'issue, sans erreur ni surveillance.
- **Contexte des agents** (`build-context.sh`) : seuls les commentaires du PM et du bot sont recopiés ; ceux des autres sont comptés, pas transmis (repo public).
- **Suivi PM sans Project** : la recherche « Action PM attendue » est `is:open label:"ai:question","ai:à-valider","ai:escalade","ai:erreur"`.

## Agents specs et plan (lot 3)

- **Un workflow générique** `ai-agent-run.yml` porte le corps commun ; chaque agent a un workflow mince `ai-agent-<nom>.yml` (zone d'écriture, outils autorisés) que le routeur appelle. Un agent à la fois par issue (groupe de concurrence par issue).
- **Résultat structuré** : chaque agent termine par `status` / `comment` / `summary` (`.ai-flow/result-schema.json`, option `--json-schema` de Claude Code). Statuts : specs `QUESTIONS` ou `SPEC_FINALE`, autres agents `DONE`, tous `BLOCKED`.
- **L'agent ne pose ni label ni commentaire et ne pousse pas.** Après lui, `collect.sh` vérifie les chemins modifiés (zone `run` : seulement `.ai-flow/runs/<n>/` ; zone `code` : tout sauf `.ai-flow/forbidden-paths.txt`), commite et pousse ; `finalize` publie le commentaire (avec le coût du run et le cumul de la demande), pose le label suivant, ou `ai:erreur` avec un diagnostic (token expiré, quota, budget de tours, agent bloqué, fichier interdit).
- **Questions** : publiées dans un commentaire du bot (marqueur `questions round=N`), archivées dans `runs/<n>/questions-<N>.md`. Le PM coche dans le commentaire, précise les « Autre » en commentaire, pose `ai:réponse-donnée`. Au dernier tour autorisé, l'agent specs doit produire la spec finale.
- **Spec finale** : `runs/<n>/spec.md` (français, critères `CA1…`), recopiée dans l'issue sous « Spec finale ». **Plan** : `runs/<n>/plan.md` (anglais, pour les agents), résumé en 5 lignes pour le PM ; à partir du tour 2, section « Retour PM, tour N ».
- **Contexte** : `.ai-flow-context.md` à la racine du checkout (Claude Code peut refuser de lire hors du projet), exclu de git.
- **Coût** : marqueur `cost usd=…` dans chaque commentaire d'agent ; le cumul de la demande est la somme de ces marqueurs.

## Agents tests, code, review (lot 4)

- **Linux vérifié** : `cargo fmt`, `clippy -D warnings`, `cargo test`, Vitest, `ng lint` et `ng build` passent sur ubuntu sans modifier le code (dépendances système : `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`).
- **Cargo depuis `src-tauri/`** : seule sa `.cargo/config.toml` envoie les bindings ts-rs dans `frontend/src/app/core/bindings/` ; avec `--manifest-path` depuis la racine ils atterrissent dans les crates (`/src-tauri/crates/*/bindings/` est ignoré par git par sécurité).
- **Check rapide** (`scripts/ai-flow/fast-check.sh`) : format, clippy, tests Rust, Vitest, `ng lint`, `ng build` (seul à vérifier les templates Angular), et en CI les bindings commités. Rapport Markdown en français.
- **CI du flux** `ai-ci.yml`, job `ai-ci-check` (check requis sur `integration`) : à chaque push `ai/*` et PR vers `integration`. Rouge juste après l'agent tests : attendu.
- **tests** (zone code, Sonnet) : écrit les tests de la stratégie du plan, vérifie que leurs échecs viennent seulement du code absent, `tests.md`. **code** (zone code, Opus) : implémente jusqu'au check rapide vert, `code-notes.md` pour tout écart, test corrigé ou exception de lint. **Le workflow relance le check rapide après lui** : vert → PR en brouillon `[#n] titre` vers `integration` (`Closes #n`) puis review ; rouge → l'agent code est relancé avec `checks.md` (compte dans les 3 reprises de code avant escalade). **review** (zone run, Opus, git en lecture) : verdict `APPROUVÉ` / `RETOUR CODE` dans `review.md`, comparaison des tests avant/après, remarques publiées ligne à ligne sur la PR (review GitHub en simple commentaire), `rapport-pm.md` au gabarit si approuvé.
- **Outils Bash** limités par motifs (`cargo`, `npm`, `npx`, `node`, `git add/commit/diff/log/show/status/rm/mv`, `cd`, `ls`, `mkdir`, le check rapide) : ni `git push`, ni `gh`, ni `curl`.
- **Identité des commits** : le bot de l'App (`AI_FLOW_BOT_USER_ID`), passé aussi à claude-code-action qui imposait sinon `claude[bot]`.
