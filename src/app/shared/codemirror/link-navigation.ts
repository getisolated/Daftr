import { syntaxTree } from '@codemirror/language';
import { EditorState, Extension } from '@codemirror/state';
import { EditorView, hoverTooltip } from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';
import { translatorOf } from './translator';

const NOTE_URL = /note:\/\/\d+/g;
const CTRL_HELD_CLASS = 'cm-ctrl-held';

interface LinkHit {
  from: number;
  to: number;
  target: string;
}

/** Link under `pos` (`[label](target)`, a bare URL or a `note://id`) with the range to hover; `null` when there is none. */
export function linkAt(state: EditorState, pos: number): LinkHit | null {
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1); node; node = node.parent) {
    if (node.name === 'URL') {
      return { from: node.from, to: node.to, target: state.sliceDoc(node.from, node.to) };
    }

    if (node.name === 'Link') {
      const url = node.getChild('URL');
      return url ? { from: node.from, to: node.to, target: state.sliceDoc(url.from, url.to) } : null;
    }
  }

  // A `note://id` written as plain text is no markdown node
  const line = state.doc.lineAt(pos);

  for (const match of line.text.matchAll(NOTE_URL)) {
    const from = line.from + match.index;
    const to = from + match[0].length;

    if (pos >= from && pos <= to) {
      return { from, to, target: match[0] };
    }
  }

  return null;
}

/** Target of the link under `pos`; `null` when there is no link. */
export function linkTargetAt(state: EditorState, pos: number): string | null {
  return linkAt(state, pos)?.target ?? null;
}

/** Hover hint telling how to follow a link; a plain DOM tooltip so it matches the app theme, unlike the native `title`. */
function linkHoverTooltip(): Extension {
  const isMac = /mac/i.test(navigator.platform);

  return [
    hoverTooltip((view, pos) => {
      const hit = linkAt(view.state, pos);

      if (hit === null) {
        return null;
      }

      return {
        pos: hit.from,
        end: hit.to,
        above: true,
        create: view => {
          const dom = document.createElement('div');

          dom.className = 'cm-link-hint';
          dom.textContent = translatorOf(view.state)('editor.link.hint', { key: isMac ? 'Cmd' : 'Ctrl' });
          return { dom };
        },
      };
    }),
    EditorView.baseTheme({
      '.cm-tooltip.cm-tooltip-hover': { border: 'none', background: 'transparent' },
      '.cm-link-hint': {
        padding: '3px 8px',
        fontFamily: "'Segoe UI', system-ui, sans-serif",
        fontSize: '12px',
        color: 'var(--evn-text)',
        background: 'var(--evn-bg-chip)',
        border: '1px solid var(--evn-border-soft)',
        borderRadius: '4px',
        userSelect: 'none',
      },
    }),
  ];
}

/** Ctrl+click (Cmd+click on macOS) on a link hands its target to `open`; while Ctrl is held the links look clickable. */
export function linkNavigation(open: (target: string) => void): Extension {
  const setHeld = (view: EditorView, held: boolean): void => {
    view.dom.classList.toggle(CTRL_HELD_CLASS, held);
  };

  return [
    linkHoverTooltip(),
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
