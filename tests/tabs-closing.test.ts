import { describe, it, expect } from 'vitest';
import { Tab } from '../src/app/core/models/note.model';
import { closeTabsWhere, orderByRecency, touchRecent } from '../src/app/core/utils/tabs-closing.util';

function tab(noteId: number, isPinned = false): Tab {
  return { noteId, title: `n${noteId}`, isDirty: false, isPinned };
}

const TABS = [tab(1, true), tab(2), tab(3), tab(4)];

describe('closeTabsWhere', () => {
  it('closes the others and keeps the target and the pinned tabs', () => {
    const result = closeTabsWhere(TABS, 4, t => t.noteId !== 3, 3);

    expect(result.tabs.map(t => t.noteId)).toEqual([1, 3]);
    // The active tab (4) was closed: the tab the user acted on takes over
    expect(result.activeIndex).toBe(1);
  });

  it('closes all tabs but the pinned ones, and goes home when none is left', () => {
    expect(closeTabsWhere(TABS, 2, () => true).tabs.map(t => t.noteId)).toEqual([1]);
    expect(closeTabsWhere([tab(2), tab(3)], 2, () => true)).toEqual({ tabs: [], activeIndex: -1 });
  });

  it('follows the active tab when it survives', () => {
    const result = closeTabsWhere(TABS, 3, t => t.noteId === 2);

    expect(result.tabs.map(t => t.noteId)).toEqual([1, 3, 4]);
    expect(result.activeIndex).toBe(1);
  });

  it('keeps home active (-1) when no tab was active and nothing takes over', () => {
    expect(closeTabsWhere(TABS, null, t => t.noteId === 2).activeIndex).toBe(-1);
  });
});

describe('touchRecent / orderByRecency', () => {
  it('moves the shown document to the front of the recent ones', () => {
    expect(touchRecent([1, 2, 3], 3)).toEqual([3, 1, 2]);
    expect(touchRecent([], 5)).toEqual([5]);
  });

  it('orders the open tabs by recency, the never shown ones last in their order', () => {
    const tabs = [{ noteId: 1 }, { noteId: 2 }, { noteId: 3 }, { noteId: 4 }];
    expect(orderByRecency(tabs, [3, 9, 1]).map(tab => tab.noteId)).toEqual([3, 1, 2, 4]);
  });
});
