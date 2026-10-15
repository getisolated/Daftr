import { syntaxTree } from '@codemirror/language';
import { EditorState, Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';

const NOTE_URL = /note:\/\/\d+/g;
const CTRL_HELD_CLASS = 'cm-ctrl-held';

/** Target of the link under `pos` (`[label](target)`, a bare URL or a `note://id`); `null` when there is no link. */
export function linkTargetAt(state: EditorState, pos: number): string | null {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1); node; node = node.parent) {
    if (node.name === 'URL') {
      return state.sliceDoc(node.from, node.to);
    }

    if (node.name === 'Link') {
      const url = node.getChild('URL');
      return url ? state.sliceDoc(url.from, url.to) : null;
    }
  }

  // A `note://id` written as plain text is no markdown node
  const line = state.doc.lineAt(pos);

  for (const match of line.text.matchAll(NOTE_URL)) {
    const from = line.from + match.index;

    if (pos >= from && pos <= from + match[0].length) {
      return match[0];
    }
  }

  return null;
}

/** Ctrl+click (Cmd+click on macOS) on a link hands its target to `open`; while Ctrl is held the links look clickable. */
export function linkNavigation(open: (target: string) => void): Extension {
  const setHeld = (view: EditorView, held: boolean): void => {
    view.dom.classList.toggle(CTRL_HELD_CLASS, held);
  };

  return [
    EditorView.domEventHandlers({
      mousedown: (event, view) => {
        if (event.button !== 0 || !(event.ctrlKey || event.metaKey)) {
          return false;
        }

        const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
        const target = pos === null ? null : linkTargetAt(view.state, pos);

        if (target === null) {
          return false;
        }

        // No selection change: the click follows the link
        event.preventDefault();
        open(target);
        return true;
      },
      keydown: (event, view) => {
        setHeld(view, event.ctrlKey || event.metaKey);
        return false;
      },
      keyup: (event, view) => {
        setHeld(view, event.ctrlKey || event.metaKey);
        return false;
      },
      blur: (_event, view) => {
        setHeld(view, false);
        return false;
      },
    }),
    EditorView.baseTheme({
      [`&.${CTRL_HELD_CLASS} .cm-md-link`]: { cursor: 'pointer', textDecoration: 'underline' },
    }),
  ];
}
