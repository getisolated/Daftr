import { ChangeDetectionStrategy, Component, computed, effect, inject, input, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { ProfileSync, SpaceDevice, TrustedDevice } from '../../core/models/sync.model';
import { SyncService } from '../../core/services/sync.service';
import { offerTargets, sharedDevices } from '../../core/utils/sync.util';
import { CheckboxComponent } from '../../shared/ui/checkbox.component';
import { ConfirmDialogAction, ConfirmDialogComponent } from '../../shared/ui/confirm-dialog.component';

/**
 * Sync of one profile, in its settings dialog: on or off, the devices it is synchronized with ("Stop with…"), and
 * "Sync with…" a trusted device. Every action applies at once (not on "Save").
 */
@Component({
  selector: 'app-profile-sync',
  standalone: true,
  imports: [CheckboxComponent, ConfirmDialogComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="heading">{{ i18n.t('sync.profile.title') }}</div>
    <app-checkbox [checked]="enabled()" (checkedChange)="setEnabled($event)">{{ i18n.t('sync.profile.enabled') }}</app-checkbox>

    @if (enabled()) {
      <div class="shared">
        @if (shared().length > 0) {
          <span class="hint">{{ i18n.t('sync.profile.sharedWith') }}</span>
          @for (device of shared(); track device.key) {
            <div class="device">
              <span class="name">{{ device.name }}</span>
              <button class="link" type="button" [disabled]="busy()" (click)="stopping.set(device)">
                {{ i18n.t('sync.profile.stopWith', { name: device.name }) }}
              </button>
            </div>
          }
        } @else {
          <span class="hint">{{ i18n.t('sync.profile.sharedWithNobody') }}</span>
        }
      </div>

      @if (targets().length > 0) {
        <button class="link" type="button" [attr.aria-expanded]="offering()" (click)="offering.set(!offering())">{{ i18n.t('sync.profile.offer') }}</button>
        @if (offering()) {
          <div class="targets">
            @for (device of targets(); track device.key) {
              <button class="target" type="button" [disabled]="busy()" (click)="offer(device)">{{ device.name }}</button>
            }
          </div>
        }
      } @else if (trusted().length === 0) {
        <span class="hint">{{ i18n.t('sync.profile.noDevice') }}</span>
      }
    }

    @if (notice(); as message) {
      <span class="hint">{{ message }}</span>
    }
    @if (error(); as message) {
      <span class="error" role="alert">{{ message }}</span>
    }

    @if (stopping(); as device) {
      <app-confirm-dialog
        [title]="i18n.t('sync.profile.stopTitle', { name: device.name })"
        [message]="i18n.t('sync.profile.stopMessage', { name: device.name })"
        [actions]="stopActions()"
        (chosen)="onStopChosen(device, $event)"
      />
    }
  `,
  styles: [`
    :host {
      display: flex;
      flex-direction: column;
      align-items: flex-start;
      gap: 6px;
      padding-top: 8px;
      border-top: 1px solid var(--daftr-border-soft);
      font-size: 12px;
    }
    .heading { color: var(--daftr-text); }
    .shared { display: flex; flex-direction: column; gap: 4px; width: 100%; }
    .device { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
    .name { color: var(--daftr-text-strong); }
    .hint { color: var(--daftr-text-dimmed); font-size: 11.5px; line-height: 1.45; }
    .error { color: var(--daftr-danger); }
    .link {
      padding: 0;
      border: none;
      background: none;
      color: var(--daftr-accent-text);
      font: inherit;
      cursor: pointer;
    }
    .link:hover:not(:disabled) { text-decoration: underline; }
    .link:disabled { opacity: 0.5; cursor: default; }
    .targets { display: flex; flex-wrap: wrap; gap: 6px; }
    .target {
      padding: 3px 10px;
      border: 1px solid var(--daftr-border);
      border-radius: 4px;
      background: transparent;
      color: var(--daftr-text);
      font: inherit;
      cursor: pointer;
    }
    .target:hover:not(:disabled) { border-color: var(--daftr-accent); }
  `],
})
export class ProfileSyncComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);

  readonly profileId = input.required<string>();

  private readonly profileSync = signal<ProfileSync | null>(null);
  protected readonly offering = signal(false);
  protected readonly stopping = signal<SpaceDevice | null>(null);
  protected readonly busy = signal(false);
  protected readonly notice = signal<string | null>(null);
  protected readonly error = signal<string | null>(null);

  protected readonly enabled = computed(() => this.profileSync()?.enabled ?? false);
  protected readonly shared = computed(() => sharedDevices(this.profileSync()));
  protected readonly trusted = this.sync.trustedDevices;
  protected readonly targets = computed(() => offerTargets(this.trusted(), this.profileSync()));
  protected readonly stopActions = computed<ConfirmDialogAction[]>(() => [
    { id: 'stop', label: this.i18n.t('sync.profile.stop'), danger: true },
  ]);

  constructor() {
    void this.sync.loadDevices();
    effect(() => {
      const profileId = this.profileId();
      untracked(() => { void this.load(profileId); });
    });
  }

  protected async setEnabled(enabled: boolean): Promise<void> {
    await this.run(() => this.sync.setProfileEnabled(this.profileId(), enabled));
  }

  protected async offer(device: TrustedDevice): Promise<void> {
    const offered = await this.run(() => this.sync.offerProfile(this.profileId(), device.key));

    if (offered) {
      this.offering.set(false);
      this.notice.set(this.i18n.t('sync.profile.offerSent', { name: device.name }));
    }
  }

  protected async onStopChosen(device: SpaceDevice, choice: string | null): Promise<void> {
    this.stopping.set(null);

    if (choice === 'stop') {
      await this.run(() => this.sync.stopSharing(this.profileId(), device.key));
    }
  }

  private async load(profileId: string): Promise<void> {
    try {
      const loaded = await this.sync.profileSync(profileId);

      if (profileId === this.profileId()) {
        this.profileSync.set(loaded);
      }
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
    }
  }

  /** Runs an action, then reads the sync of the profile again. */
  private async run(action: () => Promise<void>): Promise<boolean> {
    this.busy.set(true);
    this.error.set(null);
    this.notice.set(null);

    try {
      await action();
      return true;
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
      return false;
    } finally {
      await this.load(this.profileId());
      this.busy.set(false);
    }
  }
}
