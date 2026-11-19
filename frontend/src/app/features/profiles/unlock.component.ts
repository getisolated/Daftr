import { ChangeDetectionStrategy, Component, OnInit, computed, inject, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { Profile } from '../../core/models/profile.model';
import { DesktopBridgeService } from '../../core/services/desktop-bridge.service';
import { PlatformService } from '../../core/services/platform.service';
import { ProfilesService } from '../../core/services/profiles.service';
import { ThemeService } from '../../core/services/theme.service';
import { profileDisplayName, profileErrorMessage } from '../../core/utils/profile.util';
import { FocusSelectDirective } from '../../shared/ui/focus-select.directive';
import { ProfileAvatarComponent } from './profile-avatar.component';

/**
 * Window content at launch when the last profile is protected: nothing of its data is read before its password. Another
 * profile can be opened from here instead.
 */
@Component({
  selector: 'app-unlock',
  standalone: true,
  imports: [ProfileAvatarComponent, FocusSelectDirective],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './unlock.component.html',
  styleUrls: ['./unlock.component.scss'],
})
export class UnlockComponent implements OnInit {
  protected readonly i18n = inject(I18nService);
  protected readonly profiles = inject(ProfilesService);
  protected readonly window = inject(DesktopBridgeService);
  protected readonly platform = inject(PlatformService);
  // Injected eagerly: it applies the theme of the settings (shared by every profile)
  private readonly theme = inject(ThemeService);

  /** Profile picked in the list; the locked active one until then. */
  private readonly picked = signal<Profile | null>(null);
  protected readonly target = computed(() => this.picked() ?? this.profiles.active());
  protected readonly others = computed(() => this.profiles.profiles().filter(profile => profile.id !== this.target()?.id));
  protected readonly password = signal('');
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);

  async ngOnInit(): Promise<void> {
    await this.profiles.load();
  }

  protected label(profile: Profile | null): string {
    return profile ? profileDisplayName(profile, this.i18n.t) : this.i18n.t('profiles.defaultName');
  }

  /** An unprotected profile opens right away; a protected one asks its own password here. */
  protected async pick(profile: Profile): Promise<void> {
    this.picked.set(profile);
    this.password.set('');
    this.error.set(null);

    if (!profile.protected) {
      await this.open(null);
    }
  }

  protected async submit(event: Event): Promise<void> {
    event.preventDefault();

    if (this.password()) {
      await this.open(this.password());
    }
  }

  private async open(password: string | null): Promise<void> {
    const target = this.target();

    if (!target) {
      return;
    }

    this.busy.set(true);
    this.error.set(null);

    try {
      if (target.id === this.profiles.activeId() && password !== null) {
        await this.profiles.unlock(password);
      } else {
        await this.profiles.switchTo(target.id, password);
      }
    } catch (error) {
      this.error.set(profileErrorMessage(error, this.i18n.t));
      this.busy.set(false);
    }
  }
}
