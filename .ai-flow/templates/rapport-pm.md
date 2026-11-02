<!--
Gabarit du rapport PM, écrit par l'agent review quand il approuve, publié par l'agent deploy
avec les liens de téléchargement de la version de test. En français, pour un chef de projet
non développeur : pas de jargon, pas de nom de fichier ni de fonction. Une page au maximum.
Garder les titres tels quels ; remplacer les <…>.
-->

# Rapport — <titre court de la demande>

## En résumé

<2 ou 3 phrases : ce que l'utilisateur peut faire maintenant, qui ne l'était pas avant.>

## Ce qui change dans l'application

- <Changement visible 1 : où, quoi, à quoi ça ressemble.>
- <Changement visible 2.>
- <Réglage ajouté, valeur par défaut, si c'est le cas.>

## Comment tester

Sur la version de test « Daftr Test », installée à côté de votre Daftr habituel :

1. <Action précise : ouvrir…, cliquer sur…, taper…>
   → <Ce que vous devez voir.>
2. <…>
   → <…>
3. <Un cas limite à essayer : note vide, texte très long, changement de langue…>
   → <…>

## Critères d'acceptation

| Critère | Vérifié par | État |
| --- | --- | --- |
| CA1 — <reformulé simplement> | Test automatique | ✅ |
| CA2 — <…> | À vérifier par vous (étape 2 ci-dessus) | ⏳ |

## Points d'attention

- <Risque ou limite connue, choix fait sans vous le demander (hypothèse de la spec), ou « Aucun ».>

## Hors de cette version

- <Ce qui a été volontairement laissé de côté (hors périmètre de la spec), ou « Rien ».>
