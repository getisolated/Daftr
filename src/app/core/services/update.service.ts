import { Injectable, OnDestroy, computed, inject, signal } from '@angular/core';
import { UpdateInfo, UpdateProgress } from '../models/update.model';
import { ElectronBridgeService } from './electron-bridge.service';

/**
 * Update state of the main window. The main process finds the updates (at startup, then hourly)
 * and pushes them here; the sidebar shows an entry and a popup offers to install.
 */
@Injectable({ providedIn: 'root' })
export class UpdateService implements OnDestroy {
  private readonly bridge = inject(ElectronBridgeService);
  private readonly _info = signal<UpdateInfo | null>(null);
  private readonly _progress = signal<UpdateProgress | null>(null);
  private readonly _applying = signal(false);
  private readonly _dialogOpen = signal(false);
  private readonly unsubscribers: (() => void)[];

  readonly info = this._info.asReadonly();
  readonly progress = this._progress.asReadonly();
  readonly applying = this._applying.asReadonly();
  readonly dialogOpen = this._dialogOpen.asReadonly();
  /** Short label of the sidebar entry, following the state of the update. */
  readonly label = computed(() => {
    const progress = this._progress();
    if (progress) {
      return progress.percent >= 0 ? `Downloading update… ${progress.percent}%` : 'Downloading update…';
    }
    if (this._applying()) return 'Installing update…';
    return this._info()?.downloaded ? 'Restart to update' : 'Update available';
  });

  constructor() {
    this.unsubscribers = [
      this.bridge.onUpdateAvailable(info => {
        this._info.set(info);
        // Offered once, when found; the sidebar entry reopens it afterwards
        this._dialogOpen.set(true);
      }),
      this.bridge.onUpdateProgress(progress => this._progress.set(progress)),
      this.bridge.onUpdateDownloaded(info => {
        this._progress.set(null);
        this._info.set(info);
      }),
      this.bridge.onUpdateDownloadFailed(error => {
        this._progress.set(null);
        console.warn('[Update] Download failed:', error);
      }),
    ];
    // A reloaded window misses the push: read what the main process already knows
    void this.bridge.getUpdateInfo().then(info => { if (info) this._info.set(info); });
  }

  openDialog(): void {
    if (this._info() && !this._applying()) this._dialogOpen.set(true);
  }

  closeDialog(): void { this._dialogOpen.set(false); }

  /** Downloads if needed, then installs and restarts the app (the process exits on success). */
  async install(): Promise<void> {
    this._dialogOpen.set(false);
    if (this._applying()) return;
    this._applying.set(true);
    try {
      await this.bridge.applyUpdate();
    } catch (err) {
      console.error('[Update] Install failed:', err);
    } finally {
      this._applying.set(false);
      this._progress.set(null);
    }
  }

  ngOnDestroy(): void {
    this.unsubscribers.forEach(unsubscribe => unsubscribe());
  }
}
