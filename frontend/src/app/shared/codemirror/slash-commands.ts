import {
  ViewPlugin, ViewUpdate, EditorView, WidgetType, Decoration, DecorationSet,
  keymap,
} from '@codemirror/view';
import { StateField, StateEffect, EditorState, Transaction } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';
import { FormatId } from '../../core/models/formatting-bar.model';
import { Translator } from '../../core/i18n/translate';
import { TranslationKey } from '../../core/i18n/translations';
import { translatorOf } from './translator';
import {
  FORMAT_ACTIONS, clearInlineFormatting, insertCodeBlock, insertHorizontalRule, insertLinePrefix, insertTable, insertWrap,
} from './formatting-commands';

// ─── Slash Command Definitions ───────────────────────────────────────────────

export interface SlashCommand {
  id: string;
  labelKey: TranslationKey;
  groupKey: TranslationKey;
  icon: string;       // SVG path(s) for 24x24 viewBox
  descriptionKey: TranslationKey;
  execute: (view: EditorView, range: { from: number; to: number }) => void;
}

// Returns true if cursor is inside a fenced code block
export function isInCodeBlock(state: EditorState, pos: number): boolean {
  let node = syntaxTree(state).resolveInner(pos, -1);
  while (node.parent) {
    if (node.name === 'FencedCode' || node.name === 'InlineCode') return true;
    node = node.parent;
  }
  return false;
}

const GROUP_STYLE = 'editor.slash.group.style';
const GROUP_FORMATTING = 'editor.slash.group.formatting';
const GROUP_INSERT = 'editor.slash.group.insert';

export const slashCommands: SlashCommand[] = [
  // ── Style ──
  slashCommand('paragraph', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '')),
  slashCommand('h1', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '# ')),
  slashCommand('h2', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '## ')),
  slashCommand('h3', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '### ')),
  slashCommand('h4', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '#### ')),
  slashCommand('h5', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '##### ')),
  slashCommand('h6', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '###### ')),
  slashCommand('bulletList', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '- ')),
  slashCommand('orderedList', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '1. ')),
  slashCommand('taskList', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '- [ ] ')),
  slashCommand('quote', GROUP_STYLE, (view, range) => insertLinePrefix(view, range, '> ')),
  slashCommand('codeBlock', GROUP_STYLE, (view, range) => insertCodeBlock(view, range)),
  // ── Formatage ──
  slashCommand('bold', GROUP_FORMATTING, (view, range) => insertWrap(view, range, '**', '**', placeholder(view, 'editor.insert.text'))),
  slashCommand('italic', GROUP_FORMATTING, (view, range) => insertWrap(view, range, '*', '*', placeholder(view, 'editor.insert.text'))),
  slashCommand('strikethrough', GROUP_FORMATTING, (view, range) => insertWrap(view, range, '~~', '~~', placeholder(view, 'editor.insert.text'))),
  slashCommand('inlineCode', GROUP_FORMATTING, (view, range) => insertWrap(view, range, '`', '`', placeholder(view, 'editor.insert.code'))),
  slashCommand('underline', GROUP_FORMATTING, (view, range) => insertWrap(view, range, '<u>', '</u>', placeholder(view, 'editor.insert.text'))),
  slashCommand('superscript', GROUP_FORMATTING, (view, range) => insertWrap(view, range, '<sup>', '</sup>', placeholder(view, 'editor.insert.text'))),
  slashCommand('subscript', GROUP_FORMATTING, (view, range) => insertWrap(view, range, '<sub>', '</sub>', placeholder(view, 'editor.insert.text'))),
  slashCommand('clearFormatting', GROUP_FORMATTING, (view, range) => {
    // The typed `/query` goes first, then the caret line is cleared
    view.dispatch({ changes: { from: range.from, to: range.to } });
    clearInlineFormatting(view);
  }),
  // ── Insérer ──
  slashCommand('hr', GROUP_INSERT, (view, range) => insertHorizontalRule(view, range)),
  slashCommand('table', GROUP_INSERT, (view, range) => insertTable(view, range)),
  slashCommand('link', GROUP_INSERT, (view, range) => insertWrap(view, range, '[', '](url)', placeholder(view, 'editor.insert.text'))),
  slashCommand('image', GROUP_INSERT, (view, range) => insertWrap(view, range, '![', '](url)', placeholder(view, 'editor.insert.image'))),
];

/** Text a command puts in the note for the user to replace, in the language of the editor. */
function placeholder(view: EditorView, key: TranslationKey): string {
  return translatorOf(view.state)(key);
}

/** Label, icon and description come from the shared formatting actions, so the slash menu and the toolbar never drift. */
function slashCommand(id: FormatId, groupKey: TranslationKey, execute: SlashCommand['execute']): SlashCommand {
  const { labelKey, icon, descriptionKey } = FORMAT_ACTIONS[id];
  return { id, labelKey, groupKey, icon, descriptionKey, execute };
}

