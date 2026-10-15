import { WidgetType, EditorView } from '@codemirror/view';

// ── Checkbox widget ───────────────────────────────────────────────────────────
export class CheckboxWidget extends WidgetType {
  constructor(
    readonly checked: boolean,
    readonly from: number,
    readonly syntax: '[]' | '()'
  ) { super(); }

  toDOM(view: EditorView): HTMLElement {
    const wrap = document.createElement('span');
    wrap.className = 'cm-checkbox-wrap';

    if (this.syntax === '[]') {
      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.checked = this.checked;
      cb.className = 'cm-task-checkbox';
      cb.addEventListener('mousedown', (e) => e.preventDefault());
      cb.addEventListener('change', () => {
        const insert = this.syntax === '[]'
          ? (cb.checked ? '[x]' : '[ ]')
          : (cb.checked ? '(x)' : '( )');
        view.dispatch({
          changes: { from: this.from, to: this.from + 3, insert }
        });
      });
      wrap.appendChild(cb);
    } else {
      // Circle bullet for () syntax
      const bullet = document.createElement('span');
      bullet.className = this.checked ? 'cm-bullet-done' : 'cm-bullet-todo';
      bullet.innerHTML = this.checked ? '●' : '○';
      bullet.addEventListener('mousedown', (e) => e.preventDefault());
      bullet.addEventListener('click', () => {
        const insert = this.checked ? '( )' : '(x)';
        view.dispatch({
          changes: { from: this.from, to: this.from + 3, insert }
        });
      });
      wrap.appendChild(bullet);
    }

    return wrap;
  }

  override eq(other: CheckboxWidget): boolean {
    return this.checked === other.checked &&
           this.from === other.from &&
           this.syntax === other.syntax;
  }

  override ignoreEvent(): boolean { return false; }
}

// ── External link icon widget ─────────────────────────────────────────────────
// Any scheme except `note://`, which targets a local note
export const EXTERNAL_URL = /^(?!note:\/\/)[a-z][a-z0-9+.-]*:\/\//i;

export class ExternalLinkWidget extends WidgetType {
  toDOM(): HTMLElement {
    const span = document.createElement('span');
    span.className = 'cm-external-link-icon';
    span.setAttribute('aria-hidden', 'true');
    span.innerHTML =
      '<svg viewBox="0 0 16 16" width="1em" height="1em" fill="none" stroke="currentColor" ' +
      'stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">' +
      '<path d="M9 2.5h4.5V7M13.5 2.5 7.5 8.5M12 9.5v3a1 1 0 0 1-1 1H3.5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1h3"/>' +
      '</svg>';
    return span;
  }

  override eq(): boolean { return true; }
  override ignoreEvent(): boolean { return true; }
}

// ── Heading level widget (hidden # symbols) ───────────────────────────────────
export class HiddenMarkWidget extends WidgetType {
  toDOM(): HTMLElement {
    const span = document.createElement('span');
    span.className = 'cm-hidden-mark';
    return span;
  }
  override eq(): boolean { return true; }
  override ignoreEvent(): boolean { return true; }
}
