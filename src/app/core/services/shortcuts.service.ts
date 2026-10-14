import { Injectable, OnDestroy, inject } from '@angular/core';
import { keyEventToAccelerator } from '../utils/accelerator.util';
import { SettingsService } from './settings.service';

type ShortcutHandler = (event: KeyboardEvent) => void;

interface ShortcutDef {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  handler: ShortcutHandler;
  preventDefault?: boolean;
}

@Injectable({ providedIn: 'root' })
export class ShortcutsService implements OnDestroy {
  private settings = inject(SettingsService);
  private shortcuts: ShortcutDef[] = [];
  private actions = new Map<string, ShortcutHandler>();
  private boundListener: (e: KeyboardEvent) => void;

  constructor() {
    this.boundListener = this.handleKeydown.bind(this);
    window.addEventListener('keydown', this.boundListener, true);
  }

  register(def: ShortcutDef): () => void {
    this.shortcuts.push(def);
    return () => {
      this.shortcuts = this.shortcuts.filter(s => s !== def);
    };
  }

  /** Binds a handler to a rebindable shortcut (APP_SHORTCUTS id): the key combination is whatever the user configured. */
  bindAction(id: string, handler: ShortcutHandler): () => void {
    this.actions.set(id, handler);
    return () => { this.actions.delete(id); };
  }

  private handleKeydown(event: KeyboardEvent): void {
    // Let shortcut recorders (settings) receive raw key combos instead of triggering app actions
    if (event.target instanceof Element && event.target.closest('[data-shortcut-recorder]')) return;

    const accelerator = keyEventToAccelerator(event);
    if (accelerator) {
      const resolved = this.settings.appShortcuts();
      for (const [id, handler] of this.actions) {
        if (resolved[id] === accelerator) {
          event.preventDefault();
          handler(event);
          return;
        }
      }
    }

    for (const shortcut of this.shortcuts) {
      const keyMatch = event.key.toLowerCase() === shortcut.key.toLowerCase() ||
                       event.code.toLowerCase() === shortcut.key.toLowerCase();
      const ctrlMatch = !!shortcut.ctrl === (event.ctrlKey || event.metaKey);
      const shiftMatch = !!shortcut.shift === event.shiftKey;
      const altMatch = !!shortcut.alt === event.altKey;

      if (keyMatch && ctrlMatch && shiftMatch && altMatch) {
        if (shortcut.preventDefault !== false) event.preventDefault();
        shortcut.handler(event);
        break;
      }
    }
  }

  ngOnDestroy(): void {
    window.removeEventListener('keydown', this.boundListener, true);
  }
}
