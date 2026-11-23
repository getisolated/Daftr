import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { SettingsService } from '../../core/services/settings.service';
import { SyncService } from '../../core/services/sync.service';

/** Controls of the sync settings that are more than a value: its state with "Sync now", and the name of the device. */
@Component({
  selector: 'app-sync-settings',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (part() === 'status') {
      @if (indicator(); as shown) {
        <div class="status">
          <span class="dot" [class]="shown.tone"></span>
          <span>{{ shown.label }}</span>
        </div>
        @if (shown.reason; as reason) {
          <div class="error">{{ reason }}</div>
        }
      }
      @if (overview(); as state) {
        <div class="hint">
          {{ i18n.t('sync.settings.thisDevice', { name: state.deviceName }) }}
          @if (state.listeningPort !== null) {
            · {{ i18n.t('sync.settings.listening', { port: state.listeningPort }) }}
          }
        </div>
      }
      <div class="row actions">
        <button class="btn" type="button" [disabled]="!canSyncNow() || busy()" (click)="syncNow()">{{ i18n.t('sync.settings.syncNow') }}</button>
      </div>
    } @else {
      <input
        class="field name"
        type="text"
        maxlength="64"
        spellcheck="false"
        [placeholder]="hostName()"
        [value]="settings().syncDeviceName"
        (change)="rename($event)"
      />
    }
    @if (error(); as message) {
      <div class="error">{{ message }}</div>
    }
  `,
  styleUrls: ['./sync-panel.scss'],
  styles: [`
    .actions { margin-top: 8px; }
    .name { width: 280px; }
  `],
})
export class SyncSettingsComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly settingsService = inject(SettingsService);

  /** `status`: state and "Sync now"; `deviceName`: the name of this device. */
  readonly part = input.required<'status' | 'deviceName'>();

  protected readonly overview = this.sync.overview;
  protected readonly indicator = this.sync.indicator;
  protected readonly settings = this.settingsService.settings;
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  /** Shown while the name is empty: the name the other devices see then. */
  protected readonly hostName = computed(() => {
    const overview = this.overview();
    return overview?.deviceNameIsDefault ? overview.deviceName : '';
  });
  protected readonly canSyncNow = computed(() => {
    const state = this.sync.status()?.state;
    return state !== undefined && state !== 'disabled' && state !== 'paused';
  });

  protected async syncNow(): Promise<void> {
    await this.run(() => this.sync.syncNow());
  }

  protected async rename(event: Event): Promise<void> {
    const field = event.target as HTMLInputElement;
    await this.run(() => this.sync.setDeviceName(field.value));
    // The name actually kept (trimmed, or refused)
    field.value = this.settings().syncDeviceName;
  }

  private async run(action: () => Promise<void>): Promise<void> {
    this.busy.set(true);
    this.error.set(null);

    try {
      await action();
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
