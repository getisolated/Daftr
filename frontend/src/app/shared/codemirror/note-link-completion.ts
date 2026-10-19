import {
  Completion, CompletionContext, CompletionResult, autocompletion, startCompletion,
} from '@codemirror/autocomplete';
import { Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import {
  PathCompletion, PathFolder, PathNote, listPathCompletions, needsAngleBrackets,
} from '../../core/utils/note-paths.util';
import { isInCodeBlock } from './slash-commands';

/** What the completion offers, read each time it opens so it follows the renames and the new notes. */
export interface NoteLinkSource {
  notes: () => readonly PathNote[];
  folders: () => readonly PathFolder[];
}

/** The text before the caret ends inside the target of a markdown link: `[label](` + path typed so far. */
const LINK_TARGET = /\]\((?<open><?)(?<path>[^)>\n]*)$/;
const EXTERNAL_SCHEME = /^[a-z][a-z0-9+.-]*:/i;

/** Completion applied: a folder extends the path and reopens the list, a note closes the link. */
function applyEntry(entry: PathCompletion, angleOpen: boolean): Completion['apply'] {
  return (view, _completion, from, to) => {
    const angle = angleOpen || needsAngleBrackets(entry.path);
    const typed = angle ? `<${entry.path}` : entry.path;

    if (entry.kind === 'folder') {
      view.dispatch({ changes: { from, to, insert: typed }, selection: { anchor: from + typed.length } });
      startCompletion(view);
      return;
    }

    // The auto-closed bracket is already there: step over it instead of doubling it
    const hasCloser = view.state.sliceDoc(to, to + 1) === ')';
    const insert = `${typed}${angle ? '>' : ''})`;

    view.dispatch({
      changes: { from, to: hasCloser ? to + 1 : to, insert },
      selection: { anchor: from + insert.length },
    });
  };
}

/** Like the path completion of a code editor: after `](`, folders and notes to walk down to the target note. */
export function noteLinkCompletion(source: NoteLinkSource): Extension {
  const complete = (context: CompletionContext): CompletionResult | null => {
    const line = context.state.doc.lineAt(context.pos);
    const match = LINK_TARGET.exec(line.text.slice(0, context.pos - line.from));
    const open = match?.groups?.['open'] ?? '';
    const path = match?.groups?.['path'] ?? '';

    if (!match || EXTERNAL_SCHEME.test(path) || isInCodeBlock(context.state, context.pos)) {
      return null;
    }

    const options = listPathCompletions(path, source.notes(), source.folders()).map<Completion>(entry => ({
      label: entry.label,
      detail: entry.detail,
      type: entry.kind === 'folder' ? 'namespace' : 'text',
      apply: applyEntry(entry, open === '<'),
    }));

    // The options are already filtered by the path: CodeMirror must not filter them again on the typed text
    return options.length === 0 ? null : { from: context.pos - open.length - path.length, options, filter: false };
  };

  return [
    autocompletion({ override: [complete], icons: true }),
    EditorView.theme({
      '.cm-tooltip.cm-tooltip-autocomplete': {
        background: 'var(--evn-bg-menu)',
        border: '1px solid var(--evn-border)',
        borderRadius: '6px',
        overflow: 'hidden',
      },
      '.cm-tooltip-autocomplete > ul': { fontFamily: "'Segoe UI', system-ui, sans-serif", fontSize: '13px' },
      '.cm-tooltip-autocomplete > ul > li': { color: 'var(--evn-text)', padding: '3px 8px' },
      '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
        background: 'var(--evn-bg-selected)',
        color: 'var(--evn-text-bright)',
      },
      '.cm-completionDetail': { color: 'var(--evn-text-dimmed)', fontStyle: 'normal', marginLeft: '8px' },
      '.cm-completionIcon-namespace:after': { content: '"▸"' },
      '.cm-completionIcon-text:after': { content: '"¶"' },
    }),
  ];
}
