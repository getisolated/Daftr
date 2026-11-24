# Instructions pour Claude Code

## Vérifications : rapides pendant l'écriture, complètes une seule fois à la fin

Ne lance pas toute la batterie de vérifications après chaque modification ni avant chaque commit : c'est long et sans gain de qualité. Le code poussé est vérifié exactement autant, à condition que la vérification complète finale reste obligatoire.

**Pendant l'écriture : uniquement ce que tu viens de modifier**

- Rust : `cargo check -p <crate>`, puis `cargo test -p <crate> <filtre>` pour le test concerné (depuis `src-tauri`).
- Front : `npx vitest run <fichier ou filtre>` et un lint ciblé sur les fichiers modifiés (depuis `frontend/`).
- Pas de `ng build`, ni de `cargo` pour du travail purement front, ni de `npm run` de l'ensemble entre deux commits, sauf si la modification l'exige (typage global, template douteux).

**Une seule vérification complète, juste avant le push final**

- Rust : `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Front : `npm run lint`, `npm run test`, `npm run build`.
- Ne lance que la partie touchée : un travail purement front n'a pas besoin de la batterie Rust, et inversement.
- Elle doit être verte avant de rendre la main. Un défaut trouvé à ce moment se corrige dans un commit à part.
- Tests sensibles à la concurrence : 2 ou 3 passes à la fin, pas plus.

Les commits intermédiaires n'ont pas à être verts un par un : la vérification finale fait foi.

## Rappels

- Un seul commit sur une ligne, en français, au format Conventional Commits, sans `Co-Authored-By`.
- Une branche et une PR par fonctionnalité, pas par lot.
- Ne jamais lancer `npm install` ni `npm ci` dans le dépôt.
