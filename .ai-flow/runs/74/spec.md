# Spec — Compteur de mots et temps de lecture

## Objectif
Quand le réglage est activé, l'utilisateur voit en permanence, sous la note ouverte, le nombre de mots et le temps de lecture estimé, mis à jour pendant la frappe.

## Contexte
- L'éditeur (`frontend/src/app/features/editor/editor.component.ts`) affiche le fil d'Ariane, le texte (CodeMirror) et, à droite, le plan des titres. Il n'existe aucun compteur de mots ni barre d'état.
- Les réglages à case à cocher existent déjà, par exemple « afficher le plan » (`showOutline`) : défini côté Rust (`src-tauri/crates/domain/src/settings*`, `presentation/src/dto/settings.rs`), côté front (`core/models/settings.model.ts`, `features/settings/settings-definitions.ts`) et traduit dans `core/i18n/translations/{en,fr}/settings.ts`.
- Aujourd'hui, l'utilisateur copie son texte dans un autre outil pour compter les mots, ce qui est lent et sort de la note.

## Périmètre
- Inclus : un indicateur discret « N mots · X min de lecture » lié à la note ouverte ; un réglage on/off dans les paramètres, désactivé par défaut ; libellés en français et en anglais ; mise à jour en direct.
- Hors périmètre : comptage de caractères, comptage de la sélection seule, objectif de longueur, statistiques sur plusieurs notes, réglage de la vitesse de lecture, affichage dans l'export.

## Critères d'acceptation
- CA1 — Étant donné une installation neuve ou un réglage jamais modifié, quand j'ouvre une note, alors aucun compteur n'est affiché.
- CA2 — Étant donné le réglage « Compteur de mots » dans les paramètres, quand je l'active, alors le compteur apparaît immédiatement sur la note ouverte, sans redémarrer ; quand je le désactive, il disparaît. Le choix est conservé au redémarrage.
- CA3 — Étant donné le compteur activé et une note ouverte, alors il affiche le nombre de mots et le temps de lecture, visibles en permanence (ils ne disparaissent pas en faisant défiler la note).
- CA4 — Étant donné une note ouverte, quand je tape, supprime ou colle du texte, alors le nombre de mots et le temps de lecture se mettent à jour pendant la frappe, sans enregistrement ni changement de note.
- CA5 — Étant donné une note vide (ou ne contenant que des espaces et des retours à la ligne), alors le compteur affiche « 0 mot » et un temps de lecture de moins d'une minute.
- CA6 — Étant donné un texte, alors un mot est une suite de caractères séparée par des espaces, tabulations ou retours à la ligne ; plusieurs espaces d'affilée comptent pour une seule séparation. Exemple : « Bonjour   le monde » = 3 mots.
- CA7 — Étant donné le texte Markdown de la note, alors les symboles de mise en forme isolés (par ex. `#`, `-`, `>`, `---`) ne comptent pas comme des mots ; le texte des titres, listes, liens et du code est compté.
- CA8 — Étant donné N mots, alors le temps de lecture vaut N ÷ 200 minutes arrondi à la minute supérieure, avec un minimum de 1 minute pour une note non vide (ex. : 1 mot → 1 min ; 200 mots → 1 min ; 201 mots → 2 min ; 1000 mots → 5 min). Une note vide affiche « moins d'une minute » (ou « 0 min »).
- CA9 — Étant donné la langue de l'interface, alors les libellés sont traduits (fr : « 1 mot », « 2 mots », « 3 min de lecture » ; en : « 1 word », « 2 words », « 3 min read »), avec le singulier pour 0 et 1 en français et pour 1 en anglais.
- CA10 — Étant donné des caractères spéciaux (accents, emoji, apostrophes, tirets), alors l'application ne plante pas et compte « l'été » ou « it's » comme un seul mot.
- CA11 — Étant donné une note très longue (plusieurs dizaines de milliers de mots), quand je tape, alors la frappe reste fluide.
- CA12 — Étant donné que j'ouvre une autre note ou un autre onglet, alors le compteur reflète immédiatement la note affichée ; s'il n'y a aucune note ouverte, aucun compteur n'est affiché.
- CA13 — Étant donné le mode zen, alors le compteur reste affiché s'il est activé.
- CA14 — Étant donné les trois modes d'affichage (source, partagé, aperçu), alors le compteur reste visible et compte le texte Markdown de la note.

## Réponses du PM
Aucun tour de questions : la demande de l'issue (critères, réglage désactivé par défaut) a suffi.

## Hypothèses
- Emplacement : bande fine en bas de la zone d'édition, alignée à droite, en texte discret ; c'est le plus simple et ne gêne pas la lecture.
- Seuls le texte entier de la note compte (pas la sélection) : c'est ce qui est demandé.
- Les notes rapides (liste de courtes notes) : le compteur s'applique à la note éditée quand une seule note est affichée ; il n'est pas affiché sur la vue liste.
- Le réglage s'appelle « Afficher le compteur de mots et le temps de lecture » et se range avec les réglages d'affichage de l'éditeur, à côté du plan.
- Les fichiers de collection (dossiers externes) sont comptés comme les notes.
- Un mot se définit par les séparateurs d'espaces, pour toutes les langues ; les langues sans espaces (chinois, japonais) ne sont pas traitées spécialement.
- Le temps de lecture est la seule valeur arrondie à la minute : pas de secondes.

## Contraintes
- Windows et macOS, comportement identique.
- Interface en anglais et en français (clés i18n dans `en/` et `fr/`).
- Compatibilité : les réglages existants sans cette clé doivent se charger avec le compteur désactivé.
- Performance : le calcul ne doit pas ralentir la frappe sur les notes longues.
- Accessibilité : texte lisible dans les thèmes clair et sombre ; contraste suffisant ; lecteur d'écran : l'indicateur n'annonce pas chaque frappe.
