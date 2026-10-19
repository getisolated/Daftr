import { Injectable, computed, inject, signal } from '@angular/core';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { FormatId } from '../models/formatting-bar.model';
import { detectActiveFormats } from '../../shared/codemirror/formatting-commands';
import { SettingsService } from './settings.service';

function sameFormats(a: ReadonlySet<FormatId>, b: ReadonlySet<FormatId>): boolean {
  return a.size === b.size && [...a].every(id => b.has(id));
}

/** Preferences of the formatting bar (kept in the settings) and the editor it acts on. */
@Injectable({ providedIn: 'root' })
export class FormattingBarService {
  private readonly settingsService = inject(SettingsService);
  private readonly _view = signal<EditorView | null>(null);
  private readonly _readOnly = signal(false);
  private readonly _activeFormats = signal<ReadonlySet<FormatId>>(new Set(), { equal: sameFormats });

  /** false: the user does not want the bar at all. */
  readonly visible = computed(() => this.settingsService.settings().formattingBarEnabled);
  readonly mode = computed(() => this.settingsService.settings().formattingBarMode);
  readonly style = computed(() => this.settingsService.settings().formattingBarStyle);
  readonly position = computed(() => this.settingsService.settings().formattingBarPosition);

  readonly activeFormats = this._activeFormats.asReadonly();
  /** The notes sent from the overlay are shown read-only: nothing to format. */
  readonly canEdit = computed(() => this._view() !== null && !this._readOnly());

  toggle(): void {
    void this.settingsService.update({ formattingBarEnabled: !this.visible() });
  }

  /** Called by the editor each time it creates (or drops, with null) its CodeMirror view. */
  attach(view: EditorView | null, readOnly = false): void {
    this._view.set(view);
    this._readOnly.set(readOnly);
    this._activeFormats.set(view ? detectActiveFormats(view.state) : new Set());
  }

  /** Called on every selection or document change of the editor. */
  refresh(state: EditorState): void {
    this._activeFormats.set(detectActiveFormats(state));
  }

  run(execute: (view: EditorView) => void): void {
    const view = this._view();
    if (view && this.canEdit()) execute(view);
  }
}
