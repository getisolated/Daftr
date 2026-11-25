export const EN_PROFILES = {
  // Names shown when a profile has none
  'profiles.defaultName': 'Default profile',
  'profiles.unnamed': 'Unnamed profile',

  // Sidebar item and its menu
  'profiles.switcher.label': 'Profile: {name}',
  'profiles.menu.label': 'Profiles',
  'profiles.menu.current': 'Current profile',
  'profiles.menu.protected': 'Protected by a password',
  'profiles.menu.new': 'New Profile',
  'profiles.menu.settings': 'Profile Settings…',
  'profiles.menu.sync': 'Synchronization…',

  // Settings dialog
  'profiles.settings.title': 'Profile settings',
  'profiles.settings.titleNew': 'New profile',
  'profiles.settings.name': 'Name',
  'profiles.settings.icon': 'Icon',
  'profiles.settings.chooseIcon': 'Choose an image…',
  'profiles.settings.removeIcon': 'Remove',
  'profiles.settings.delete': 'Delete the profile',
  'profiles.settings.iconError': 'This image cannot be read.',
  'profiles.settings.passwordSet': 'Set a master password',
  'profiles.settings.passwordChange': 'Change the master password',
  'profiles.settings.currentPassword': 'Current password',
  'profiles.settings.newPassword': 'New password',
  'profiles.settings.confirmPassword': 'Confirm the password',
  'profiles.settings.removePassword': 'Remove the password',
  'profiles.settings.passwordMismatch': 'The two passwords differ.',
  'profiles.settings.currentRequired': 'Enter the current password to change or remove it.',
  'profiles.settings.create': 'Create and Open',
  'profiles.settings.save': 'Save',

  // Password asked to open or delete a protected profile
  'profiles.prompt.title': 'Open {name}',
  'profiles.prompt.message': 'This profile is protected by a password.',
  'profiles.prompt.password': 'Password',
  'profiles.prompt.open': 'Open',
  'profiles.delete.title': 'Delete {name}?',
  'profiles.delete.message': 'Its notes, folders and list of collections go to the trash with its folder. The files of its collections stay on the disk.',
  'profiles.delete.protectedMessage': 'Enter its password to delete it. Its notes, folders and list of collections go to the trash with its folder.',
  'profiles.delete.activeNote': 'This profile is open: another one opens in its place (an empty default profile if none is left).',
  'profiles.delete.confirm': 'Delete',

  // Lock screen at launch
  'profiles.unlock.title': '{name} is locked',
  'profiles.unlock.message': 'Enter the master password to open this profile.',
  'profiles.unlock.submit': 'Unlock',
  'profiles.unlock.others': 'Open another profile',

  // Errors
  'profiles.error.wrongPassword': 'Wrong password.',
  'profiles.error.failed': 'The operation failed: {message}',

  // Transfer between profiles (context menus of the sidebar)
  'profiles.transfer.menu': 'Transfer to Profile',
  'profiles.transfer.failedTitle': 'Transfer failed',
  'profiles.transfer.failed': 'Nothing was lost; some items may now be in both profiles. ({message})',
} as const;
