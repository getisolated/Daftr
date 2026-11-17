import { Injectable, inject } from '@angular/core';
import { I18nService } from '../i18n/i18n.service';
import { noteDisplayTitle } from '../utils/note-title.util';
import { Note, QUICK_NOTES_TAB_ID } from '../models/note.model';
import { SidebarDragItem } from '../models/folder.model';
import { buildHtmlDocument } from '../../shared/markdown/export-html';
import { ArchiveEntry, folderArchiveEntries, selectionArchiveEntries } from '../utils/export-archive.util';
import { DesktopBridgeService } from './desktop-bridge.service';
import { FoldersService } from './folders.service';
import { NotesService } from './notes.service';
import { PlatformService } from './platform.service';
import { isPdfExportAvailable } from '../utils/export-menu.util';

/** Format of the files of an archive export. */
export type ArchiveFormat = 'md' | 'pdf';

/** Copy / export of notes. Links are kept as written: in an export they may lead nowhere, that is up to the user. */
@Injectable({ providedIn: 'root' })
export class NoteExportService {
  private readonly notes = inject(NotesService);
  private readonly folders = inject(FoldersService);
  private readonly bridge = inject(DesktopBridgeService);
  private readonly i18n = inject(I18nService);

  /** PDF entries are offered only where the system can print them (see `isPdfExportAvailable`). */
  readonly pdfAvailable = isPdfExportAvailable(inject(PlatformService).platform);

  /** The quick notes document is made of other notes: it has no content of its own to export. */
  canExport(noteId: number | null): boolean {
    return noteId !== null && noteId !== QUICK_NOTES_TAB_ID && this.notes.notes().some(note => note.id === noteId);
  }

  /** Runs a NOTE_EXPORT_MENU entry on a note; false when the id is not one of them. */
  async runNoteAction(actionId: string, noteId: number): Promise<boolean> {
    switch (actionId) {
      case 'export:copy':
        await this.copyContent(noteId);
        return true;
      case 'export:md':
        await this.exportMarkdown(noteId);
        return true;
      case 'export:html':
        await this.exportHtml(noteId);
        return true;
      case 'export:pdf':
        await this.exportPdf(noteId);
        return true;
      default:
        return false;
    }
  }

  /** Puts the markdown of the note in the clipboard. */
  async copyContent(noteId: number): Promise<void> {
    const note = await this.notes.getById(noteId);

    if (note) {
      await navigator.clipboard.writeText(note.content);
    }
  }

  async exportMarkdown(noteId: number): Promise<void> {
    const note = await this.notes.getById(noteId);

    if (note) {
      await this.bridge.saveExport('md', this.titleOf(note), note.content);
    }
  }

  /** A standalone page rendered like the preview (code colored, mermaid diagrams drawn). */
  async exportHtml(noteId: number): Promise<void> {
    const note = await this.notes.getById(noteId);

    if (note) {
      await this.bridge.saveExport('html', this.titleOf(note), await this.renderPage(note, false));
    }
  }

  /** The same page as `exportHtml`, printed to A4 pages by the system web view (main process). */
  async exportPdf(noteId: number): Promise<void> {
    const note = await this.notes.getById(noteId);

    if (note) {
      await this.bridge.saveExport('pdf', this.titleOf(note), await this.renderPage(note, true));
    }
  }

  /** A ZIP of the notes of the folder and its sub-folders, as `.md` or PDF files in the same folder tree. */
  async exportFolder(folderId: number, format: ArchiveFormat = 'md'): Promise<void> {
    const folder = this.folders.getById(folderId);

    if (folder) {
      const entries = folderArchiveEntries(folderId, this.folders.folders(), this.notes.notes(), note => this.titleOf(note));
      await this.saveArchive(format, folder.name, entries);
    }
  }

  /**
   * A ZIP of a sidebar selection (top-level items): the notes at the root of the archive, the folders with their
   * subtree. Named after the first folder of the selection, or its first note.
   */
  async exportSelection(items: readonly SidebarDragItem[], format: ArchiveFormat): Promise<void> {
    const entries = selectionArchiveEntries(items, this.folders.folders(), this.notes.notes(), note => this.titleOf(note));
    const firstFolder = items.find(item => item.kind === 'folder');
    const name = (firstFolder && this.folders.getById(firstFolder.id)?.name) ?? entries[0]?.path ?? this.i18n.t('common.untitled');

    await this.saveArchive(format, name, entries);
  }

  private async saveArchive(format: ArchiveFormat, name: string, entries: ArchiveEntry<Note>[]): Promise<void> {
    if (format === 'md') {
      await this.bridge.saveExport('zip', name, entries.map(({ note, path }) => ({ path, content: note.content })));
      return;
    }

    // One page after the other: mermaid renders a diagram at a time
    const pages: { path: string; content: string }[] = [];

    for (const { note, path } of entries) {
      pages.push({ path, content: await this.renderPage(note, true) });
    }

    await this.bridge.saveExport('pdf-zip', name, pages);
  }

  private titleOf(note: Pick<Note, 'title'>): string {
    return noteDisplayTitle(note.title, this.i18n.t);
  }

  /** The note as a standalone page, rendered like the preview; with the print stylesheet for a PDF. */
  private async renderPage(note: Note, print: boolean): Promise<string> {
    // Loaded on demand, like the preview: markdown-it, DOMPurify and mermaid stay out of the startup bundle
    const [{ createMarkdownRenderer, renderMarkdown }, { highlightCodeBlocks, renderMermaidBlocks }] = await Promise.all([
      import('../../shared/markdown/markdown-renderer'),
      import('../../shared/markdown/preview-enhancers'),
    ]);
    const container = document.createElement('div');
    container.innerHTML = renderMarkdown(createMarkdownRenderer({ linkStatus: () => 'note', t: this.i18n.t }), note.content);

    // Mermaid only draws diagrams attached to the document: the page is built off screen
    container.style.cssText = 'position: fixed; top: 0; left: -10000px; width: 860px; visibility: hidden; pointer-events: none;';
    document.body.append(container);

    try {
      await highlightCodeBlocks(container);
      await renderMermaidBlocks(container, false);
      return buildHtmlDocument(this.titleOf(note), container.innerHTML, this.i18n.language(), { print });
    } finally {
      container.remove();
    }
  }
}
