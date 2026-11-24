import { ChangeDetectionStrategy, Component, computed, effect, inject, input, output, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { FolderSummary } from '../../core/models/collection-sync.model';
import { CollectionSyncService } from '../../core/services/collection-sync.service';
import { ProfilesService } from '../../core/services/profiles.service';
import { SyncService } from '../../core/services/sync.service';
import { formatBytes, tooLargeCount } from '../../core/utils/collection-sync.util';
import { matrixDevices } from '../../core/utils/sync-share.util';
import { CheckboxComponent } from '../../shared/ui/checkbox.component';

/** Width of the menu, in pixels (also its CSS width), to keep it inside the window. */
const MENU_WIDTH = 280;
/** Room kept under the menu when it opens near the bottom of the window. */
const MENU_HEIGHT_ESTIMATE = 260;
const WINDOW_MARGIN = 8;

/**
 * "Sync with…" of a collection of the sidebar: a small menu with a box per trusted device, applied as it is ticked. It
 * tells what the folder shares (files, size, a warning for a git folder). Without any linked device, it leads to the sync
 * screen.
 */
@Component({
  selector: 'app-collection-share-menu',
  standalone: true,
  imports: [CheckboxComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'closed.emit()',
  },
  template: `
    <div class="backdrop" (click)="closed.emit()" (contextmenu)="$event.preventDefault(); closed.emit()"></div>
    <div class="menu" role="dialog" [attr.aria-label]="i18n.t('csync.menu.share')" [style.left.px]="left()" [style.top.px]="top()">
      <div class="title">{{ i18n.t('csync.menu.title') }} <span class="name">{{ name() }}</span></div>

      @if (devices().length === 0) {
        <p class="hint">{{ i18n.t('csync.menu.noDevice') }}</p>
        <button class="link" type="button" (click)="manage()">{{ i18n.t('csync.menu.linkDevice') }}</button>
      } @else {
        <div class="devices" role="group" [attr.aria-label]="i18n.t('csync.menu.title')">
          @for (device of devices(); track device.key) {
            <app-checkbox [class.busy]="applying().has(device.key)" [checked]="isChecked(device.key)" (checkedChange)="toggle(device.key, $event)">
              {{ device.name }}
            </app-checkbox>
          }
        </div>

        @if (summary(); as folder) {
          <p class="info">{{ i18n.plural('csync.share.summary', folder.mdFiles, { size: size(folder) }) }}</p>
          @if (tooLarge(folder) > 0) {
            <p class="info">{{ i18n.plural('csync.share.tooLarge', tooLarge(folder)) }}</p>
          }
          @if (folder.truncated) {
            <p class="info">{{ i18n.t('csync.share.truncated') }}</p>
          }
          @if (folder.hasGit) {
            <p class="info warning"><span aria-hidden="true">⚠</span> {{ i18n.t('csync.share.git') }}</p>
          }
        }
        <button class="link" type="button" (click)="manage()">{{ i18n.t('csync.menu.manage') }}</button>
      }

      @if (error(); as message) {
        <p class="error" role="alert">{{ message }}</p>
      }
    </div>
  `,
  styles: [`
    :host { position: fixed; inset: 0; z-index: 3000; font-family: 'Segoe UI', system-ui, sans-serif; }
    .backdrop { position: absolute; inset: 0; }
    .menu {
      position: absolute;
      display: flex;
      flex-direction: column;
      gap: 8px;
      width: 280px;
      padding: 10px 12px;
      background: var(--daftr-bg-panel);
      border: 1px solid var(--daftr-border);
      border-radius: 6px;
      box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
      font-size: 12px;
    }
    .title { overflow: hidden; color: var(--daftr-text-strong); font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
    .name { color: var(--daftr-text-soft); font-weight: 400; }
    .devices { display: flex; flex-direction: column; gap: 6px; color: var(--daftr-text); }
    .busy { opacity: 0.5; pointer-events: none; }
    .info, .hint { margin: 0; font-size: 11.5px; line-height: 1.45; color: var(--daftr-text-soft); }
    .hint { color: var(--daftr-text-dimmed); }
    .warning { color: var(--daftr-text); }
    .error { margin: 0; color: var(--daftr-danger); }
    .link {
      align-self: flex-start;
      padding: 0;
      border: none;
      background: none;
      color: var(--daftr-accent-text);
      font: inherit;
      cursor: pointer;
    }
    .link:hover { text-decoration: underline; }
  `],
})
export class CollectionShareMenuComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly collectionSync = inject(CollectionSyncService);
  private readonly profiles = inject(ProfilesService);

  /** The collection (its path) and the name shown in the title. */
  readonly root = input.required<string>();
  readonly name = input.required<string>();
  /** Where the menu was asked, in window coordinates. */
  readonly x = input.required<number>();
  readonly y = input.required<number>();
  readonly closed = output<void>();

  protected readonly devices = computed(() => matrixDevices(this.sync.devices()));
  protected readonly summary = signal<FolderSummary | null>(null);
  protected readonly applying = signal<ReadonlySet<string>>(new Set());
  protected readonly error = signal<string | null>(null);
  protected readonly left = computed(() => Math.max(WINDOW_MARGIN, Math.min(this.x(), window.innerWidth - MENU_WIDTH - WINDOW_MARGIN)));
  protected readonly top = computed(() => Math.max(WINDOW_MARGIN, Math.min(this.y(), window.innerHeight - MENU_HEIGHT_ESTIMATE - WINDOW_MARGIN)));
  private readonly share = computed(() => this.collectionSync.shareOf(this.root()));

  constructor() {
    void this.sync.loadDevices();

    effect(() => {
      const root = this.root();
      untracked(() => { void this.loadSummary(root); });
    });
  }

  protected isChecked(deviceKey: string): boolean {
    return this.share()?.devices.some(device => device.key === deviceKey) ?? false;
  }

  protected size(folder: FolderSummary): string {
    return formatBytes(folder.bytes, this.i18n.language());
  }

  protected tooLarge(folder: FolderSummary): number {
    return tooLargeCount(folder);
  }

  protected async toggle(deviceKey: string, checked: boolean): Promise<void> {
    if (this.applying().has(deviceKey)) {
      return;
    }

    this.applying.update(keys => new Set(keys).add(deviceKey));
    this.error.set(null);

    try {
      await this.collectionSync.setDevice(this.profiles.activeId(), this.root(), deviceKey, checked);
    } catch (error) {
      this.error.set(this.collectionSync.errorMessage(error));
    } finally {
      this.applying.update(keys => {
        const next = new Set(keys);
        next.delete(deviceKey);
        return next;
      });
    }
  }

  /** The sync screen: where a device is linked, and where the collections are set up. */
  protected manage(): void {
    this.closed.emit();
    this.sync.openScreen();
  }

  private async loadSummary(root: string): Promise<void> {
    try {
      const summary = await this.collectionSync.summary(root, this.profiles.activeId());

      if (root === this.root()) {
        this.summary.set(summary);
      }
    } catch (error) {
      console.error('[Sync] Summary not read:', error);
    }
  }
}
