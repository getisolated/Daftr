import { ChangeDetectionStrategy, Component, OnInit, computed, inject, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { TrustedDevice } from '../../core/models/sync.model';
import { SyncService } from '../../core/services/sync.service';
import { shortDeviceId } from '../../core/utils/sync.util';
import { ConfirmDialogAction, ConfirmDialogComponent } from '../../shared/ui/confirm-dialog.component';
import { DeviceIconComponent } from './device-icon.component';

/** The trusted devices of the sync screen: icon, name, short identifier, presence, and "Remove" (final, confirmed). */
@Component({
  selector: 'app-sync-devices',
  standalone: true,
  imports: [ConfirmDialogComponent, DeviceIconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @for (device of devices(); track device.key) {
      <div class="device">
        <app-device-icon [kind]="device.kind" />
        <div class="text">
          <div class="name" [title]="device.name">{{ device.name }}</div>
          <div class="meta">
            <span class="id">{{ idOf(device) }}</span>
            <span class="dot" [class.ok]="device.connected" [class.busy]="device.present && !device.connected"></span>
            <span>{{ presenceOf(device) }}</span>
          </div>
        </div>
        <button
          class="remove"
          type="button"
          [disabled]="busy()"
          [title]="i18n.t('sync.devices.remove')"
          [attr.aria-label]="i18n.t('sync.devices.remove')"
          (click)="removing.set(device)"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v5"/><path d="M14 11v5"/>
          </svg>
        </button>
      </div>
    } @empty {
      <p class="hint">{{ i18n.t('sync.devices.empty') }}</p>
    }
    @if (error(); as message) {
      <p class="error" role="alert">{{ message }}</p>
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
  styleUrls: ['./sync-screen-ui.scss'],
  styles: [`
    :host { display: flex; flex-direction: column; font-size: 12px; }
    .device { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--daftr-border-subtle); }
    .device:last-of-type { border-bottom: none; }
    .text { flex: 1; min-width: 0; }
    .name { overflow: hidden; color: var(--daftr-text-strong); text-overflow: ellipsis; white-space: nowrap; }
    .meta { display: flex; align-items: center; gap: 6px; margin-top: 1px; font-size: 11px; color: var(--daftr-text-dimmed); }
    .id { font-family: var(--daftr-font-mono); user-select: text; }
    .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--daftr-text-faint); }
    .dot.ok { background: var(--daftr-syntax-comment); }
    .remove {
      display: flex;
      flex: none;
      align-items: center;
      justify-content: center;
      width: 26px;
      height: 26px;
      padding: 0;
      border: none;
      border-radius: 4px;
      background: transparent;
      color: var(--daftr-text-dimmed);
      cursor: pointer;
    }
    .remove:hover:not(:disabled), .remove:focus-visible {
      background: color-mix(in srgb, var(--daftr-danger) 14%, transparent);
      color: var(--daftr-danger);
    }
    .remove:focus-visible { outline: 1px solid var(--daftr-danger); outline-offset: -1px; }
    .remove:disabled { opacity: 0.5; cursor: default; }
    .dot.busy { background: var(--daftr-accent); }
  `],
})
export class SyncDevicesComponent implements OnInit {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);

  protected readonly devices = this.sync.trustedDevices;
  protected readonly removing = signal<TrustedDevice | null>(null);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly removeActions = computed<ConfirmDialogAction[]>(() => [
    { id: 'remove', label: this.i18n.t('sync.devices.remove'), danger: true },
  ]);

  ngOnInit(): void {
    void this.sync.loadDevices();
  }

  protected idOf(device: TrustedDevice): string {
    return shortDeviceId(device.fingerprint);
  }

  protected presenceOf(device: TrustedDevice): string {
    if (device.connected) {
      return this.i18n.t('sync.devices.connected');
    }

    return this.i18n.t(device.present ? 'sync.devices.present' : 'sync.devices.absent');
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
