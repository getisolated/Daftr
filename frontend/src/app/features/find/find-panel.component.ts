import { ChangeDetectionStrategy, Component, ElementRef, computed, effect, inject, viewChild } from '@angular/core';
import { PlatformService } from '../../core/services/platform.service';
import { formatAccelerator } from '../../core/utils/accelerator.util';
import { I18nService } from '../../core/i18n/i18n.service';
import { FindService } from '../../core/services/find.service';
import { MAX_FIND_MATCHES } from '../../shared/codemirror/find-matches';
import { FindOptions } from '../../core/models/find.model';

/** Find / replace panel of the right column (in place of the outline while open), like Word's navigation pane. */
@Component({
  selector: 'app-find-panel',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    role: 'search',
    '[class.read-only]': '!find.canReplace()',
    '(keydown)': 'onKeydown($event)',
  },
  template: `
    <div class="find-fields">
      <div class="find-row">
        @if (find.canReplace()) {
          <button
            type="button"
            class="icon-btn expander"
            [class.expanded]="find.replaceVisible()"
            [attr.aria-expanded]="find.replaceVisible()"
            [title]="i18n.t(find.replaceVisible() ? 'editor.find.hideReplace' : 'editor.find.showReplace')"
            (click)="find.toggleReplace()"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>
          </button>
        }
        <div class="field" [class.invalid]="find.invalidRegexp()">
          <input
            #searchInput
            type="text"
            [placeholder]="i18n.t('editor.find.find')"
            [attr.aria-label]="i18n.t('editor.find.find')"
            spellcheck="false"
            autocomplete="off"
            [value]="find.options().search"
            (input)="setText('search', $event)"
          />
          <div class="toggles">
            <button type="button" class="icon-btn toggle" [title]="i18n.t('editor.find.matchCase', { shortcut: toggleShortcut('C') })" [class.on]="find.options().caseSensitive" [attr.aria-pressed]="find.options().caseSensitive" (click)="toggle('caseSensitive')">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M2 17 6.5 6 11 17M3.6 13.5h5.8"/><path d="M19 17v-5.5a2.5 2.5 0 0 0-5 0M19 14.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0"/></svg>
            </button>
            <button type="button" class="icon-btn toggle" [title]="i18n.t('editor.find.wholeWord', { shortcut: toggleShortcut('W') })" [class.on]="find.options().wholeWord" [attr.aria-pressed]="find.options().wholeWord" (click)="toggle('wholeWord')">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 19h18M3 15v2M21 15v2"/><path d="M10 13v-3a2 2 0 0 0-4 0M10 12a2 2 0 1 1-4 0 2 2 0 0 1 4 0M13 6v7M13 11a2 2 0 1 0 4 0 2 2 0 0 0-4 0"/></svg>
            </button>
            <button type="button" class="icon-btn toggle" [title]="i18n.t('editor.find.regexp', { shortcut: toggleShortcut('R') })" [class.on]="find.options().regexp" [attr.aria-pressed]="find.options().regexp" (click)="toggle('regexp')">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3v10M12.67 5.5l8.66 5M12.67 10.5l8.66-5"/><circle cx="6" cy="18" r="2.2" fill="currentColor" stroke="none"/></svg>
            </button>
          </div>
        </div>
      </div>

      @if (find.replaceVisible()) {
        <div class="find-row">
          <span class="expander-spacer" aria-hidden="true"></span>
          <div class="field">
            <input
              type="text"
              [placeholder]="i18n.t('editor.find.replaceField')"
              [attr.aria-label]="i18n.t('editor.find.replaceField')"
              spellcheck="false"
              autocomplete="off"
              data-role="replace"
              [value]="find.options().replace"
              (input)="setText('replace', $event)"
            />
          </div>
        </div>
      }
    </div>

    <div class="find-toolbar">
      <span class="count" [class.empty]="noResults()" aria-live="polite">{{ countLabel() }}</span>
      <div class="actions">
        <button type="button" class="icon-btn" [title]="i18n.t('editor.find.previous', { shortcut: 'Shift+Enter' })" [disabled]="!hasMatches()" (click)="find.previous()">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 19V5M5 12l7-7 7 7"/></svg>
        </button>
        <button type="button" class="icon-btn" [title]="i18n.t('editor.find.next', { shortcut: 'Enter' })" [disabled]="!hasMatches()" (click)="find.next()">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14M19 12l-7 7-7-7"/></svg>
        </button>
        @if (find.replaceVisible()) {
          <span class="divider" aria-hidden="true"></span>
          <button type="button" class="icon-btn" [title]="i18n.t('editor.find.replace')" [disabled]="!hasMatches()" (click)="find.replace()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 4a2 2 0 0 1 2-2M16 10a2 2 0 0 1-2-2M20 2a2 2 0 0 1 2 2M22 8a2 2 0 0 1-2 2M3 7l3 3 3-3M6 10V5a3 3 0 0 1 3-3h1"/><rect x="2" y="14" width="8" height="8" rx="2"/></svg>
          </button>
          <button type="button" class="icon-btn" [title]="i18n.t('editor.find.replaceAll', { shortcut: replaceAllShortcut })" [disabled]="!hasMatches()" (click)="find.replaceAll()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 14a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2M14 4a2 2 0 0 1 2-2M16 10a2 2 0 0 1-2-2M20 14a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2M20 2a2 2 0 0 1 2 2M22 8a2 2 0 0 1-2 2M3 7l3 3 3-3M6 10V5a3 3 0 0 1 3-3h1"/><rect x="2" y="14" width="8" height="8" rx="2"/></svg>
          </button>
        }
        <span class="divider" aria-hidden="true"></span>
        <button type="button" class="icon-btn" [title]="i18n.t('common.close') + ' (Escape)'" (click)="find.close(true)">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18M6 6l12 12"/></svg>
        </button>
      </div>
    </div>

    @if (hasMatches()) {
      <div class="results" role="list" #results>
        @for (match of find.matches(); track match.from; let i = $index) {
          <button
            type="button"
            class="result"
            role="listitem"
            [class.current]="i === find.currentIndex()"
            [attr.data-index]="i"
            (click)="find.goTo(i)"
          >
            <span class="result-line">{{ match.line }}</span>
            <span class="result-text">{{ match.before }}<mark>{{ match.text }}</mark>{{ match.after }}</span>
          </button>
        }
      </div>
    }
  `,
  styleUrls: ['./find-panel.component.scss'],
})
export class FindPanelComponent {
  protected readonly find = inject(FindService);
  protected readonly i18n = inject(I18nService);
  private readonly platform = inject(PlatformService);
  // ⌥⌘↩ on macOS, Ctrl+Alt+Enter on Windows
  protected readonly replaceAllShortcut = formatAccelerator('CommandOrControl+Alt+Return', this.platform.platform);
  // ⌥C on macOS, Alt+C on Windows
  protected readonly toggleShortcut = (letter: string): string => formatAccelerator(`Alt+${letter}`, this.platform.platform);

