import { EN_COLLECTION_SYNC } from '../en/collection-sync';

export const FR_COLLECTION_SYNC: Record<keyof typeof EN_COLLECTION_SYNC, string> = {
  // Menu et lignes de la barre latérale
  'csync.menu.share': 'Synchroniser avec…',
  'csync.menu.stop': 'Arrêter de synchroniser cette collection',
  'csync.menu.relink': 'Relier à un autre dossier…',
  'csync.pending.one': '{count} collection à configurer',
  'csync.pending.other': '{count} collections à configurer',
  'csync.pending.open': 'Configurer',
  'csync.conflict.badge': 'Copie en conflit : à résoudre',

  // Infobulles de la pastille d’une collection partagée
  'csync.status.upToDate': 'Synchronisée avec {devices}',
  'csync.status.upToDateAgo': 'Synchronisée avec {devices}, {ago}',
  'csync.status.syncing': 'Synchronisation…',
  'csync.status.syncingProgress': '{progress} fichiers',
  'csync.status.conflicts.one': '{count} fichier en conflit',
  'csync.status.conflicts.other': '{count} fichiers en conflit',
  'csync.status.waiting': 'En attente de {devices}',
  'csync.status.offered': 'Proposée à {devices}, en attente de réponse',
  'csync.status.missingFolder': 'Dossier introuvable. Reliez-la à un autre dossier.',
  'csync.status.openDevices': 'Ouvrir la synchronisation',
  'csync.files.one': '{count} fichier',
  'csync.files.other': '{count} fichiers',

  // Assistant de partage
  'csync.share.summary.one': '{count} fichier .md ({size}) à partager. Les autres fichiers du dossier ne sont pas touchés.',
  'csync.share.summary.other': '{count} fichiers .md ({size}) à partager. Les autres fichiers du dossier ne sont pas touchés.',
  'csync.share.tooLarge.one': '{count} fichier de plus de 10 Mo est ignoré.',
  'csync.share.tooLarge.other': '{count} fichiers de plus de 10 Mo sont ignorés.',
  'csync.share.truncated': 'Le dossier est très volumineux : une partie seulement est prise en compte.',
  'csync.share.git': 'Ce dossier est géré par git : les .md peuvent aussi être modifiés par git.',

  // Proposition reçue
  'csync.toast.offer': '{device} propose la collection {name} ({files}).',
  'csync.toast.offerMany.one': '{device} propose {count} collection.',
  'csync.toast.offerMany.other': '{device} propose {count} collections.',
  'csync.toast.view': 'Configurer',
  'csync.offer.destination': 'Dossier de destination',
  'csync.offer.modify': 'Modifier…',
  'csync.offer.exists.one': 'Ce dossier existe déjà ({count} .md).',
  'csync.offer.exists.other': 'Ce dossier existe déjà ({count} .md).',
  'csync.offer.mergeHint': 'Les fichiers identiques sont ignorés, les absents copiés, les différents gardés en double.',
  'csync.offer.nothingWritten': 'Rien n’est écrit sur le disque avant l’activation.',

  // Vue du profil : collections partagées
  'csync.profile.stop': 'Arrêter',
  'csync.profile.stopTitle': 'Arrêter de synchroniser « {name} » ?',
  'csync.profile.stopMessage': 'Les fichiers restent sur cet appareil et sur les autres : seul le lien est coupé.',

  // Réglages
  'csync.settings.receivedFolder.title': 'Dossier des collections reçues',
  'csync.settings.receivedFolder.description': 'Où arrivent par défaut les collections proposées par vos autres appareils : un dossier par collection. Propre à cet ordinateur, jamais synchronisé. Vide : Documents/Daftr.',
  'csync.settings.browse': 'Parcourir…',

  // Écran « Synchronisation » : collections
  'csync.menu.title': 'Synchroniser avec',
  'csync.menu.noDevice': 'Aucun appareil lié pour l’instant.',
  'csync.menu.linkDevice': 'Lier un appareil…',
  'csync.menu.manage': 'Partage et appareils…',
  'csync.offer.badge': 'À configurer',
  'csync.offer.meta': 'Reçue de {device} · {files} · {size}',
  'csync.offer.profile': 'Profil',
  'csync.offer.activate': 'Activer',
  'csync.offer.ignore': 'Ignorer',
  'csync.offer.expand': 'Configurer « {name} »',
  'csync.offer.noProfile': 'Sans profil',
  'csync.offer.noProfileHint': 'Le profil qui les porte n’est pas synchronisé ici : choisissez où va chacune.',

  // Conflits
  'csync.conflict.bannerMine': 'Modifié sur les deux appareils : cette copie garde votre version, l’original contient l’autre.',
  'csync.conflict.bannerOther': 'Modifié sur les deux appareils : cette copie garde la version de {device}, l’original contient la vôtre.',
  'csync.conflict.keepMine': 'Garder ma version',
  'csync.conflict.keepOther': 'Garder l’autre',
  'csync.conflict.compare': 'Comparer',
  'csync.conflict.failed': 'Le conflit n’a pas pu être résolu : {message}',
  'csync.compare.title': 'Comparer les versions',
  'csync.compare.original': 'Original',
  'csync.compare.copy': 'Copie en conflit',
  'csync.compare.identical': 'Les deux versions sont identiques.',
  'csync.compare.loading': 'Lecture des fichiers…',
  'csync.compare.failed': 'Les fichiers n’ont pas pu être lus.',
  'csync.compare.mine': 'votre version',
  'csync.compare.otherVersion': 'l’autre version',
  'csync.compare.theirs': 'version de {device}',
};
