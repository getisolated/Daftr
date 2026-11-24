import { ChangeDetectionStrategy, Component, OnDestroy, computed, inject, input, linkedSignal, output, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { Profile, ProfileDraft, ProfileIconChange } from '../../core/models/profile.model';
import { ProfilesService } from '../../core/services/profiles.service';
import { renderProfileIcon } from '../../core/utils/profile-icon.util';
import { passwordChange, profileErrorMessage } from '../../core/utils/profile.util';
import { CheckboxComponent } from '../../shared/ui/checkbox.component';
import { FocusSelectDirective } from '../../shared/ui/focus-select.directive';
import { ProfileDevicesComponent } from '../sync/profile-devices.component';
import { ProfileAvatarComponent } from './profile-avatar.component';

/**
 * Settings of a profile: name, icon (cropped to a square and resized here), master password, and its devices in a
 * sub-view that replaces the content (the typed fields are kept for the way back). Without `profile` it creates a new
 * one and switches to it once saved.
 */
@Component({
  selector: 'app-profile-settings-dialog',
  standalone: true,
  imports: [ProfileAvatarComponent, CheckboxComponent, FocusSelectDirective, ProfileDevicesComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'onEscape()',
  },
  templateUrl: './profile-settings-dialog.component.html',
  styleUrls: ['./profile-dialog.scss', './profile-settings-dialog.component.scss'],
})
export class ProfileSettingsDialogComponent implements OnDestroy {
  protected readonly i18n = inject(I18nService);
  private readonly profiles = inject(ProfilesService);

  /** The profile to edit; `null` to create one. */
  readonly profile = input<Profile | null>(null);
  readonly currentIconUrl = input<string | null>(null);
  /** Opens on "Devices and sync" instead of the main view. */
  readonly openDevices = input(false);
  readonly closed = output<void>();
  /** Asked to delete the edited profile; the owner runs the confirmation. */
  readonly deleteRequested = output<void>();

  /** Starts from the edited profile, then follows what the user types. */
  protected readonly name = linkedSignal(() => this.profile()?.name ?? '');
  protected readonly icon = signal<ProfileIconChange>({ kind: 'keep' });
  private readonly previewUrl = signal<string | null>(null);
  protected readonly currentPassword = signal('');
  protected readonly newPassword = signal('');
  protected readonly confirmPassword = signal('');
  protected readonly removePassword = signal(false);
  protected readonly passwordOpen = signal(false);
  /** "Devices and sync" replaces the content; its actions apply at once, without Save. */
  protected readonly devicesOpen = linkedSignal(() => this.openDevices());
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);

  protected readonly creating = computed(() => this.profile() === null);
  protected readonly isProtected = computed(() => this.profile()?.protected ?? false);
  protected readonly shownIconUrl = computed(() => {
    const icon = this.icon();

    if (icon.kind === 'remove') {
      return null;
    }

    return icon.kind === 'set' ? this.previewUrl() : this.currentIconUrl();
  });

  ngOnDestroy(): void {
    this.revokePreview();
  }

  /** The sub-view handles Escape itself (back to this view). */
  protected onEscape(): void {
    if (!this.devicesOpen()) {
      this.closed.emit();
    }
  }

  protected async onIconPicked(event: Event): Promise<void> {
    const field = event.target as HTMLInputElement;
    const file = field.files?.[0];
    field.value = '';

    if (!file) {
      return;
    }

    try {
      const image = await renderProfileIcon(file);
      this.revokePreview();
      this.previewUrl.set(URL.createObjectURL(image));
      this.icon.set({ kind: 'set', image });
      this.error.set(null);
    } catch {
      this.error.set(this.i18n.t('profiles.settings.iconError'));
    }
  }

  protected removeIcon(): void {
    this.revokePreview();
    this.icon.set({ kind: 'remove' });
  }

  protected async save(event: Event): Promise<void> {
    event.preventDefault();

    const password = passwordChange(this.isProtected(), {
      current: this.currentPassword(),
      next: this.newPassword(),
      confirm: this.confirmPassword(),
      remove: this.removePassword(),
    });

    if (typeof password === 'string') {
      this.error.set(this.i18n.t(password));
      return;
    }

    await this.apply({ name: this.name(), icon: this.icon(), password });
  }

  private async apply(draft: ProfileDraft): Promise<void> {
    const profile = this.profile();
    this.busy.set(true);
    this.error.set(null);

    try {
      if (profile) {
        await this.profiles.update(profile.id, draft);
        this.closed.emit();
      } else {
        // Switching reloads the window: the dialog goes with it
        await this.profiles.create(draft);
      }
    } catch (error) {
      this.error.set(profileErrorMessage(error, this.i18n.t));
    } finally {
      this.busy.set(false);
    }
  }

  private revokePreview(): void {
    const url = this.previewUrl();

    if (url) {
      URL.revokeObjectURL(url);
      this.previewUrl.set(null);
    }
  }
}
