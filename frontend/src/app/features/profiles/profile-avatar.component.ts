import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { profileInitial } from '../../core/utils/profile.util';

/** Round icon of a profile: its image, else the initial of its name, else a generic silhouette. */
@Component({
  selector: 'app-profile-avatar',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    'aria-hidden': 'true',
    '[style.--avatar-size.px]': 'size()',
  },
  template: `
    @if (iconUrl(); as url) {
      <img class="image" [src]="url" alt="" draggable="false" />
    } @else if (initial(); as letter) {
      <span class="initial">{{ letter }}</span>
    } @else {
      <svg class="generic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>
      </svg>
    }
  `,
  styles: [`
    :host {
      display: inline-flex;
      flex-shrink: 0;
      align-items: center;
      justify-content: center;
      width: var(--avatar-size);
      height: var(--avatar-size);
      border-radius: 50%;
      overflow: hidden;
      background: var(--daftr-bg-active);
      color: var(--daftr-text-soft);
      box-shadow: inset 0 0 0 1px var(--daftr-border-soft);
    }
    .image { width: 100%; height: 100%; object-fit: cover; }
    .initial {
      font-size: calc(var(--avatar-size) * 0.5);
      font-weight: 600;
      line-height: 1;
      color: var(--daftr-text-strong);
    }
    .generic { width: 62%; height: 62%; }
  `],
})
export class ProfileAvatarComponent {
  readonly name = input<string | null>(null);
  readonly iconUrl = input<string | null>(null);
  readonly size = input(20);

  protected readonly initial = computed(() => profileInitial(this.name()));
}
