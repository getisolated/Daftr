import { ChangeDetectionStrategy, Component, ElementRef, inject } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { SyncService } from '../../core/services/sync.service';
import { SyncDevicesComponent } from './sync-devices.component';
import { SyncPairingPanelComponent } from './sync-pairing-panel.component';
import { SyncShareComponent } from './sync-share.component';

/**
 * The "Synchronization" screen, a wide dialog: on the left what is shared (profile and collections against each trusted
 * device) then the devices, on the right "Link a device". It opens from the profile menu, the "to set up" row of the collections and
 * the settings; opening it starts the pairing mode and closing it stops it.
 */
@Component({
  selector: 'app-sync-screen',
  standalone: true,
  imports: [SyncShareComponent, SyncDevicesComponent, SyncPairingPanelComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'onEscape()',
  },
  template: `
    <div class="backdrop" (click)="close()"></div>
    <div class="dialog" role="dialog" aria-modal="true" [attr.aria-label]="i18n.t('sync.screen.title')">
      <header class="header">
        <h2 class="title">{{ i18n.t('sync.screen.title') }}</h2>
        @if (indicator(); as shown) {
          <span class="status" [title]="shown.reason ?? ''">
            <span class="dot" [class]="shown.tone"></span>
            {{ shown.label }}
          </span>
        }
        <button class="close" type="button" [attr.aria-label]="i18n.t('sync.screen.close')" [title]="i18n.t('sync.screen.close')" (click)="close()">✕</button>
      </header>
      <div class="columns">
        <div class="left">
          <app-sync-share />
          <section class="devices">
            <h3 class="section">{{ i18n.t('sync.screen.devices') }}</h3>
            <app-sync-devices />
          </section>
        </div>
        <aside class="right">
          <app-sync-pairing-panel />
        </aside>
      </div>
    </div>
  `,
  styleUrls: ['../profiles/profile-dialog.scss', './sync-screen-ui.scss', './sync-screen.component.scss'],
})
export class SyncScreenComponent {
  protected readonly i18n = inject(I18nService);
  private readonly sync = inject(SyncService);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);

  protected readonly indicator = this.sync.indicator;

  protected close(): void {
    this.sync.closeScreen();
  }

  /** A confirmation opened over the screen (removing a device) takes the key: it closes alone. */
  protected onEscape(): void {
    if (!this.host.nativeElement.querySelector('app-confirm-dialog')) {
      this.close();
    }
  }
}