// ─── State Management ────────────────────────────────────────────────────────

interface SlashMenuState {
  active: boolean;
  from: number;      // position of the '/'
  to: number;        // end of the query text
  query: string;
  selectedIndex: number;
  filteredCommands: SlashCommand[];
}

const defaultState: SlashMenuState = {
  active: false, from: 0, to: 0, query: '', selectedIndex: 0, filteredCommands: [],
};

const openSlashMenu = StateEffect.define<{ from: number; to: number }>();
const closeSlashMenu = StateEffect.define<void>();
const updateSlashQuery = StateEffect.define<{ to: number; query: string }>();
const setSelectedIndex = StateEffect.define<number>();

function filterCommands(query: string, t: Translator): SlashCommand[] {
  if (!query) return slashCommands;
  const q = query.toLowerCase();
  return slashCommands.filter(cmd =>
    t(cmd.labelKey).toLowerCase().includes(q) ||
    t(cmd.descriptionKey).toLowerCase().includes(q) ||
    cmd.id.toLowerCase().includes(q)
  );
}

export const slashMenuState = StateField.define<SlashMenuState>({
  create: () => defaultState,
  update(state: SlashMenuState, tr: Transaction) {
    let newState = state;
    for (const effect of tr.effects) {
      if (effect.is(openSlashMenu)) {
        const filtered = filterCommands('', translatorOf(tr.startState));
        newState = {
          active: true,
          from: effect.value.from,
          to: effect.value.to,
          query: '',
          selectedIndex: 0,
          filteredCommands: filtered,
        };
      } else if (effect.is(closeSlashMenu)) {
        newState = defaultState;
      } else if (effect.is(updateSlashQuery)) {
        const filtered = filterCommands(effect.value.query, translatorOf(tr.startState));
        newState = {
          ...state,
          to: effect.value.to,
          query: effect.value.query,
          filteredCommands: filtered,
          selectedIndex: Math.min(state.selectedIndex, Math.max(0, filtered.length - 1)),
        };
      } else if (effect.is(setSelectedIndex)) {
        newState = { ...state, selectedIndex: effect.value };
      }
    }
    return newState;
  },
});

// ─── Slash Menu DOM Widget ───────────────────────────────────────────────────

class SlashMenuWidget {
  private dom: HTMLElement | null = null;
  private currentView: EditorView | null = null;

  show(view: EditorView, state: SlashMenuState) {
    this.currentView = view;

    if (!this.dom) {
      this.dom = document.createElement('div');
      this.dom.className = 'cm-slash-menu';
      this.dom.addEventListener('mousedown', (e) => e.preventDefault());
      document.body.appendChild(this.dom);
    }

    this.render(state);
    this.dom.style.display = 'block';

    // Position after the DOM is visible so measurements are accurate
    requestAnimationFrame(() => this.position(view, state.from));
  }

  hide() {
    if (this.dom) {
      this.dom.style.display = 'none';
    }
    this.currentView = null;
  }

  destroy() {
    if (this.dom) {
      this.dom.remove();
      this.dom = null;
    }
  }

  private position(view: EditorView, pos: number) {
    if (!this.dom) return;
    const coords = view.coordsAtPos(pos);
    if (!coords) {
      // Fallback: position near top-left of editor
      const rect = view.dom.getBoundingClientRect();
      this.dom.style.top = `${rect.top + 40}px`;
      this.dom.style.left = `${rect.left + 48}px`;
      return;
    }

    // Position below the cursor
    let top = coords.bottom + 4;
    let left = coords.left;

    // Ensure menu stays within viewport
    const menuHeight = this.dom.offsetHeight || 320;
    const menuWidth = this.dom.offsetWidth || 240;

    if (top + menuHeight > window.innerHeight) {
      top = coords.top - menuHeight - 4;
    }
    if (left + menuWidth > window.innerWidth) {
      left = window.innerWidth - menuWidth - 8;
    }
    if (left < 0) left = 8;

    this.dom.style.top = `${top}px`;
    this.dom.style.left = `${left}px`;
  }

