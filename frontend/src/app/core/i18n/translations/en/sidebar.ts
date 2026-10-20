export const EN_SIDEBAR = {
  // Toolbar
  'sidebar.toolbar.newNote': 'New note ({shortcut})',
  'sidebar.toolbar.newFolder': 'New folder',
  'sidebar.toolbar.search': 'Search ({shortcut})',
  'sidebar.toolbar.collapseAll': 'Collapse all folders',

  // Tree
  'sidebar.tree.allNotes': 'All notes',
  'sidebar.tree.folderNoteCount': 'Notes in this folder and its sub-folders',

  // System folders (their stored name stays the identifier, only the display is translated)
  'sidebar.folder.inbox': 'Inbox',
  'sidebar.folder.gettingStarted': 'Getting started',
  'sidebar.folder.newName': 'New folder',

  // Counts
  'sidebar.count.notes.one': '{count} note',
  'sidebar.count.notes.other': '{count} notes',
  'sidebar.count.selectedNotes.one': '{count} selected note',
  'sidebar.count.selectedNotes.other': '{count} selected notes',
  'sidebar.count.folders.one': '{count} folder',
  'sidebar.count.folders.other': '{count} folders',
  'sidebar.count.emptyFolders.one': '{count} empty folder',
  'sidebar.count.emptyFolders.other': '{count} empty folders',
  'sidebar.count.subfolders.one': '{count} sub-folder',
  'sidebar.count.subfolders.other': '{count} sub-folders',

  // Footer
  'sidebar.footer.updateAvailable': 'Version {version} is available (current: {current})',
  'sidebar.footer.settings': 'Settings ({shortcut})',

  // Context menus
  'sidebar.menu.newSubfolder': 'New Subfolder',
  'sidebar.menu.moveTo': 'Move To…',
  'sidebar.menu.color': 'Color…',
  'sidebar.menu.emptyFolder': 'Empty Folder…',
  'sidebar.menu.deleteFolder': 'Delete Folder',
  'sidebar.menu.deleteNote': 'Delete Note',
  'sidebar.menu.moveItems': 'Move {count} Items To…',
  'sidebar.menu.deleteItems': 'Delete {count} Items',

  // Move to… dialog
  'sidebar.moveTo.title': 'Move to',
  'sidebar.moveTo.titleMany': 'Move {count} items to',
  'sidebar.moveTo.root': 'Root (no folder)',
  'sidebar.moveTo.filter': 'Filter folders',
  'sidebar.moveTo.empty': 'No folder matches',

  // Confirmations
  'sidebar.dialog.deepFolder.title': 'Deeply nested folder',
  'sidebar.dialog.deepFolder.message': 'This folder would be more than {max} levels deep. Deep trees get hard to navigate: 3 to {max} levels are recommended.',
  'sidebar.dialog.deepFolder.confirm': 'Create anyway',
  'sidebar.dialog.deleteFolder.title': 'Delete "{name}"?',
  'sidebar.dialog.deleteFolder.empty': 'This folder is empty.',
  'sidebar.dialog.deleteFolder.message': 'It contains {notes} (sub-folders included). Its sub-folders are deleted in both cases.',
  'sidebar.dialog.moveNotesToRoot': 'Move notes to root',
  'sidebar.dialog.deleteEverything': 'Delete everything',
  'sidebar.dialog.emptyFolder.title': 'Empty "{name}"?',
  'sidebar.dialog.emptyFolder.message': '{notes} and {subfolders} will be deleted. The folder itself is kept.',
  'sidebar.dialog.emptyFolder.confirm': 'Empty',
  'sidebar.dialog.bulkDelete.title': 'Delete {count} items?',
  'sidebar.dialog.bulkDelete.messageEmpty': '{notes} and {folders}.',
  'sidebar.dialog.bulkDelete.message': '{selected} and {folders} holding {inFolders}. The selected notes and the folders are deleted in both cases.',
  'sidebar.dialog.bulkDelete.moveToRoot': 'Move folder notes to root',

  // Collections (folders and files of the disk)
  'sidebar.collections.title': 'Collections',
  'sidebar.collections.openFolder': 'Open Folder…',
  'sidebar.collections.openFile': 'Open Markdown File…',
  'sidebar.collections.notFound': 'Not found on disk',
  'sidebar.collections.notLoaded': 'Not loaded: expand it to load it',
  'sidebar.collections.fileName': 'File name',
  'sidebar.collections.empty': 'Open a folder or a markdown file of your disk: it is listed here, apart from your notes.',
  'sidebar.collections.trash.title': 'Move {name} to the Recycle Bin?',
  'sidebar.collections.trash.message': 'It is deleted from the disk; it can still be restored from the Windows Recycle Bin.',
  'sidebar.collections.trash.confirm': 'Move to Recycle Bin',
  'sidebar.collections.trash.title.mac': 'Move {name} to the Trash?',
  'sidebar.collections.trash.message.mac': 'It is deleted from the disk; it can still be restored from the Trash.',
  'sidebar.collections.trash.confirm.mac': 'Move to Trash',
  'sidebar.collections.menu.locate': 'Locate on Disk…',
  'sidebar.collections.menu.newFile': 'New File',
  'sidebar.collections.menu.renameCollection': 'Rename Collection',
  'sidebar.collections.menu.reveal': 'Reveal in File Explorer',
  'sidebar.collections.menu.reveal.mac': 'Reveal in Finder',
  'sidebar.collections.menu.refresh': 'Refresh',
  'sidebar.collections.menu.remove': 'Remove from Collections',
  'sidebar.collections.error.unknownCollection': 'Unknown collection',
  'sidebar.collections.error.outsideCollection': 'Path outside of the collection',
  'sidebar.collections.error.invalidName': 'Invalid name',
  'sidebar.collections.error.markdownOnly': 'Only markdown files can be written',
  'sidebar.collections.error.createInFolderOnly': 'Files can only be created in a folder collection',
  'sidebar.collections.error.renameFromFolder': 'Rename the file from its folder collection',
  'sidebar.collections.error.moveIntoItself': 'Cannot move a folder into itself',
  'sidebar.collections.error.nameTaken': 'A file or folder with this name already exists',
} as const;
