import { Injectable, computed, effect, inject, signal } from '@angular/core';
import { QUICK_NOTES_TAB_ID } from '../models/note.model';
import { QUICK_NOTE_TAG } from '../models/settings.model';
import { Backlink, buildNoteGraph, findBacklinks, NoteGraph } from '../utils/note-links.util';
import { createLinkResolver } from '../utils/note-paths.util';
import { I18nService } from '../i18n/i18n.service';
import { FoldersService } from './folders.service';
import { NotesService } from './notes.service';
import { SidebarStateService } from './sidebar-state.service';
import { TabsService } from './tabs.service';
import { CollectionsService } from './collections.service';
import { FileDocsService } from './file-docs.service';
import { baseName, parentPath } from '../utils/collection.util';
import { graphGroups } from '../utils/graph-groups.util';
import { collectSubtreeIds } from '../utils/folder-tree.util';
import { Folder } from '../models/folder.model';

/** Links between notes (`note://id`): who points at the active note, and the whole graph. */
@Injectable({ providedIn: 'root' })
export class BacklinksService {
  private notes = inject(NotesService);
  private folders = inject(FoldersService);
  private tabs = inject(TabsService);
  private sidebarState = inject(SidebarStateService);
  private i18n = inject(I18nService);

  private readonly _hideOrphans = signal(false);
  private readonly lastActiveId = signal<number | null>(null);
  private readonly collections = inject(CollectionsService);
  private readonly fileDocs = inject(FileDocsService);

  readonly hideOrphans = this._hideOrphans.asReadonly();

  /** The active note, or the last one that was while on the home page: the graph stays centred on it. */
  readonly referenceNoteId = computed<number | null>(() => {
    const id = this.tabs.activeNoteId() ?? this.lastActiveId();
    return id !== null && this.notes.notes().some(n => n.id === id) ? id : null;
  });

  constructor() {
    effect(() => {
      const activeId = this.tabs.activeNoteId();

      if (activeId !== null && activeId !== QUICK_NOTES_TAB_ID) {
        this.lastActiveId.set(activeId);
      }
    });
  }

  /** Quick notes live together in their own document: they are neither linked nor drawn. */
  private readonly linkableNotes = computed(() =>
    this.notes.notes().filter(n => !n.tags.includes(QUICK_NOTE_TAG))
  );

  /** Resolves `[label](Folder/Note title)` links; follows the renames and moves. */
  private readonly resolver = computed(() => createLinkResolver(this.linkableNotes(), this.folders.folders()));

  /** Notes linking to the active one; follows the active tab and the edits. */
  readonly backlinks = computed<Backlink[]>(() => {
    const activeId = this.tabs.activeNoteId();
    return activeId === null || activeId === QUICK_NOTES_TAB_ID ? [] : findBacklinks(this.linkableNotes(), activeId, this.i18n.t, this.resolver());
  });

  /** Graph of the folder selected in the sidebar with its sub-folders (everything when none is), or of the selected collection. */
  readonly graph = computed<NoteGraph>(() => {
    const root = this.collections.selectedRoot();

    if (root !== null) {
      return this.collectionGraph(root);
    }

    const folderId = this.sidebarState.selectedFolderId();
    const graph = buildNoteGraph(this.linkableNotes(), this.i18n.t, {
      folderIds: folderId === null ? null : new Set(collectSubtreeIds(this.folders.folders(), folderId)),
      focusId: this.referenceNoteId(),
      hideOrphans: this._hideOrphans(),
      resolve: this.resolver(),
    });
    const folderOfNote = new Map(this.linkableNotes().map(note => [note.id, note.folderId]));
    const paths = folderPaths(this.folders.folders());

    return withGroups(graph, id => paths.get(folderOfNote.get(id) ?? -1) ?? '');
  });

  /** The files of the collection take their session ids (FileDocsService): the graph view opens them like notes. */
  private collectionGraph(root: string): NoteGraph {
    const graph = this.collections.selectedGraph();

    if (!graph) {
      return { nodes: [], links: [] };
    }

    const idOf = (path: string): number => this.fileDocs.idFor({ root, path });
    const shownNodes = graph.nodes.filter(node => !this._hideOrphans() || node.degree > 0);
    const groups = graphGroups(shownNodes.map(node => parentPath(node.path)));
    const nodes = shownNodes.map((node, index) => ({
      id: idOf(node.path), title: baseName(node.path), degree: node.degree, missing: node.missing, group: groups[index] ?? null,
    }));
    const shown = new Set(nodes.map(node => node.id));
    const links = graph.links
      .map(link => ({ source: idOf(link.source), target: idOf(link.target) }))
      .filter(link => shown.has(link.source) && shown.has(link.target));

    return { nodes, links };
  }

  toggleHideOrphans(): void {
    this._hideOrphans.update(hidden => !hidden);
  }
}

/** Path (`Parent/Child`) of every folder, by id; a cycle in the data cuts the walk short. */
function folderPaths(folders: readonly Folder[]): Map<number, string> {
  const byId = new Map(folders.map(folder => [folder.id, folder]));
  const pathOf = (folder: Folder): string => {
    const names: string[] = [];
    const seen = new Set<number>();
    for (let current: Folder | undefined = folder; current && !seen.has(current.id); current = byId.get(current.parentId ?? -1)) {
      seen.add(current.id);
      names.unshift(current.name);
    }
    return names.join('/');
  };
  return new Map(folders.map(folder => [folder.id, pathOf(folder)]));
}

/** The graph with the colour group of each node, from the folder holding it. */
function withGroups(graph: NoteGraph, folderOf: (id: number) => string): NoteGraph {
  const groups = graphGroups(graph.nodes.map(node => folderOf(node.id)));
  return { ...graph, nodes: graph.nodes.map((node, index) => ({ ...node, group: groups[index] ?? null })) };
}
