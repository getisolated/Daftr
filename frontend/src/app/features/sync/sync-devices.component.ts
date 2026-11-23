import { ChangeDetectionStrategy, Component, OnInit, computed, inject, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { TrustedDevice } from '../../core/models/sync.model';
import { ProfilesService } from '../../core/services/profiles.service';
import { SyncService } from '../../core/services/sync.service';
import { formatNoteTimestamp } from '../../core/utils/note-timestamp.util';
import { profileDisplayName } from '../../core/utils/profile.util';
import { lastSyncAt, sharedProfileIds } from '../../core/utils/sync.util';
import { ConfirmDialogAction, ConfirmDialogComponent } from '../../shared/ui/confirm-dialog.component';

/** The trusted devices: name, presence, profiles shared, last sync, and "Remove" (final, after a confirmation). */
@Component({
  selector: 'app-sync-devices',
  standalone: true,
  imports: [ConfirmDialogComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @for (device of devices(); track device.key) {
      <div class="device" [class.revoked]="device.revokedAt !== null">
        <span class="dot" [class.ok]="device.connected" [class.busy]="device.present && !device.connected"></span>
        <div class="device-text">
          <div class="name">{{ device.name }} <span class="presence">· {{ presenceOf(device) }}</span></div>
          <div class="hint">{{ profilesOf(device) }}</div>
          <div class="hint">{{ lastSyncOf(device) }}</div>
        </div>
        @if (device.revokedAt === null) {
          <button class="btn danger" type="button" [disabled]="busy()" (click)="removing.set(device)">{{ i18n.t('sync.devices.remove') }}</button>
        }
      </div>
    } @empty {
      <div class="hint">{{ i18n.t('sync.devices.empty') }}</div>
    }
    @if (error(); as message) {
      <div class="error" role="alert">{{ message }}</div>
    }
    @if (removing(); as device) {
      <app-confirm-dialog
        [title]="i18n.t('sync.devices.removeTitle', { name: device.name })"
        [message]="i18n.t('sync.devices.removeMessage', { name: device.name })"
        [actions]="removeActions()"
        (chosen)="onRemoveChosen(device, $event)"
      />
    }
  `,
  styleUrls: ['./sync-panel.scss'],
  styles: [`
    .device {
      display: flex;
      align-items: flex-start;
      gap: 10px;
      padding: 8px 0;
      border-bottom: 1px solid var(--daftr-border-subtle);
    }
    .device:last-of-type { border-bottom: none; }
    .device .dot { margin-top: 5px; }
    .device-text { flex: 1; min-width: 0; }
    .device-text .hint { margin-top: 2px; }
    .name { color: var(--daftr-text-strong); user-select: text; }
    .presence { color: var(--daftr-text-soft); }
    .revoked .name { color: var(--daftr-text-dimmed); text-decoration: line-through; }
  `],
})
export class SyncDevicesComponent implements OnInit {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly profiles = inject(ProfilesService);

  protected readonly devices = this.sync.devices;
  protected readonly removing = signal<TrustedDevice | null>(null);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly removeActions = computed<ConfirmDialogAction[]>(() => [
    { id: 'remove', label: this.i18n.t('sync.devices.remove'), danger: true },
  ]);

  ngOnInit(): void {
    void this.sync.loadDevices();
  }

  protected presenceOf(device: TrustedDevice): string {
    if (device.revokedAt !== null) {
      return this.i18n.t('sync.devices.removedState');
    }

    if (device.connected) {
      return this.i18n.t('sync.devices.connected');
    }

    return this.i18n.t(device.present ? 'sync.devices.present' : 'sync.devices.absent');
  }

  protected profilesOf(device: TrustedDevice): string {
    const names = sharedProfileIds(device).map(id => {
      const profile = this.profiles.profiles().find(candidate => candidate.id === id);
      return profile ? profileDisplayName(profile, this.i18n.t) : id;
    });

    return names.length > 0 ? this.i18n.t('sync.devices.profiles', { names: names.join(', ') }) : this.i18n.t('sync.devices.noProfile');
  }

  protected lastSyncOf(device: TrustedDevice): string {
    const at = lastSyncAt(device);

    return at ? this.i18n.t('sync.devices.lastSync', { date: formatNoteTimestamp(at, this.i18n.language()) }) : this.i18n.t('sync.devices.never');
  }

  protected async onRemoveChosen(device: TrustedDevice, choice: string | null): Promise<void> {
    this.removing.set(null);

    if (choice !== 'remove') {
      return;
    }

    this.busy.set(true);
    this.error.set(null);

    try {
      await this.sync.removeDevice(device.key);
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