  private render(state: SlashMenuState) {
    if (!this.dom || !this.currentView) return;

    const { filteredCommands, selectedIndex } = state;
    const t = translatorOf(this.currentView.state);

    if (filteredCommands.length === 0) {
      this.dom.innerHTML = `<div class="cm-slash-empty">${this.escapeHtml(t('editor.slash.empty'))}</div>`;
      return;
    }

    // Group commands
    const groups = new Map<TranslationKey, { cmd: SlashCommand; globalIndex: number }[]>();
    filteredCommands.forEach((cmd, i) => {
      if (!groups.has(cmd.groupKey)) groups.set(cmd.groupKey, []);
      groups.get(cmd.groupKey)!.push({ cmd, globalIndex: i });
    });

    let html = '';
    for (const [group, items] of groups) {
      html += `<div class="cm-slash-group-label">${this.escapeHtml(t(group))}</div>`;
      for (const { cmd, globalIndex } of items) {
        const isSelected = globalIndex === selectedIndex;
        html += `<div class="cm-slash-item${isSelected ? ' cm-slash-item-selected' : ''}" data-index="${globalIndex}">
          <div class="cm-slash-icon"><svg width="18" height="18" viewBox="0 0 24 24" fill="none">${cmd.icon}</svg></div>
          <div class="cm-slash-item-content">
            <span class="cm-slash-item-label">${this.escapeHtml(t(cmd.labelKey))}</span>
            <span class="cm-slash-item-desc">${this.escapeHtml(t(cmd.descriptionKey))}</span>
          </div>
        </div>`;
      }
    }

    this.dom.innerHTML = html;

    // Add click handlers
    this.dom.querySelectorAll('.cm-slash-item').forEach((el) => {
      el.addEventListener('mousedown', (e) => {
        e.preventDefault();
        e.stopPropagation();
        const index = parseInt((el as HTMLElement).dataset['index']!);
        const view = this.currentView;
        if (!view) return;
        const menuState = view.state.field(slashMenuState);
        const cmd = menuState.filteredCommands[index];
        if (cmd) {
          view.dispatch({ effects: closeSlashMenu.of(undefined) });
          cmd.execute(view, { from: menuState.from, to: menuState.to });
        }
      });
    });

    // Scroll selected into view
    const selectedEl = this.dom.querySelector('.cm-slash-item-selected');
    if (selectedEl) {
      selectedEl.scrollIntoView({ block: 'nearest' });
    }
  }

  private escapeHtml(text: string): string {
    const d = document.createElement('div');
    d.textContent = text;
    return d.innerHTML;
  }
}

// ─── Placeholder for empty lines ─────────────────────────────────────────────

class PlaceholderWidget extends WidgetType {
  constructor(private readonly text: string) {
    super();
  }

  override eq(other: PlaceholderWidget): boolean {
    return other.text === this.text;
  }

  override toDOM(): HTMLElement {
    const span = document.createElement('span');
    span.className = 'cm-slash-placeholder';
    span.textContent = this.text;
    return span;
  }

  override ignoreEvent(): boolean {
    return true;
  }
}

function buildPlaceholderDecorations(view: EditorView): DecorationSet {
  const state = view.state.field(slashMenuState);
  // Don't show placeholder when slash menu is active
  if (state.active) return Decoration.none;

  const doc = view.state.doc;
  // Only show placeholder on the current cursor line if it's empty
  const sel = view.state.selection.main;
  const cursorLine = doc.lineAt(sel.head);

  if (cursorLine.length === 0 && sel.empty) {
    return Decoration.set([
      Decoration.widget({
        widget: new PlaceholderWidget(translatorOf(view.state)('editor.slash.placeholder')),
        side: 1,
      }).range(cursorLine.from),
    ]);
  }

  return Decoration.none;
}

const placeholderPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildPlaceholderDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet) {
        this.decorations = buildPlaceholderDecorations(update.view);
      }
    }
  },
  { decorations: (v: { decorations: DecorationSet }) => v.decorations }
);

// ─── Input handler for '/' detection ─────────────────────────────────────────
// Uses EditorView.inputHandler instead of keymap for reliable character detection
// across all keyboard layouts (AZERTY, QWERTZ, etc.)

const slashInputHandler = EditorView.inputHandler.of(
  (view: EditorView, from: number, to: number, text: string) => {
    // Only handle single '/' character input (no replacements)
    if (text !== '/') return false;

    const menuState = view.state.field(slashMenuState);
    if (menuState.active) return false;

    if (isInCodeBlock(view.state, from)) return false;

    const line = view.state.doc.lineAt(from);
    const textBefore = view.state.sliceDoc(line.from, from);

    // Only activate if at start of line (optionally preceded by whitespace)
    if (textBefore.trim() !== '') return false;

    // Insert '/' and open the menu
    view.dispatch({
      changes: { from, to, insert: '/' },
      selection: { anchor: from + 1 },
      effects: openSlashMenu.of({ from, to: from + 1 }),
    });
    return true;
  }
);

// ─── Main Plugin ─────────────────────────────────────────────────────────────

