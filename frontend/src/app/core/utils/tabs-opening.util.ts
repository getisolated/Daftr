import { Tab } from '../models/note.model';

export interface TabsAfterOpening {
  tabs: Tab[];
  activeIndex: number;
}

/**
 * Opens `tab` as the active one, or shows its note when a tab already holds it.
 * A temporary tab takes the place of the temporary tab already open (there is only one, like VS Code's preview tab);
 * a tab opened to be kept makes the tab of its note, if any, a regular one.
 */
export function openTab(tabs: Tab[], tab: Tab, temporary: boolean): TabsAfterOpening {
  const existing = tabs.findIndex(t => t.noteId === tab.noteId);

  if (existing >= 0) {
    return { tabs: temporary ? tabs : keepTabs(tabs, t => t.noteId === tab.noteId), activeIndex: existing };
  }

  const opened: Tab = temporary ? { ...tab, isTemporary: true } : tab;
  const replaced = temporary ? tabs.findIndex(t => t.isTemporary) : -1;

  if (replaced >= 0) {
    return { tabs: tabs.map((t, i) => (i === replaced ? opened : t)), activeIndex: replaced };
  }
  return { tabs: [...tabs, opened], activeIndex: tabs.length };
}

/** The temporary tabs matching `shouldKeep` become regular ones (edited, pinned, double-clicked…). */
export function keepTabs(tabs: Tab[], shouldKeep: (tab: Tab) => boolean): Tab[] {
  return tabs.some(t => t.isTemporary && shouldKeep(t))
    ? tabs.map(t => (t.isTemporary && shouldKeep(t) ? { ...t, isTemporary: false } : t))
    : tabs;
}
