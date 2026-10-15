import { Tab } from '../models/note.model';

export interface TabsAfterClosing {
  tabs: Tab[];
  activeIndex: number;
}

/**
 * Closes every tab for which `shouldClose` is true, except the pinned ones (a pinned tab can never be closed).
 * The active tab is followed when it survives; otherwise `fallbackNoteId` (the tab the user acted on) takes over,
 * and with no such tab either the app goes back home (-1).
 */
export function closeTabsWhere(
  tabs: Tab[],
  activeNoteId: number | null,
  shouldClose: (tab: Tab) => boolean,
  fallbackNoteId: number | null = null,
): TabsAfterClosing {
  const remaining = tabs.filter(tab => tab.isPinned || !shouldClose(tab));
  const activeSurvives = activeNoteId !== null && remaining.some(tab => tab.noteId === activeNoteId);
  const nextActiveId = activeSurvives ? activeNoteId : fallbackNoteId;

  return {
    tabs: remaining,
    activeIndex: remaining.findIndex(tab => tab.noteId === nextActiveId),
  };
}
