import { ChangeDetectionStrategy, Component, computed, inject, output, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { SyncService } from '../../core/services/sync.service';
import { isPairingCode } from '../../core/utils/sync.util';
import { OtpInputComponent } from './otp-input.component';

/** Distinct ids when several forms exist at once. */
let nextFormId = 0;

/**
 * "Enter a code": the code shown by the other device in six boxes, and its address when it is not found on the network.
 * It takes the place of the pairing block of the sync screen.
 */
@Component({
  selector: 'app-sync-join-form',
  standalone: true,
  imports: [OtpInputComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <form class="join" (submit)="join($event)">
      <h3 class="section">{{ i18n.t('sync.join.title') }}</h3>
      <p class="intro">{{ i18n.t('sync.join.intro') }}</p>

      <app-otp-input class="otp" (codeChange)="code.set($event)" />

      <div class="address">
        <label class="label" [attr.for]="addressId">{{ i18n.t('sync.join.address.label') }}</label>
        <input
          class="input"
          type="text"
          spellcheck="false"
          placeholder="192.168.1.20:47820"
          [id]="addressId"
          [value]="address()"
          (input)="address.set($any($event.target).value)"
        />
        <p class="hint">{{ i18n.t('sync.join.address.hint') }}</p>
      </div>

      @if (error(); as message) {
        <p class="error" role="alert">{{ message }}</p>
      }

      <div class="actions">
        <button class="btn connect" type="submit" [disabled]="busy() || !codeComplete()">
          {{ busy() ? i18n.t('sync.pairing.connecting') : i18n.t('sync.pairing.connect') }}
        </button>
        <button class="cancel" type="button" [disabled]="busy()" (click)="cancelled.emit()">{{ i18n.t('sync.join.cancel') }}</button>
      </div>
    </form>
  `,
  styleUrls: ['./sync-screen-ui.scss'],
  styles: [`
    :host { display: block; }
    .join { display: flex; flex-direction: column; gap: 22px; }
    .intro { margin: 0; font-size: 12.5px; line-height: 1.5; color: var(--daftr-text-soft); text-align: center; }
    .otp { margin: 8px 0 10px; }
    .address { display: flex; flex-direction: column; gap: 4px; }
    .actions { display: flex; flex-direction: column; align-items: stretch; gap: 8px; margin-top: 6px; }
    .connect { width: 100%; height: 40px; font-size: 14px; font-weight: 600; }
    .cancel {
      align-self: center;
      padding: 4px 12px;
      border: none;
      background: none;
      color: var(--daftr-text-soft);
      font: inherit;
      font-size: 12.5px;
      cursor: pointer;
    }
    .cancel:hover:not(:disabled) { color: var(--daftr-text); }
    .cancel:disabled { opacity: 0.6; cursor: default; }
  `],
})
export class SyncJoinFormComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);

  /** Linked: the other device is now one of the trusted devices. */
  readonly joined = output<void>();
  readonly cancelled = output<void>();

  protected readonly addressId = `sync-join-address-${nextFormId++}`;
  protected readonly code = signal('');
  protected readonly address = signal('');
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly codeComplete = computed(() => isPairingCode(this.code()));

  protected async join(event: Event): Promise<void> {
    event.preventDefault();
    // An enclosing form (a dialog) must not take this submission for its own
    event.stopPropagation();

    if (!this.codeComplete()) {
      this.error.set(this.i18n.t('sync.error.invalidCode'));
      return;
    }

    const address = this.address().trim() || null;
    this.busy.set(true);
    this.error.set(null);

    try {
      await this.sync.pair(this.code(), address);
      this.joined.emit();
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
