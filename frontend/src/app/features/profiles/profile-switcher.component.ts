import { ChangeDetectionStrategy, Component, ElementRef, OnDestroy, computed, inject, signal, viewChild } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { Profile, ProfileSwitcherDialog } from '../../core/models/profile.model';
import { ProfileTransferService } from '../../core/services/profile-transfer.service';
import { ProfilesService } from '../../core/services/profiles.service';
import { HoverIntent, MenuPosition, profileMenuPosition } from '../../core/utils/profile-menu.util';
import { profileDisplayName, profileErrorMessage } from '../../core/utils/profile.util';
import { ConfirmDialogAction, ConfirmDialogComponent } from '../../shared/ui/confirm-dialog.component';
import { PasswordPromptComponent } from './password-prompt.component';
import { ProfileAvatarComponent } from './profile-avatar.component';
import { ProfileMenuComponent } from './profile-menu.component';
import { ProfileSettingsDialogComponent } from './profile-settings-dialog.component';

/** Width of the profile menu, in pixels (also its CSS width). */
const MENU_WIDTH = 240;

/**
 * Item of the sidebar footer showing the active profile. Hovering it (or clicking it, or the keyboard) opens the menu
 * of the profiles on its right; this component also owns the dialogs that menu opens.
 */
@Component({
  selector: 'app-profile-switcher',
  standalone: true,
  imports: [ProfileAvatarComponent, ProfileMenuComponent, ProfileSettingsDialogComponent, PasswordPromptComponent, ConfirmDialogComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:mousedown)': 'onDocumentPointerDown($event)',
    '(keydown.escape)': 'closeMenu(true)',
  },
  templateUrl: './profile-switcher.component.html',
  styleUrls: ['./profile-switcher.component.scss'],
})
export class ProfileSwitcherComponent implements OnDestroy {
  protected readonly i18n = inject(I18nService);
  protected readonly profiles = inject(ProfilesService);
  protected readonly transfer = inject(ProfileTransferService);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  private readonly trigger = viewChild.required<ElementRef<HTMLButtonElement>>('trigger');

  protected readonly menuWidth = MENU_WIDTH;
  protected readonly menuPosition = signal<MenuPosition | null>(null);
  protected readonly focusMenu = signal(false);
  protected readonly dialog = signal<ProfileSwitcherDialog | null>(null);
  protected readonly busy = signal(false);
  protected readonly dialogError = signal<string | null>(null);
  protected readonly hover = new HoverIntent(open => (open ? this.openMenu(false) : this.closeMenu(false)));

  protected readonly activeName = computed(() => {
    const active = this.profiles.active();
    return active ? profileDisplayName(active, this.i18n.t) : this.i18n.t('profiles.defaultName');
  });

  protected readonly deleteActions = computed<ConfirmDialogAction[]>(() => [
    { id: 'delete', label: this.i18n.t('profiles.delete.confirm'), danger: true },
  ]);

  ngOnDestroy(): void {
    this.hover.cancel();
  }

  /** Message of a deletion; the active profile also tells what takes its place. */
  protected deleteMessage(profile: Profile, key: 'profiles.delete.message' | 'profiles.delete.protectedMessage'): string {
    const message = this.i18n.t(key);

    return profile.id === this.profiles.activeId() ? `${message} ${this.i18n.t('profiles.delete.activeNote')}` : message;
  }

  protected label(profile: Profile): string {
    return profileDisplayName(profile, this.i18n.t);
  }

  /**
   * A click opens the menu (the hover may already have: it stays open). From the keyboard (Enter, Space: no pointer
   * detail), its first entry takes the focus; a second activation closes it.
   */
  protected onTriggerClick(event: MouseEvent): void {
    const fromKeyboard = event.detail === 0;
    this.hover.cancel();

    if (!this.menuPosition()) {
      this.openMenu(fromKeyboard);
    } else if (fromKeyboard) {
      this.closeMenu(true);
    }
  }

  protected openMenu(fromKeyboard: boolean): void {
    if (this.dialog()) {
      return;
    }

    const anchor = this.trigger().nativeElement.getBoundingClientRect();
    const viewport = { width: window.innerWidth, height: window.innerHeight };
    this.focusMenu.set(fromKeyboard);
    this.menuPosition.set(profileMenuPosition(anchor, viewport, MENU_WIDTH));
  }

  protected closeMenu(refocus: boolean): void {
    if (!this.menuPosition()) {
      return;
    }

    this.hover.cancel();
    this.menuPosition.set(null);

    if (refocus) {
      this.trigger().nativeElement.focus();
    }
  }

  protected onDocumentPointerDown(event: MouseEvent): void {
    if (this.menuPosition() && event.target instanceof Node && !this.host.nativeElement.contains(event.target)) {
      this.closeMenu(false);
    }
  }

  protected async onPicked(profile: Profile): Promise<void> {
    this.closeMenu(false);

    if (profile.id === this.profiles.activeId()) {
      return;
    }

    if (profile.protected) {
      this.openDialog({ kind: 'password', purpose: 'switch', profile });
      return;
    }

    await this.run(() => this.profiles.switchTo(profile.id, null));
  }

  protected onCreate(): void {
    this.closeMenu(false);
    this.openDialog({ kind: 'settings', profile: null });
  }

  protected onConfigure(): void {
    this.closeMenu(false);
    this.openDialog({ kind: 'settings', profile: this.profiles.active() });
  }

  protected onRemove(profile: Profile): void {
    this.closeMenu(false);
    this.openDialog(profile.protected ? { kind: 'password', purpose: 'delete', profile } : { kind: 'confirmDelete', profile });
  }

  protected async onPassword(password: string): Promise<void> {
    const dialog = this.dialog();

    if (dialog?.kind !== 'password') {
      return;
    }

    const action = dialog.purpose === 'switch'
      ? (): Promise<void> => this.profiles.switchTo(dialog.profile.id, password)
      : (): Promise<void> => this.profiles.delete(dialog.profile.id, password);

    if (await this.run(action)) {
      this.dialog.set(null);
    }
  }

  protected async onDeleteConfirmed(choice: string | null): Promise<void> {
    const dialog = this.dialog();
    this.dialog.set(null);

    if (choice === 'delete' && dialog?.kind === 'confirmDelete') {
      await this.run(() => this.profiles.delete(dialog.profile.id, null));
    }
  }

  protected closeDialog(): void {
    this.dialog.set(null);
    this.dialogError.set(null);
  }

  private openDialog(dialog: ProfileSwitcherDialog): void {
    this.dialogError.set(null);
    this.dialog.set(dialog);
  }

  /** Runs a profile command; a failure is shown in the dialog (or alone when none is open). */
  private async run(action: () => Promise<void>): Promise<boolean> {
    this.busy.set(true);
    this.dialogError.set(null);

    try {
      await action();
      return true;
    } catch (error) {
      this.dialogError.set(profileErrorMessage(error, this.i18n.t));
      return false;
    } finally {
      this.busy.set(false);
    }
  }
}
