# Règles communes aux agents du flux IA

Ce fichier s'applique à **tous** les agents du flux (specs, plan, tests, code, review, docs, deploy). Ton prompt d'agent (`.ai-flow/prompts/<agent>.md`) précise ta mission ; en cas de conflit, ce fichier l'emporte.

## 1. Le flux et ta place dedans

- Une demande du PM (chef de projet, non développeur) arrive par une issue GitHub. Sept agents la traitent à la suite : specs → plan → tests → code → review → docs → deploy. Chacun tourne dans un run GitHub Actions séparé, sans mémoire des autres runs.
- **La mémoire partagée est le dossier `.ai-flow/runs/<n>/`** (`<n>` = numéro de l'issue) sur la branche `ai/<n>-<slug>`. Tu y lis ce que les agents précédents ont produit (`spec.md`, `plan.md`, `tests.md`, `code-notes.md`, `review.md`, `rapport-pm.md`) et tu y écris ta sortie. Ton contexte d'entrée (issue, commentaires, réponses cochées, fichiers du run) est assemblé dans `.ai-flow-context.md`.
- Tu ne poses **jamais** de label et tu ne changes pas l'état de l'issue : le workflow s'en charge d'après tes fichiers de sortie. Tu ne fusionnes rien et tu ne touches pas à la PR au-delà de ce que ton prompt demande.
- Tu ne vois pas l'interface de l'application desktop. Tu vérifies par les tests et la compilation ; la vérification visuelle et fonctionnelle revient au PM, sur la version de test.

## 2. Langue

- **Français** pour tout ce que lit le PM : commentaires d'issue et de PR, questions, `spec.md`, `rapport-pm.md`, résumés. Phrases simples, sans jargon technique non expliqué.
- **Anglais** pour le code, les commentaires de code, les noms (variables, fonctions, tests, fichiers), les messages de commit, et les documents d'agent à agent (`plan.md`, `tests.md`, `code-notes.md`, `review.md`).
- Exception : les textes affichés dans l'application passent par l'i18n (voir § 4) et existent en anglais **et** en français.

## 3. Repo public : rien de sensible

Le repo, ses issues, ses PR et ses runs sont lisibles par tout le monde. N'écris jamais de secret, de token, de clé, d'adresse e-mail, de donnée client ou de chemin personnel dans un fichier, un commit, un commentaire ou un log. Si le contexte en contient un par erreur, ne le recopie pas et signale-le dans ton commentaire de fin.

## 4. Le projet Daftr

Application de prise de notes : front Angular 22 dans une coquille Tauri 2 (Rust), stockage SQLite. Plateformes livrées : Windows x64 et macOS Apple Silicon. Le check rapide de la CI du flux tourne sur Linux.

```
src-tauri/                  workspace Cargo (edition 2024, rust 1.90+)
  crates/platform/          base générique : logs, verrous, chemins canoniques
  crates/domain/            règles pures, std uniquement (aucune dépendance d'I/O)
  crates/app/               cas d'usage et leurs ports (traits)
  crates/adapter-sqlite/    stockage : schéma, migrations idempotentes, FTS5
  crates/adapter-fs/        settings.json, fichiers de collections, corbeille, export ZIP, watcher
  crates/adapter-mcp/       serveur MCP local
  crates/adapter-print/     export PDF natif (seul crate avec du code `unsafe`)
  crates/presentation/      DTO exportés en TypeScript (ts-rs), erreurs IPC, noms d'événements
  crates/desktop/           coquille Tauri : commandes IPC, fenêtres, tray, raccourci global
frontend/src/app/
  core/                     services, modèles, utils purs, i18n (dictionnaires en/ et fr/)
  core/bindings/            types TypeScript générés depuis Rust : ne pas éditer à la main
  features/                 composants par fonctionnalité
  shared/                   CodeMirror, menus, composants UI
frontend/tests/             tests Vitest (*.test.ts)
```

Règles d'architecture :

- Respecte les couches : `domain` ne dépend de rien d'autre que std ; `app` définit les ports, les `adapter-*` les implémentent ; `desktop` assemble. Pas de raccourci d'un adaptateur vers un autre.
- Un DTO qui traverse l'IPC vit dans `presentation` et dérive `ts-rs` ; **`cargo test` régénère `frontend/src/app/core/bindings/`** : commite les fichiers régénérés avec le changement Rust qui les a produits.
- Tout texte visible dans l'app passe par l'i18n : ajoute la clé dans `frontend/src/app/core/i18n/translations/en/` **et** `fr/`.
- Front : composants standalone, signals, `ChangeDetectionStrategy.OnPush`, comme le code existant. Imite le style du fichier que tu modifies (`.editorconfig` : 2 espaces, guillemets simples en TypeScript).

## 5. Commandes

Depuis la racine du repo, avec des chemins explicites (pas de `cd`) :

| But | Commande |
| --- | --- |
| Dépendances front | `npm ci` |
| Tests front (Vitest) | `npm test` |
| Lint front | `npm run lint` |
| Tests des scripts Node | `npm run test:scripts` |
| Format Rust | `cargo fmt --manifest-path src-tauri/Cargo.toml --all --check` |
| Lint Rust | `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings` |
| Tests Rust | `cargo test --manifest-path src-tauri/Cargo.toml --workspace` |

Le **check rapide** que doivent passer les agents tests et code : format, lint, tests Rust, tests front. Le build complet (`tauri build`, trois OS) est réservé à l'agent deploy, via la CI.

## 6. Règles de code

- Lints du workspace (`src-tauri/Cargo.toml`, `src-tauri/clippy.toml`) : `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!` interdits hors tests ; `unsafe` interdit (hors `adapter-print`) ; `missing_docs` actif, donc **tout élément public doit avoir sa doc `///`** (sinon `-D warnings` échoue).
- Interdit de désactiver un lint, d'ajouter `#[allow(...)]`, `// eslint-disable` ou `@ts-ignore` sans le justifier dans `code-notes.md` (fichier, ligne, raison). L'agent review vérifie chaque cas.
- **Tests** : ne supprime, ne désactive (`#[ignore]`, `.skip`, `.only`) et n'affaiblis jamais un test pour faire passer la suite. Seul l'agent tests écrit les tests ; l'agent code ne les modifie que pour corriger une erreur avérée du test, documentée dans `code-notes.md`. L'agent review compare les tests avant/après.
- **Dépendances** : une nouvelle dépendance Cargo ou npm est autorisée si elle est nécessaire et justifiée dans `code-notes.md` (pourquoi, alternatives écartées, licence). Préfère ce qui existe déjà dans le workspace. Ne change jamais la version d'une dépendance existante sans raison documentée.
- Pas de code mort, pas de `println!`/`console.log` de débogage oublié.

## 7. Zones interdites en écriture

Tu ne modifies, ne crées ni ne supprimes **aucun** fichier dans ces zones, même si cela semble nécessaire ; si c'est bloquant, explique-le dans ta sortie et le PM décidera :

- `.github/` (workflows, formulaires d'issue, CI) ;
- `.ai-flow/`, **sauf** `.ai-flow/runs/<n>/` de ton issue ; `scripts/ai-flow/` ;
- la chaîne de release et de mise à jour : `scripts/stamp-version.mjs`, `scripts/update-manifest.mjs`, `scripts/update-manifest.test.mjs`, `src-tauri/crates/desktop/windows/hooks.nsh`, et dans `src-tauri/crates/desktop/tauri.conf.json` / `tauri.macos.conf.json` les clés `identifier`, `productName`, `mainBinaryName`, `version`, `bundle` et `plugins.updater` ;
- les numéros de version : champ `version` de `package.json`, `frontend/package.json`, `package-lock.json` (racine), `[workspace.package] version` de `src-tauri/Cargo.toml` ;
- `LICENSE`, `.claude/`, et tout `CLAUDE.md` hors de ce fichier.

Le workflow vérifie mécaniquement le diff : un fichier interdit modifié bloque le flux en `ai:erreur`.

## 8. Git

- Tu travailles uniquement sur la branche `ai/<n>-<slug>` déjà extraite. Ne pousse jamais sur `main` ni sur `integration`, ne crée pas d'autre branche.
- Ajoute les fichiers explicitement (`git add <chemins>`) : pas de `git add -A`, `git add .`, `git commit -a`.
- Interdits : `--force`, `git reset --hard`, `git rebase`, `git stash`, réécriture d'historique, modification de la configuration git.
- Commits en **Conventional Commits, en anglais** : `feat(scope): …`, `fix: …`, `test: …`, `docs: …`, `chore(ai-flow): …` pour les fichiers de `runs/`. Un commit par unité cohérente, chacun laissant le code compilable (sauf les tests volontairement en échec de l'agent tests).
- **Tu ne pousses jamais** : après ton passage, le workflow vérifie les chemins modifiés, commite ce qui reste dans le dossier du run (et, pour les agents qui codent, le reste de ton travail), puis pousse la branche. L'historique de `runs/<n>/` est la trace d'audit.

## 9. Fin de mission

- Écris ta sortie dans `.ai-flow/runs/<n>/` au format demandé par ton prompt (les agents qui codent commitent aussi leur code, sans pousser).
- Termine par le **résultat structuré** (`.ai-flow/result-schema.json`) : `status`, `comment` (en français, pour le PM : ce que tu as fait en 2 à 5 lignes, ou le bloc de questions pour l'agent specs) et `summary` (une ligne technique en anglais). Le workflow publie `comment` dans l'issue, avec le lien du run et le coût, puis pose le label de l'étape suivante.
- Si tu ne peux pas accomplir ta mission (information manquante, blocage technique, zone interdite nécessaire), ne bricole pas : statut `BLOCKED` avec la cause précise dans `comment`. Le workflow passe l'issue en `ai:erreur` ; le PM peut répondre en commentaire puis relancer l'étape.
