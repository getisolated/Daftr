import { ChangeDetectionStrategy, Component, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { PairingStep, PairingTicket } from '../../core/models/sync.model';
import { ProfilesService } from '../../core/services/profiles.service';
import { SyncService } from '../../core/services/sync.service';
import { profileDisplayName } from '../../core/utils/profile.util';
import {
  formatCountdown, formatPairingCode, initialPairingProfiles, isPairingCode, normalizePairingCode, pairingAddresses, remainingMs, svgDataUrl,
} from '../../core/utils/sync.util';
import { Profile } from '../../core/models/profile.model';
import { CheckboxComponent } from '../../shared/ui/checkbox.component';

/** Refresh of the countdown of the pairing code. */
const COUNTDOWN_TICK_MS = 1000;

/**
 * "Link a device" (profiles to share, then QR code and code with its countdown) and "Link to a device" (the code
 * shown by the other device, and its address when it is not found).
 */
@Component({
  selector: 'app-sync-pairing',
  standalone: true,
  imports: [CheckboxComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @switch (step()) {
      @case ('idle') {
        <div class="row">
          <button class="btn" type="button" (click)="chooseProfiles()">{{ i18n.t('sync.pairing.link') }}</button>
          <button class="btn" type="button" (click)="enterCode()">{{ i18n.t('sync.pairing.join') }}</button>
        </div>
      }
      @case ('chooseProfiles') {
        <div class="label">{{ i18n.t('sync.pairing.chooseProfiles') }}</div>
        <div class="profiles">
          @for (profile of profiles(); track profile.id) {
            <app-checkbox [checked]="isSelected(profile)" (checkedChange)="toggle(profile, $event)">{{ nameOf(profile) }}</app-checkbox>
          }
        </div>
        <div class="row">
          <button class="btn primary" type="button" [disabled]="busy() || selected().length === 0" (click)="showCode()">
            {{ i18n.t('sync.pairing.showCode') }}
          </button>
          <button class="btn" type="button" (click)="reset()">{{ i18n.t('common.cancel') }}</button>
        </div>
      }
      @case ('showCode') {
        @if (liveTicket(); as current) {
          <div class="ticket">
            <img class="qr" [src]="qrUrl()" [alt]="i18n.t('sync.pairing.qrAlt')" width="168" height="168" />
            <div class="ticket-text">
              <div class="hint first">{{ i18n.t('sync.pairing.instructions') }}</div>
              <div class="code" aria-live="polite">{{ formattedCode(current.code) }}</div>
              <div class="hint">{{ i18n.t('sync.pairing.expiresIn', { time: countdown() }) }}</div>
              @if (renewed()) {
                <div class="hint">{{ i18n.t('sync.pairing.renewed') }}</div>
              }
              <div class="hint addresses">{{ addresses() }}</div>
              <div class="row">
                <button class="btn" type="button" (click)="stop()">{{ i18n.t('sync.pairing.stop') }}</button>
              </div>
            </div>
          </div>
        } @else {
          <div class="hint first">{{ i18n.t('sync.pairing.expired') }}</div>
          <div class="row">
            <button class="btn primary" type="button" [disabled]="busy()" (click)="showCode()">{{ i18n.t('sync.pairing.newCode') }}</button>
            <button class="btn" type="button" (click)="reset()">{{ i18n.t('common.close') }}</button>
          </div>
        }
      }
      @case ('enterCode') {
        <form class="join" (submit)="join($event)">
          <label class="label" for="sync-pairing-code">{{ i18n.t('sync.pairing.code') }}</label>
          <input
            id="sync-pairing-code"
            class="field code-field"
            type="text"
            inputmode="numeric"
            autocomplete="one-time-code"
            spellcheck="false"
            placeholder="000000"
            [value]="code()"
            (input)="onCodeInput($event)"
          />
          <label class="label" for="sync-pairing-address">{{ i18n.t('sync.pairing.address') }}</label>
          <input
            id="sync-pairing-address"
            class="field address-field"
            type="text"
            spellcheck="false"
            placeholder="192.168.1.20:47820"
            [value]="address()"
            (input)="address.set($any($event.target).value)"
          />
          <div class="hint">{{ i18n.t('sync.pairing.addressHint') }}</div>
          <div class="row">
            <button class="btn primary" type="submit" [disabled]="busy() || !codeComplete()">
              {{ busy() ? i18n.t('sync.pairing.connecting') : i18n.t('sync.pairing.connect') }}
            </button>
            <button class="btn" type="button" [disabled]="busy()" (click)="reset()">{{ i18n.t('common.cancel') }}</button>
          </div>
        </form>
      }
    }
    @if (error(); as message) {
      <div class="error" role="alert">{{ message }}</div>
    }
  `,
  styleUrls: ['./sync-panel.scss'],
  styles: [`
    .label { margin: 8px 0 4px; color: var(--daftr-text); }
    .label:first-child { margin-top: 0; }
    .profiles { display: flex; flex-direction: column; gap: 6px; margin-bottom: 10px; }
    .ticket { display: flex; flex-wrap: wrap; gap: 16px; align-items: flex-start; }
    .qr { flex: none; padding: 8px; background: #fff; border-radius: 4px; }
    .ticket-text { display: flex; flex-direction: column; min-width: 200px; flex: 1; }
    .ticket-text .row { margin-top: 10px; }
    .hint.first { margin-top: 0; }
    .code {
      margin: 8px 0 2px;
      font-family: var(--daftr-font-mono);
      font-size: 30px;
      letter-spacing: 0.12em;
      color: var(--daftr-text-strong);
      user-select: text;
    }
    .addresses { font-family: var(--daftr-font-mono); font-size: 11px; }
    .join { display: flex; flex-direction: column; align-items: flex-start; }
    .join .row { margin-top: 10px; }
    .code-field { width: 140px; font-family: var(--daftr-font-mono); font-size: 16px; letter-spacing: 0.2em; }
    .address-field { width: 240px; }
  `],
})
export class SyncPairingComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly profilesService = inject(ProfilesService);

  /** An open pairing mode comes back when the settings are shown again. */
  protected readonly step = signal<PairingStep>(this.sync.pairing() ? 'showCode' : 'idle');
  protected readonly selected = signal<string[]>([]);
  protected readonly code = signal('');
  protected readonly address = signal('');
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  private readonly now = signal(Date.now());
  /** Last code shown: when the mode closes, tells an expiry from a linking. */
  private lastTicket: PairingTicket | null = this.sync.pairing();

  protected readonly profiles = this.profilesService.profiles;
  protected readonly ticket = this.sync.pairing;
  protected readonly renewed = this.sync.pairingRenewed;
  protected readonly codeComplete = computed(() => isPairingCode(this.code()));
  protected readonly qrUrl = computed(() => {
    const ticket = this.ticket();
    return ticket ? svgDataUrl(ticket.qrSvg) : '';
  });
  private readonly remaining = computed(() => remainingMs(this.ticket()?.expiresAt ?? 0, this.now()));
  /** The code being shown, `null` once the mode closed or the countdown ran out. */
  protected readonly liveTicket = computed(() => (this.remaining() > 0 ? this.ticket() : null));
  protected readonly countdown = computed(() => formatCountdown(this.remaining()));
  /** Addresses of this device, for "Link to a device" on a network where it is not found. */
  protected readonly addresses = computed(() => pairingAddresses(this.ticket()?.uri ?? ''));

  constructor() {
    effect(onCleanup => {
      if (this.step() !== 'showCode' || !this.ticket()) {
        return;
      }

      const timer = setInterval(() => this.now.set(Date.now()), COUNTDOWN_TICK_MS);
      onCleanup(() => clearInterval(timer));
    });

    // The mode closes by itself: at the first linking (back to the buttons), or once expired (a new code is offered)
    effect(() => {
      const ticket = this.ticket();
      untracked(() => this.onTicketChange(ticket));
    });
  }

  protected nameOf(profile: Profile): string {
    return profileDisplayName(profile, this.i18n.t);
  }

  protected formattedCode(code: string): string {
    return formatPairingCode(code);
  }

  protected isSelected(profile: Profile): boolean {
    return this.selected().includes(profile.id);
  }

  protected toggle(profile: Profile, checked: boolean): void {
    this.selected.update(ids => (checked ? [...ids, profile.id] : ids.filter(id => id !== profile.id)));
  }

  protected chooseProfiles(): void {
    this.error.set(null);
    this.selected.set(initialPairingProfiles(this.profiles(), this.profilesService.activeId()));
    this.step.set('chooseProfiles');
  }

  protected enterCode(): void {
    this.error.set(null);
    this.code.set('');
    this.address.set('');
    this.step.set('enterCode');
  }

  protected async showCode(): Promise<void> {
    if (this.selected().length === 0) {
      this.error.set(this.i18n.t('sync.pairing.noProfile'));
      return;
    }

    const started = await this.run(() => this.sync.startPairing(this.selected()));

    if (started) {
      this.now.set(Date.now());
      this.step.set('showCode');
    }
  }

  protected async stop(): Promise<void> {
    await this.run(() => this.sync.stopPairing());
    this.reset();
  }

  protected reset(): void {
    this.error.set(null);
    this.step.set('idle');
  }

  protected onCodeInput(event: Event): void {
    const field = event.target as HTMLInputElement;
    const code = normalizePairingCode(field.value);
    field.value = code;
    this.code.set(code);
  }

  protected async join(event: Event): Promise<void> {
    event.preventDefault();

    if (!this.codeComplete()) {
      this.error.set(this.i18n.t('sync.error.invalidCode'));
      return;
    }

    const address = this.address().trim() || null;

    if (await this.run(() => this.sync.pair(this.code(), address))) {
      this.reset();
    }
  }

  private onTicketChange(ticket: PairingTicket | null): void {
    const previous = this.lastTicket;
    this.lastTicket = ticket ?? previous;

    if (ticket || !previous || this.step() !== 'showCode') {
      return;
    }

    // Closed well before its end: linked (or stopped elsewhere), nothing more to show
    if (Date.now() < previous.expiresAt - COUNTDOWN_TICK_MS) {
      this.reset();
    }
  }

  private async run(action: () => Promise<void>): Promise<boolean> {
    this.busy.set(true);
    this.error.set(null);

    try {
      await action();
      return true;
    } catch (error) {
      this.error.set(this.sync.errorMessage(error));
      return false;
    } finally {
      this.busy.set(false);
    }
  }
}
