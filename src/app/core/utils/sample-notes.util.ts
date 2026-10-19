import { Folder, GETTING_STARTED_FOLDER_NAMES, TreeNote } from '../models/folder.model';

/**
 * Sample notes created before the folders existed sit at the root of an existing database.
 * They are gathered once into the "Getting started" folder; as soon as that folder exists
 * (created by the seed or by this very migration) nothing is moved again, so a note the user
 * moves back to the root stays there.
 */
export function findSampleNotesToGather(folders: Folder[], notes: TreeNote[], sampleTitles: string[]): TreeNote[] {
  if (folders.some(f => GETTING_STARTED_FOLDER_NAMES.includes(f.name))) {
    return [];
  }

  const titles = new Set(sampleTitles);

  return notes.filter(n => n.folderId === null && titles.has(n.title));
}