const slashMenuPlugin = ViewPlugin.fromClass(
  class {
    private widget = new SlashMenuWidget();
    private trackTimeout: ReturnType<typeof setTimeout> | null = null;

    constructor(private view: EditorView) {
      this.syncWidget();
    }

    update(update: ViewUpdate) {
      const state = update.state.field(slashMenuState);
      const prevState = update.startState.field(slashMenuState);

      // Sync widget display on state changes
      if (state.active !== prevState.active ||
          state.query !== prevState.query ||
          state.selectedIndex !== prevState.selectedIndex ||
          state.filteredCommands !== prevState.filteredCommands) {
        this.syncWidget();
      }

      // Track typing after '/' was inserted
      if (state.active && update.docChanged) {
        this.scheduleTrackQuery(update);
      }

      // Close menu if cursor moves away from the slash range
      if (state.active && update.selectionSet && !update.docChanged) {
        const sel = update.state.selection.main;
        if (state.from >= update.state.doc.length) {
          update.view.dispatch({ effects: closeSlashMenu.of(undefined) });
        } else {
          const line = update.state.doc.lineAt(state.from);
          const cursorLine = update.state.doc.lineAt(sel.head);
          if (cursorLine.number !== line.number || sel.head < state.from) {
            update.view.dispatch({ effects: closeSlashMenu.of(undefined) });
          }
        }
      }
    }

    destroy() {
      this.widget.destroy();
      if (this.trackTimeout) clearTimeout(this.trackTimeout);
    }

    private syncWidget() {
      const state = this.view.state.field(slashMenuState);
      if (state.active && state.filteredCommands.length > 0) {
        this.widget.show(this.view, state);
      } else {
        this.widget.hide();
      }
    }

    private scheduleTrackQuery(update: ViewUpdate) {
      // Defer the dispatch to avoid dispatching inside update synchronously
      if (this.trackTimeout) clearTimeout(this.trackTimeout);
      const view = update.view;
      this.trackTimeout = setTimeout(() => {
        this.trackTimeout = null;
        const state = view.state.field(slashMenuState);
        if (!state.active) return;

        // Safety check: make sure from is within document bounds
        if (state.from >= view.state.doc.length) {
          view.dispatch({ effects: closeSlashMenu.of(undefined) });
          return;
        }

        const line = view.state.doc.lineAt(state.from);
        const textFromSlash = view.state.sliceDoc(state.from, line.to);

        // If the slash itself was deleted, close menu
        if (!textFromSlash.startsWith('/')) {
          view.dispatch({ effects: closeSlashMenu.of(undefined) });
          return;
        }

        const query = textFromSlash.slice(1); // remove the '/' prefix

        // Close if user typed a space (they're just writing a normal /)
        if (query.includes(' ')) {
          view.dispatch({ effects: closeSlashMenu.of(undefined) });
          return;
        }

        view.dispatch({
          effects: updateSlashQuery.of({ to: line.to, query }),
        });
      }, 0);
    }
  }
);

// ─── Keymap for menu navigation ──────────────────────────────────────────────

export const slashCommandKeymap = keymap.of([
  {
    key: 'ArrowDown',
    run(view: EditorView) {
      const state = view.state.field(slashMenuState);
      if (!state.active) return false;
      const next = Math.min(state.selectedIndex + 1, state.filteredCommands.length - 1);
      view.dispatch({ effects: setSelectedIndex.of(next) });
      return true;
    },
  },
  {
    key: 'ArrowUp',
    run(view: EditorView) {
      const state = view.state.field(slashMenuState);
      if (!state.active) return false;
      const prev = Math.max(state.selectedIndex - 1, 0);
      view.dispatch({ effects: setSelectedIndex.of(prev) });
      return true;
    },
  },
  {
    key: 'Enter',
    run(view: EditorView) {
      const state = view.state.field(slashMenuState);
      if (!state.active) return false;
      const cmd = state.filteredCommands[state.selectedIndex];
      if (cmd) {
        view.dispatch({ effects: closeSlashMenu.of(undefined) });
        cmd.execute(view, { from: state.from, to: state.to });
      }
      return true;
    },
  },
  {
    key: 'Escape',
    run(view: EditorView) {
      const state = view.state.field(slashMenuState);
      if (!state.active) return false;
      view.dispatch({ effects: closeSlashMenu.of(undefined) });
      return true;
    },
  },
  {
    key: 'Tab',
    run(view: EditorView) {
      const state = view.state.field(slashMenuState);
      if (!state.active) return false;
      const cmd = state.filteredCommands[state.selectedIndex];
      if (cmd) {
        view.dispatch({ effects: closeSlashMenu.of(undefined) });
        cmd.execute(view, { from: state.from, to: state.to });
      }
      return true;
    },
  },
]);

// ─── Export all extensions ───────────────────────────────────────────────────

export const slashCommandExtensions = [
  slashMenuState,
  slashInputHandler,
  slashCommandKeymap,
  slashMenuPlugin,
];

/** "Type / to see the commands" hint on the empty line under the cursor: notes only, not the quick notes document. */
export const slashPlaceholder = placeholderPlugin;
