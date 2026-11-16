import type { Translator } from '../i18n/translate';
import type { Platform } from '../services/platform.service';
import type { ContextMenuItem } from '../../shared/context-menu/context-menu.component';

const ICON_COPY = 'M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2M16 4h2a2 2 0 0 1 2 2v4M21 14H11M15 10l-4 4 4 4';
const ICON_EXPORT = 'M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3';

/**
 * Whether the system web view can print a page to PDF: WebView2 (Windows) and WKWebView (macOS) can, the Linux
 * build has no native printing (`adapter-print`), so the PDF entries are left out there.
 */
export function isPdfExportAvailable(platform: Platform): boolean {
  return platform !== 'linux';
}

/** Context-menu entry of a note, the same in the tabs, the editor and the sidebar: its actions open in a submenu. */
export function noteExportMenu(t: Translator, pdf: boolean): ContextMenuItem {
  const children: ContextMenuItem[] = [
    { id: 'export:copy', label: t('editor.export.copyContent'), icon: ICON_COPY },
    { id: 'export:md', label: t('editor.export.markdown'), icon: ICON_EXPORT },
    { id: 'export:html', label: t('editor.export.html'), icon: ICON_EXPORT },
  ];

  if (pdf) {
    children.push({ id: 'export:pdf', label: t('editor.export.pdf'), icon: ICON_EXPORT });
  }

  return { id: 'export', label: t('editor.export.exportAs'), icon: ICON_EXPORT, children };
}

/** Context-menu entries of a folder: its notes as a ZIP of `.md` files, or of PDF files. */
export function folderExportMenu(t: Translator, pdf: boolean): ContextMenuItem[] {
  const items: ContextMenuItem[] = [{ id: 'export:zip', label: t('editor.export.zip'), icon: ICON_EXPORT }];

  if (pdf) {
    items.push({ id: 'export:pdf-zip', label: t('editor.export.pdfZip'), icon: ICON_EXPORT });
  }

  return items;
}

/** Context-menu entry of a sidebar selection (several notes and / or folders): one archive of all of them. */
export function selectionExportMenu(t: Translator, pdf: boolean): ContextMenuItem {
  const children: ContextMenuItem[] = [
    { id: 'export:zip', label: t('editor.export.selectionMarkdown'), icon: ICON_EXPORT },
  ];

  if (pdf) {
    children.push({ id: 'export:pdf-zip', label: t('editor.export.selectionPdf'), icon: ICON_EXPORT });
  }

  return { id: 'export', label: t('editor.export.export'), icon: ICON_EXPORT, children };
}
