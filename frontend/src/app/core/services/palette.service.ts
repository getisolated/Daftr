import { Injectable, signal } from '@angular/core';
import { PaletteMode } from '../models/note.model';

@Injectable({ providedIn: 'root' })
export class PaletteService {
  private readonly _isOpen = signal(false);
  private readonly _mode = signal<PaletteMode>('command');
  private readonly _initialQuery = signal('');

  readonly isOpen = this._isOpen.asReadonly();
  readonly mode = this._mode.asReadonly();
  readonly initialQuery = this._initialQuery.asReadonly();

  open(mode: PaletteMode = 'command', initialQuery = ''): void {
    this._mode.set(mode);
    this._initialQuery.set(initialQuery);
    this._isOpen.set(true);
  }

  close(): void {
    this._isOpen.set(false);
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
