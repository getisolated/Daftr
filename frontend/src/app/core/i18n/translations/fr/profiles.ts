import { EN_PROFILES } from '../en/profiles';

export const FR_PROFILES: Record<keyof typeof EN_PROFILES, string> = {
  // Noms affichés quand un profil n'en a pas
  'profiles.defaultName': 'Profil par défaut',
  'profiles.unnamed': 'Profil sans nom',

  // Élément de la sidebar et son menu
  'profiles.switcher.label': 'Profil : {name}',
  'profiles.menu.label': 'Profils',
  'profiles.menu.current': 'Profil actuel',
  'profiles.menu.protected': 'Protégé par un mot de passe',
  'profiles.menu.new': 'Nouveau profil',
  'profiles.menu.settings': 'Paramétrer le profil…',

  // Dialogue de paramétrage
  'profiles.settings.title': 'Paramètres du profil',
  'profiles.settings.titleNew': 'Nouveau profil',
  'profiles.settings.name': 'Nom',
  'profiles.settings.icon': 'Icône',
  'profiles.settings.chooseIcon': 'Choisir une image…',
  'profiles.settings.removeIcon': 'Retirer',
  'profiles.settings.delete': 'Supprimer le profil',
  'profiles.settings.iconError': 'Cette image ne peut pas être lue.',
  'profiles.settings.passwordSet': 'Définir un mot de passe maître',
  'profiles.settings.passwordChange': 'Changer le mot de passe maître',
  'profiles.settings.currentPassword': 'Mot de passe actuel',
  'profiles.settings.newPassword': 'Nouveau mot de passe',
  'profiles.settings.confirmPassword': 'Confirmer le mot de passe',
  'profiles.settings.removePassword': 'Retirer le mot de passe',
  'profiles.settings.passwordMismatch': 'Les deux mots de passe diffèrent.',
  'profiles.settings.currentRequired': 'Saisissez le mot de passe actuel pour le changer ou le retirer.',
  'profiles.settings.create': 'Créer et ouvrir',
  'profiles.settings.save': 'Enregistrer',

  // Mot de passe demandé pour ouvrir ou supprimer un profil protégé
  'profiles.prompt.title': 'Ouvrir {name}',
  'profiles.prompt.message': 'Ce profil est protégé par un mot de passe.',
  'profiles.prompt.password': 'Mot de passe',
  'profiles.prompt.open': 'Ouvrir',
  'profiles.delete.title': 'Supprimer {name} ?',
  'profiles.delete.message': 'Ses notes, dossiers et sa liste de collections partent à la corbeille avec son dossier. Les fichiers de ses collections restent sur le disque.',
  'profiles.delete.protectedMessage': 'Saisissez son mot de passe pour le supprimer. Ses notes, dossiers et sa liste de collections partent à la corbeille avec son dossier.',
  'profiles.delete.activeNote': 'Ce profil est ouvert : un autre profil s’ouvre à sa place (un profil par défaut vide s’il n’en reste aucun).',
  'profiles.delete.confirm': 'Supprimer',

  // Écran de verrouillage au lancement
  'profiles.unlock.title': '{name} est verrouillé',
  'profiles.unlock.message': 'Saisissez le mot de passe maître pour ouvrir ce profil.',
  'profiles.unlock.submit': 'Déverrouiller',
  'profiles.unlock.others': 'Ouvrir un autre profil',

  // Erreurs
  'profiles.error.wrongPassword': 'Mot de passe incorrect.',
  'profiles.error.failed': 'L’opération a échoué : {message}',

  // Transfert entre profils (menus contextuels de la sidebar)
  'profiles.transfer.menu': 'Transférer vers le profil',
  'profiles.transfer.failedTitle': 'Transfert échoué',
  'profiles.transfer.failed': 'Rien n’est perdu ; certains éléments peuvent se trouver dans les deux profils. ({message})',
};
