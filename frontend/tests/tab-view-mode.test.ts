import { describe, it, expect } from 'vitest';
import { isEditorViewMode, viewModeOnOpen } from '../src/app/core/utils/tab-view-mode.util';

describe('viewModeOnOpen', () => {
  it('restores the own mode of a tab already shown', () => {
    expect(viewModeOnOpen('split', 'preview', false)).toBe('split');
  });

  it('gives a newly opened note the mode of the last note shown', () => {
    expect(viewModeOnOpen(undefined, 'preview', false)).toBe('preview');
    expect(viewModeOnOpen(undefined, 'source', true)).toBe('source');
  });

  it('follows the settings when no note was shown yet', () => {
    expect(viewModeOnOpen(undefined, null, true)).toBe('preview');
    expect(viewModeOnOpen(undefined, null, false)).toBe('source');
  });
});

describe('isEditorViewMode', () => {
  it('accepts the three view modes only', () => {
    expect(['source', 'preview', 'split'].every(isEditorViewMode)).toBe(true);
    expect(isEditorViewMode('wysiwyg')).toBe(false);
    expect(isEditorViewMode(undefined)).toBe(false);
  });
});
