import { EN_SIDEBAR } from '../en/sidebar';

export const FR_SIDEBAR: Record<keyof typeof EN_SIDEBAR, string> = {
  // Barre d’outils
  'sidebar.toolbar.newNote': 'Nouvelle note ({shortcut})',
  'sidebar.toolbar.newFolder': 'Nouveau dossier',
  'sidebar.toolbar.search': 'Rechercher ({shortcut})',
  'sidebar.toolbar.collapseAll': 'Réduire tous les dossiers',

  // Arborescence
  'sidebar.tree.allNotes': 'Toutes les notes',
  'sidebar.tree.folderNoteCount': 'Notes de ce dossier et de ses sous-dossiers',

  // Dossiers système
  'sidebar.folder.inbox': 'Boîte de réception',
  'sidebar.folder.gettingStarted': 'Premiers pas',
  'sidebar.folder.newName': 'Nouveau dossier',

  // Compteurs
  'sidebar.count.notes.one': '{count} note',
  'sidebar.count.notes.other': '{count} notes',
  'sidebar.count.selectedNotes.one': '{count} note sélectionnée',
  'sidebar.count.selectedNotes.other': '{count} notes sélectionnées',
  'sidebar.count.folders.one': '{count} dossier',
  'sidebar.count.folders.other': '{count} dossiers',
  'sidebar.count.emptyFolders.one': '{count} dossier vide',
  'sidebar.count.emptyFolders.other': '{count} dossiers vides',
  'sidebar.count.subfolders.one': '{count} sous-dossier',
  'sidebar.count.subfolders.other': '{count} sous-dossiers',

  // Pied
  'sidebar.footer.updateAvailable': 'La version {version} est disponible (actuelle : {current})',
  'sidebar.footer.settings': 'Paramètres ({shortcut})',

  // Menus contextuels
  'sidebar.menu.newSubfolder': 'Nouveau sous-dossier',
  'sidebar.menu.moveTo': 'Déplacer vers…',
  'sidebar.menu.color': 'Couleur…',
  'sidebar.menu.emptyFolder': 'Vider le dossier…',
  'sidebar.menu.deleteFolder': 'Supprimer le dossier',
  'sidebar.menu.deleteNote': 'Supprimer la note',
  'sidebar.menu.moveItems': 'Déplacer {count} éléments vers…',
  'sidebar.menu.deleteItems': 'Supprimer {count} éléments',

  // Dialogue « Déplacer vers… »
  'sidebar.moveTo.title': 'Déplacer vers',
  'sidebar.moveTo.titleMany': 'Déplacer {count} éléments vers',
  'sidebar.moveTo.root': 'Racine (aucun dossier)',
  'sidebar.moveTo.filter': 'Filtrer les dossiers',
  'sidebar.moveTo.empty': 'Aucun dossier ne correspond',

  // Confirmations
  'sidebar.dialog.deepFolder.title': 'Dossier trop imbriqué',
  'sidebar.dialog.deepFolder.message': 'Ce dossier serait imbriqué sur plus de {max} niveaux. Les arborescences profondes sont difficiles à parcourir : 3 à {max} niveaux sont recommandés.',
  'sidebar.dialog.deepFolder.confirm': 'Créer quand même',
  'sidebar.dialog.deleteFolder.title': 'Supprimer « {name} » ?',
  'sidebar.dialog.deleteFolder.empty': 'Ce dossier est vide.',
  'sidebar.dialog.deleteFolder.message': 'Il contient {notes} (sous-dossiers inclus). Ses sous-dossiers sont supprimés dans les deux cas.',
  'sidebar.dialog.moveNotesToRoot': 'Déplacer les notes à la racine',
  'sidebar.dialog.deleteEverything': 'Tout supprimer',
  'sidebar.dialog.emptyFolder.title': 'Vider « {name} » ?',
  'sidebar.dialog.emptyFolder.message': '{notes} et {subfolders} seront supprimés. Le dossier lui-même est conservé.',
  'sidebar.dialog.emptyFolder.confirm': 'Vider',
  'sidebar.dialog.bulkDelete.title': 'Supprimer {count} éléments ?',
  'sidebar.dialog.bulkDelete.messageEmpty': '{notes} et {folders}.',
  'sidebar.dialog.bulkDelete.message': '{selected} et {folders} contenant {inFolders}. Les notes sélectionnées et les dossiers sont supprimés dans les deux cas.',
  'sidebar.dialog.bulkDelete.moveToRoot': 'Déplacer les notes des dossiers à la racine',

  // Collections (dossiers et fichiers du disque)
  'sidebar.collections.title': 'Collections',
  'sidebar.collections.openFolder': 'Ouvrir un dossier…',
  'sidebar.collections.openFile': 'Ouvrir un fichier Markdown…',
  'sidebar.collections.notFound': 'Introuvable sur le disque',
  'sidebar.collections.notLoaded': 'Non chargé : développez-le pour le charger',
  'sidebar.collections.fileName': 'Nom du fichier',
  'sidebar.collections.empty': 'Ouvrez un dossier ou un fichier Markdown de votre disque : il est listé ici, à part de vos notes.',
  'sidebar.collections.trash.title': 'Déplacer {name} vers la Corbeille ?',
  'sidebar.collections.trash.message': 'Il est supprimé du disque ; il reste récupérable depuis la Corbeille de Windows.',
  'sidebar.collections.trash.confirm': 'Déplacer vers la Corbeille',
  'sidebar.collections.menu.locate': 'Localiser sur le disque…',
  'sidebar.collections.menu.newFile': 'Nouveau fichier',
  'sidebar.collections.menu.renameCollection': 'Renommer la collection',
  'sidebar.collections.menu.reveal': 'Afficher dans l’Explorateur de fichiers',
  'sidebar.collections.menu.refresh': 'Actualiser',
  'sidebar.collections.menu.remove': 'Retirer des collections',
  'sidebar.collections.error.unknownCollection': 'Collection inconnue',
  'sidebar.collections.error.outsideCollection': 'Chemin en dehors de la collection',
  'sidebar.collections.error.invalidName': 'Nom invalide',
  'sidebar.collections.error.markdownOnly': 'Seuls les fichiers markdown peuvent être écrits',
  'sidebar.collections.error.createInFolderOnly': 'Les fichiers ne peuvent être créés que dans une collection de dossier',
  'sidebar.collections.error.renameFromFolder': 'Renommez le fichier depuis sa collection de dossier',
  'sidebar.collections.error.moveIntoItself': 'Impossible de déplacer un dossier dans lui-même',
  'sidebar.collections.error.nameTaken': 'Un fichier ou un dossier portant ce nom existe déjà',
};
