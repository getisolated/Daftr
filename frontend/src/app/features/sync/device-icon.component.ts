import { ChangeDetectionStrategy, Component, computed, inject, input } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { DeviceKind } from '../../core/models/sync.model';
import { deviceIcon, deviceIconLabel } from '../../core/utils/sync.util';

/** Icon of a device kind: laptop, desktop monitor, or a generic device when the kind is unknown. */
@Component({
  selector: 'app-device-icon',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    role: 'img',
    '[attr.aria-label]': 'label()',
    '[attr.title]': 'label()',
  },
  template: `
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      @switch (icon()) {
        @case ('laptop') {
          <rect x="4" y="5" width="16" height="11" rx="1.5"/>
          <path d="M2 19h20"/>
        }
        @case ('desktop') {
          <rect x="3" y="4" width="18" height="12" rx="1.5"/>
          <path d="M8 20h8M12 16v4"/>
        }
        @case ('device') {
          <rect x="2" y="5" width="13" height="10" rx="1.5"/>
          <path d="M5 19h7M8.5 15v4"/>
          <rect x="17" y="9" width="5" height="10" rx="1"/>
        }
      }
    </svg>
  `,
  styles: [`
    :host { display: inline-flex; flex: none; color: var(--daftr-text-soft); }
  `],
})
export class DeviceIconComponent {
  private readonly i18n = inject(I18nService);

  readonly kind = input.required<DeviceKind>();

  protected readonly icon = computed(() => deviceIcon(this.kind()));
  protected readonly label = computed(() => deviceIconLabel(this.icon(), this.i18n.language()));
}
