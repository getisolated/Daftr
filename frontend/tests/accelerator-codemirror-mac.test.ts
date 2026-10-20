import { describe, it, expect, vi, beforeAll } from 'vitest';
import type { EditorView, KeyBinding } from '@codemirror/view';
import { acceleratorToCodeMirrorKey } from '../src/app/core/utils/accelerator.util';

// CodeMirror reads the platform when its module loads: pretend to be WKWebView on macOS before importing it
vi.stubGlobal('navigator', {
  platform: 'MacIntel',
  userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)',
  vendor: 'Apple Computer, Inc.',
  maxTouchPoints: 0,
});

type KeyEventInit = { key: string; keyCode: number; ctrlKey?: boolean; metaKey?: boolean; altKey?: boolean; shiftKey?: boolean };

let fires: (accelerator: string, init: KeyEventInit) => boolean;

beforeAll(async () => {
  const { EditorState } = await import('@codemirror/state');
  const { keymap, runScopeHandlers } = await import('@codemirror/view');
  fires = (accelerator, init) => {
    let ran = false;
    const binding: KeyBinding = { key: acceleratorToCodeMirrorKey(accelerator), run: () => { ran = true; return true; } };
    const state = EditorState.create({ extensions: keymap.of([binding]) });
    // runScopeHandlers only reads `view.state` and hands the view to the command
    const view = { state } as unknown as EditorView;
    const event = { ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...init } as KeyboardEvent;
    runScopeHandlers(view, event, 'editor');
    return ran;
  };
});

describe('acceleratorToCodeMirrorKey on macOS (CodeMirror keymap)', () => {
  it('binds CommandOrControl to ⌘, not to ⌃', () => {
    expect(fires('CommandOrControl+B', { key: 'b', keyCode: 66, metaKey: true })).toBe(true);
    expect(fires('CommandOrControl+B', { key: 'b', keyCode: 66, ctrlKey: true })).toBe(false);
  });

  it('binds Control to ⌃, not to ⌘', () => {
    expect(fires('Control+Tab', { key: 'Tab', keyCode: 9, ctrlKey: true })).toBe(true);
    expect(fires('Control+Tab', { key: 'Tab', keyCode: 9, metaKey: true })).toBe(false);
  });

  it('matches every modifier combination whatever the order in the accelerator', () => {
    expect(fires('CommandOrControl+Shift+K', { key: 'K', keyCode: 75, metaKey: true, shiftKey: true })).toBe(true);
    expect(fires('CommandOrControl+Control+Alt+Shift+K', { key: 'K', keyCode: 75, metaKey: true, ctrlKey: true, altKey: true, shiftKey: true })).toBe(true);
    // ⌥ changes the character typed (⌥⌘S gives "ß"): CodeMirror falls back on the physical key
    expect(fires('CommandOrControl+Alt+S', { key: 'ß', keyCode: 83, metaKey: true, altKey: true })).toBe(true);
    expect(fires('CommandOrControl+S', { key: 's', keyCode: 83, metaKey: true, shiftKey: true })).toBe(false);
  });

  it('maps Super to ⌘ as well', () => {
    expect(fires('Super+B', { key: 'b', keyCode: 66, metaKey: true })).toBe(true);
  });
});
