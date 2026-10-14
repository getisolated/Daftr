import { Injectable, OnDestroy, computed, inject, signal } from '@angular/core';
import { formatAccelerator } from '../utils/accelerator.util';
import { resolveAppShortcuts } from '../models/app-shortcuts.model';
import { AppSettings, DEFAULT_SETTINGS, SettingsPatch, SettingsUpdateResult } from '../models/settings.model';
import { ElectronBridgeService } from './electron-bridge.service';

@Injectable({ providedIn: 'root' })
export class SettingsService implements OnDestroy {
  private bridge = inject(ElectronBridgeService);
  private readonly _settings = signal<AppSettings>(DEFAULT_SETTINGS);
  private readonly _isOpen = signal(false);
  private readonly _loaded = signal(false);
  private readonly unsubscribers: (() => void)[];

  readonly settings = this._settings.asReadonly();
  readonly isOpen = this._isOpen.asReadonly();
  /** False until the main process answered: before that, `settings` only holds the defaults. */
  readonly loaded = this._loaded.asReadonly();
  /** Accelerator in effect for every rebindable in-app shortcut, by shortcut id. */
  readonly appShortcuts = computed(() => resolveAppShortcuts(this._settings().appShortcuts));

  constructor() {
    this.load();
    // Keeps every window in sync (e.g. the overlay picks up "Enter to send" changed from the main app)
    this.unsubscribers = [
      this.bridge.onSettingsChanged(settings => this._settings.set(settings)),
      // "Settings..." from the tray menu
      this.bridge.onOpenSettings(() => this.open()),
    ];
  }

  /** Human-readable shortcut currently bound to an in-app action, for hints and tooltips. */
  shortcutLabel(id: string): string {
    return formatAccelerator(this.appShortcuts()[id] ?? '');
  }

  async load(): Promise<void> {
    const settings = await this.bridge.getSettings();
    if (settings) {
      this._settings.set(settings);
      this._loaded.set(true);
    }
  }

  /** Applies a change; when the main process refuses it (e.g. shortcut already taken), the signal is left untouched. */
  async update(patch: SettingsPatch): Promise<SettingsUpdateResult | null> {
    const result = await this.bridge.setSettings(patch);
    if (result) this._settings.set(result.settings);
    return result;
  }

  open(): void { this._isOpen.set(true); }
  close(): void { this._isOpen.set(false); }

  ngOnDestroy(): void {
    this.unsubscribers.forEach(unsubscribe => unsubscribe());
  }
}
