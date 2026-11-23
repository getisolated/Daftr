import { ChangeDetectionStrategy, Component, computed, inject, output, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { SyncService } from '../../core/services/sync.service';
import { isPairingCode, normalizePairingCode } from '../../core/utils/sync.util';

/** Distinct ids when several forms exist at once (sync settings and profile settings). */
let nextFormId = 0;

/**
 * "Link to a device": the code shown by the other device, and its address when it is not found on the network. Used by
 * the sync settings and by the devices of a profile.
 */
@Component({
  selector: 'app-sync-join-form',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <form class="join" (submit)="join($event)">
      <label class="label" [attr.for]="ids.code">{{ i18n.t('sync.pairing.code') }}</label>
      <input
        class="field code-field"
        type="text"
        inputmode="numeric"
        autocomplete="one-time-code"
        spellcheck="false"
        placeholder="000000"
        [id]="ids.code"
        [value]="code()"
        (input)="onCodeInput($event)"
      />
      <label class="label" [attr.for]="ids.address">{{ i18n.t('sync.pairing.address') }}</label>
      <input
        class="field address-field"
        type="text"
        spellcheck="false"
        placeholder="192.168.1.20:47820"
        [id]="ids.address"
        [value]="address()"
        (input)="address.set($any($event.target).value)"
      />
      <div class="hint">{{ i18n.t('sync.pairing.addressHint') }}</div>
      <div class="row">
        <button class="btn primary" type="submit" [disabled]="busy() || !codeComplete()">
          {{ busy() ? i18n.t('sync.pairing.connecting') : i18n.t('sync.pairing.connect') }}
        </button>
        <button class="btn" type="button" [disabled]="busy()" (click)="cancelled.emit()">{{ i18n.t('common.cancel') }}</button>
      </div>
    </form>
    @if (error(); as message) {
      <div class="error" role="alert">{{ message }}</div>
    }
  `,
  styleUrls: ['./sync-panel.scss'],
  styles: [`
    .label { margin: 8px 0 4px; color: var(--daftr-text); }
    .label:first-child { margin-top: 0; }
    .join { display: flex; flex-direction: column; align-items: flex-start; }
    .join .row { margin-top: 10px; }
    .code-field { width: 140px; font-family: var(--daftr-font-mono); font-size: 16px; letter-spacing: 0.2em; }
    .address-field { width: 240px; max-width: 100%; }
  `],
})
export class SyncJoinFormComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);

  /** Linked: the profiles the other device proposes open their choice. */
  readonly joined = output<void>();
  readonly cancelled = output<void>();

  protected readonly ids = { code: `sync-join-code-${nextFormId}`, address: `sync-join-address-${nextFormId++}` };
  protected readonly code = signal('');
  protected readonly address = signal('');
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly codeComplete = computed(() => isPairingCode(this.code()));

  protected onCodeInput(event: Event): void {
    const field = event.target as HTMLInputElement;
    const code = normalizePairingCode(field.value);
    field.value = code;
    this.code.set(code);
  }

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
