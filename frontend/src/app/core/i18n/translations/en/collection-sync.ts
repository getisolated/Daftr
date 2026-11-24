export const EN_COLLECTION_SYNC = {
  // Sidebar menu and rows
  'csync.menu.share': 'Sync with…',
  'csync.menu.stop': 'Stop Syncing This Collection',
  'csync.menu.relink': 'Relink to Another Folder…',
  'csync.pending.one': '{count} collection to set up',
  'csync.pending.other': '{count} collections to set up',
  'csync.pending.open': 'Set up',
  'csync.conflict.badge': 'Conflict copy: to resolve',

  // Tooltips of the badge of a shared collection
  'csync.status.upToDate': 'Synced with {devices}',
  'csync.status.upToDateAgo': 'Synced with {devices}, {ago}',
  'csync.status.syncing': 'Syncing…',
  'csync.status.syncingProgress': '{progress} files',
  'csync.status.conflicts.one': '{count} file in conflict',
  'csync.status.conflicts.other': '{count} files in conflict',
  'csync.status.waiting': 'Waiting for {devices}',
  'csync.status.offered': 'Proposed to {devices}, waiting for an answer',
  'csync.status.missingFolder': 'Folder not found. Relink it to another folder.',
  'csync.status.openDevices': 'Open the synchronization',
  'csync.files.one': '{count} file',
  'csync.files.other': '{count} files',

  // Sharing assistant
  'csync.share.summary.one': '{count} .md file ({size}) to share. The other files of the folder are not touched.',
  'csync.share.summary.other': '{count} .md files ({size}) to share. The other files of the folder are not touched.',
  'csync.share.tooLarge.one': '{count} file over 10 MB is skipped.',
  'csync.share.tooLarge.other': '{count} files over 10 MB are skipped.',
  'csync.share.truncated': 'The folder is very large: only a part of it is taken into account.',
  'csync.share.git': 'This folder is managed by git: the .md files can also be changed by git.',

  // Proposal received
  'csync.toast.offer': '{device} proposes the collection {name} ({files}).',
  'csync.toast.offerMany.one': '{device} proposes {count} collection.',
  'csync.toast.offerMany.other': '{device} proposes {count} collections.',
  'csync.toast.view': 'Set up',
  'csync.offer.destination': 'Destination folder',
  'csync.offer.modify': 'Change…',
  'csync.offer.exists.one': 'This folder already exists ({count} .md file).',
  'csync.offer.exists.other': 'This folder already exists ({count} .md files).',
  'csync.offer.mergeHint': 'Identical files are kept, missing ones are copied, different ones are kept side by side.',
  'csync.offer.nothingWritten': 'Nothing is written to the disk until you activate it.',

  // Profile view: shared collections
  'csync.profile.stop': 'Stop',
  'csync.profile.stopTitle': 'Stop syncing “{name}”?',
  'csync.profile.stopMessage': 'The files stay on this device and on the others: only the link is cut.',

  // Settings
  'csync.settings.receivedFolder.title': 'Received Collections Folder',
  'csync.settings.receivedFolder.description': 'Where the collections proposed by your other devices arrive by default: one folder per collection. Specific to this computer, never synced. Empty: Documents/Daftr.',
  'csync.settings.browse': 'Browse…',

  // Synchronization screen: collections
  'csync.menu.title': 'Sync with',
  'csync.menu.noDevice': 'No linked device yet.',
  'csync.menu.linkDevice': 'Link a Device…',
  'csync.menu.manage': 'Sharing and Devices…',
  'csync.offer.badge': 'To set up',
  'csync.offer.meta': 'Received from {device} · {files} · {size}',
  'csync.offer.profile': 'Profile',
  'csync.offer.activate': 'Activate',
  'csync.offer.ignore': 'Ignore',
  'csync.offer.expand': 'Set up “{name}”',
  'csync.offer.noProfile': 'Without profile',
  'csync.offer.noProfileHint': 'The profile that holds them is not synced here: choose where each one goes.',

  // Conflicts
  'csync.conflict.bannerMine': 'Edited on both devices: this copy keeps your version, the original holds the other one.',
  'csync.conflict.bannerOther': 'Edited on both devices: this copy keeps the version of {device}, the original holds yours.',
  'csync.conflict.keepMine': 'Keep My Version',
  'csync.conflict.keepOther': 'Keep the Other',
  'csync.conflict.compare': 'Compare',
  'csync.conflict.failed': 'The conflict could not be resolved: {message}',
  'csync.compare.title': 'Compare the versions',
  'csync.compare.original': 'Original',
  'csync.compare.copy': 'Conflict copy',
  'csync.compare.identical': 'The two versions are identical.',
  'csync.compare.loading': 'Reading the files…',
  'csync.compare.failed': 'The files could not be read.',
  'csync.compare.mine': 'your version',
  'csync.compare.otherVersion': 'the other version',
  'csync.compare.theirs': 'version of {device}',
} as const;