  private readonly searchInput = viewChild<ElementRef<HTMLInputElement>>('searchInput');
  private readonly results = viewChild<ElementRef<HTMLElement>>('results');

  protected readonly hasMatches = computed(() => this.find.matches().length > 0);
  protected readonly noResults = computed(() => this.find.options().search !== '' && !this.hasMatches());

  protected readonly countLabel = computed(() => {
    const total = this.find.matches().length;

    if (this.find.invalidRegexp()) {
      return this.i18n.t('editor.find.invalid');
    }

    if (total === 0) {
      return this.find.options().search ? this.i18n.t('editor.find.noResults') : '';
    }

    const totalLabel = total >= MAX_FIND_MATCHES ? `${MAX_FIND_MATCHES}+` : `${total}`;
    const current = this.find.currentIndex();
    return current === -1
      ? this.i18n.plural('editor.find.results', total, { count: totalLabel })
      : this.i18n.t('editor.find.currentOf', { current: current + 1, total: totalLabel });
  });

  constructor() {
    effect(() => {
      this.find.focusRequest();
      const input = this.searchInput()?.nativeElement;
      input?.focus();
      input?.select();
    });

    // The current match stays visible in a long results list
    effect(() => {
      const index = this.find.currentIndex();
      const item = this.results()?.nativeElement.querySelector<HTMLElement>(`[data-index="${index}"]`);
      item?.scrollIntoView({ block: 'nearest' });
    });
  }

  protected setText(key: 'search' | 'replace', event: Event): void {
    const value = (event.target as HTMLInputElement).value;
    this.find.setOptions({ [key]: value });
  }

  protected toggle(key: keyof Pick<FindOptions, 'caseSensitive' | 'wholeWord' | 'regexp'>): void {
    this.find.setOptions({ [key]: !this.find.options()[key] });
  }

  protected onKeydown(event: KeyboardEvent): void {
    const inReplace = event.target instanceof HTMLElement && event.target.dataset['role'] === 'replace';

    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      this.find.close(true);
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      this.runEnter(event, inReplace);
      return;
    }

    // VS Code's option toggles
    const toggles: Record<string, keyof Pick<FindOptions, 'caseSensitive' | 'wholeWord' | 'regexp'>> = { c: 'caseSensitive', w: 'wholeWord', r: 'regexp' };
    const toggleKey = toggles[this.toggleLetter(event)];

    if (event.altKey && !this.primaryMod(event) && toggleKey) {
      event.preventDefault();
      this.toggle(toggleKey);
    }
  }

  /**
   * Letter of an option toggle. On macOS ⌥ turns the letter into a symbol (⌥C gives `ç`): the physical key
   * (`code`) is read instead, as `keyEventToAccelerator` does.
   */
  private toggleLetter(event: KeyboardEvent): string {
    const key = event.key.toLowerCase();
    if (!this.platform.isMac || /^[a-z]$/.test(key)) {
      return key;
    }
    return /^Key(?<letter>[A-Z])$/.exec(event.code)?.groups?.['letter']?.toLowerCase() ?? key;
  }

  /** The primary modifier: ⌘ on macOS, Ctrl elsewhere. */
  private primaryMod(event: KeyboardEvent): boolean {
    return this.platform.isMac ? event.metaKey : event.ctrlKey;
  }

  private runEnter(event: KeyboardEvent, inReplace: boolean): void {
    if (inReplace && this.primaryMod(event) && event.altKey) {
      this.find.replaceAll();
    } else if (inReplace) {
      this.find.replace();
    } else if (event.shiftKey) {
      this.find.previous();
    } else {
      this.find.next();
    }
  }
}
