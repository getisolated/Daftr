import { Injectable, signal } from '@angular/core';
import { PaletteMode } from '../models/note.model';

@Injectable({ providedIn: 'root' })
export class PaletteService {
  private readonly _isOpen = signal(false);
  private readonly _mode = signal<PaletteMode>('command');
  private readonly _initialQuery = signal('');
  private readonly _quickNavigate = signal(false);
  private readonly _cycle = signal(0);

  readonly isOpen = this._isOpen.asReadonly();
  readonly mode = this._mode.asReadonly();
  readonly initialQuery = this._initialQuery.asReadonly();
  /** The shortcut was pressed again while open (Ctrl+E Ctrl+E…): releasing its modifier opens the highlighted entry. */
  readonly quickNavigate = this._quickNavigate.asReadonly();
  /** Incremented each time the shortcut is pressed again while open: the highlight moves to the next entry. */
  readonly cycle = this._cycle.asReadonly();

  open(mode: PaletteMode = 'command', initialQuery = '', quickNavigate = false): void {
    this._mode.set(mode);
    this._initialQuery.set(initialQuery);
    this._quickNavigate.set(quickNavigate);
    this._isOpen.set(true);
  }

  /**
   * Ctrl+E: opens the recents switcher. Pressed again while open (Ctrl held), it moves to the next document and, like
   * VS Code, releasing Ctrl then opens it.
   */
  openRecents(): void {
    if (this._isOpen() && this._mode() === 'recent') {
      this._quickNavigate.set(true);
      this._cycle.update(count => count + 1);
    } else {
      this.open('recent');
    }
  }

  close(): void {
    this._isOpen.set(false);
    this._quickNavigate.set(false);
    this._initialQuery.set('');
  }

  toggle(mode: PaletteMode = 'command'): void {
    if (this._isOpen()) {
      this.close();
    } else {
      this.open(mode);
    }
  }
}
