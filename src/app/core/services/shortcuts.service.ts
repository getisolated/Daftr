import { Injectable, OnDestroy, inject } from '@angular/core';
import { isModifierKey, keyEventToAccelerator } from '../utils/accelerator.util';
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
  private readonly blurListener = (): void => this.clearChord();
  /** First key of a chord already pressed (e.g. Ctrl+K), waiting — with no time limit, like VS Code — for the second one. */
  private pendingChord: string | null = null;

  constructor() {
    this.boundListener = this.handleKeydown.bind(this);
    window.addEventListener('keydown', this.boundListener, true);
    // The second key cannot come from another window: leaving the app cancels the chord
    window.addEventListener('blur', this.blurListener);
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
    const resolved = this.settings.appShortcuts();

    if (this.pendingChord !== null) {
      // A lone modifier press is part of typing the second key
      if (isModifierKey(event)) {
        return;
      }
      const first = this.pendingChord;
      const second = keyEventToAccelerator(event, { allowBareKey: true });
      this.clearChord();
      // Whatever follows the first key is consumed, even when it completes no chord (same as VS Code): Escape cancels it
      event.preventDefault();
      event.stopPropagation();
      if (!second) {
        return;
      }
      const chord = `${first}, ${second}`;
      for (const [id, handler] of this.actions) {
        if (resolved[id] === chord) {
          handler(event);
          return;
        }
      }
      return;
    }

    if (accelerator) {
      if ([...this.actions.keys()].some(id => resolved[id]?.startsWith(`${accelerator}, `))) {
        // Keeps the key from reaching the editor, which would run its own binding
        event.preventDefault();
        event.stopPropagation();
        this.pendingChord = accelerator;
        return;
      }
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

  private clearChord(): void {
    this.pendingChord = null;
  }

  ngOnDestroy(): void {
    this.clearChord();
    window.removeEventListener('keydown', this.boundListener, true);
    window.removeEventListener('blur', this.blurListener);
  }
}
