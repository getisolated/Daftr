import { ChangeDetectionStrategy, Component, computed, effect, inject, signal, untracked } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { PairingStep, PairingTicket } from '../../core/models/sync.model';
import { ProfilesService } from '../../core/services/profiles.service';
import { SyncService } from '../../core/services/sync.service';
import { profileDisplayName } from '../../core/utils/profile.util';
import { formatPairingCode, initialPairingProfiles, pairingAddresses, svgDataUrl } from '../../core/utils/sync.util';
import { Profile } from '../../core/models/profile.model';
import { CheckboxComponent } from '../../shared/ui/checkbox.component';
import { COUNTDOWN_TICK_MS, pairingCountdown } from './pairing-countdown';
import { SyncJoinFormComponent } from './sync-join-form.component';

/**
 * "Link a device" (profiles to share, then QR code and code with its countdown) and "Link to a device" (the code
 * shown by the other device, and its address when it is not found).
 */
@Component({
  selector: 'app-sync-pairing',
  standalone: true,
  imports: [CheckboxComponent, SyncJoinFormComponent],
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
        <app-sync-join-form (joined)="reset()" (cancelled)="reset()" />
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
  `],
})
export class SyncPairingComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly profilesService = inject(ProfilesService);

  /** The code of the pairing mode opened here; another screen (profile settings) may have replaced it. */
  protected readonly ticket = computed(() => (this.sync.pairingOwner() === 'settings' ? this.sync.pairing() : null));
  /** An open pairing mode comes back when the settings are shown again. */
  protected readonly step = signal<PairingStep>(this.ticket() ? 'showCode' : 'idle');
  protected readonly selected = signal<string[]>([]);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  /** Last code shown: when the mode closes, tells an expiry from a linking. */
  private lastTicket: PairingTicket | null = this.ticket();

  protected readonly profiles = this.profilesService.profiles;
  protected readonly renewed = this.sync.pairingRenewed;
  protected readonly qrUrl = computed(() => {
    const ticket = this.ticket();
    return ticket ? svgDataUrl(ticket.qrSvg) : '';
  });
  private readonly timer = pairingCountdown(this.ticket, () => this.step() === 'showCode');
  /** The code being shown, `null` once the mode closed or the countdown ran out. */
  protected readonly liveTicket = computed(() => (this.timer.remaining() > 0 ? this.ticket() : null));
  protected readonly countdown = this.timer.label;
  /** Addresses of this device, for "Link to a device" on a network where it is not found. */
  protected readonly addresses = computed(() => pairingAddresses(this.ticket()?.uri ?? ''));

  constructor() {
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
    this.step.set('enterCode');
  }

  protected async showCode(): Promise<void> {
    if (this.selected().length === 0) {
      this.error.set(this.i18n.t('sync.pairing.noProfile'));
      return;
    }

    const started = await this.run(() => this.sync.startPairing(this.selected(), 'settings'));

    if (started) {
      this.step.set('showCode');
    }
  }

  protected async stop(): Promise<void> {
    await this.run(() => this.sync.stopPairing('settings'));
    this.reset();
  }

  protected reset(): void {
    this.error.set(null);
    this.step.set('idle');
  }

  private onTicketChange(ticket: PairingTicket | null): void {
    const previous = this.lastTicket;
    this.lastTicket = ticket ?? previous;

    if (ticket || !previous || this.step() !== 'showCode') {
      return;
    }

    // Closed well before its end: linked (or stopped, or taken by another screen), nothing more to show
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
