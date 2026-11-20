import { AfterViewInit, ChangeDetectionStrategy, Component, ElementRef, inject, input, output } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { Profile } from '../../core/models/profile.model';
import { ProfilesService } from '../../core/services/profiles.service';
import { profileDisplayName } from '../../core/utils/profile.util';
import { ProfileAvatarComponent } from './profile-avatar.component';

/** Menu of the profiles, opened on the right of the sidebar item: switch, create, configure. */
@Component({
  selector: 'app-profile-menu',
  standalone: true,
  imports: [ProfileAvatarComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    role: 'menu',
    '[attr.aria-label]': "i18n.t('profiles.menu.label')",
    '(keydown.arrowDown)': 'moveFocus($event, 1)',
    '(keydown.arrowUp)': 'moveFocus($event, -1)',
  },
  templateUrl: './profile-menu.component.html',
  styleUrls: ['./profile-menu.component.scss'],
})
export class ProfileMenuComponent implements AfterViewInit {
  protected readonly i18n = inject(I18nService);
  protected readonly profiles = inject(ProfilesService);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);

  /** Opened from the keyboard: the first entry takes the focus. */
  readonly focusFirst = input(false);
  readonly picked = output<Profile>();
  readonly created = output<void>();
  readonly configured = output<void>();

  ngAfterViewInit(): void {
    if (this.focusFirst()) {
      this.items()[0]?.focus();
    }
  }

  protected label(profile: Profile): string {
    return profileDisplayName(profile, this.i18n.t);
  }

  protected moveFocus(event: Event, step: number): void {
    event.preventDefault();

    const items = this.items();
    const current = items.indexOf(document.activeElement as HTMLElement);
    const next = (current + step + items.length) % items.length;

    items[next]?.focus();
  }

  private items(): HTMLElement[] {
    return [...this.host.nativeElement.querySelectorAll<HTMLElement>('[role="menuitem"], [role="menuitemradio"]')];
  }
}
