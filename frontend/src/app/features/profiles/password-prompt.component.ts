import { ChangeDetectionStrategy, Component, inject, input, output, signal } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';
import { FocusSelectDirective } from '../../shared/ui/focus-select.directive';

/** Asks the password of a protected profile (to open or delete it); closing it (Escape, backdrop) emits `closed`. */
@Component({
  selector: 'app-password-prompt',
  standalone: true,
  imports: [FocusSelectDirective],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'closed.emit()',
  },
  template: `
    <div class="backdrop" (click)="closed.emit()"></div>
    <form class="dialog" role="dialog" aria-modal="true" [attr.aria-label]="title()" (submit)="submit($event)">
      <h2 class="title">{{ title() }}</h2>
      <p class="message">{{ message() }}</p>
      <label class="field">
        {{ i18n.t('profiles.prompt.password') }}
        <input
          class="input"
          type="password"
          appFocusSelect
          autocomplete="current-password"
          [value]="password()"
          (input)="password.set($any($event.target).value)"
        />
      </label>
      @if (error(); as message) {
        <p class="error" role="alert">{{ message }}</p>
      }
      <div class="actions">
        <button class="btn" type="submit" [class.danger]="danger()" [disabled]="busy() || !password()">{{ confirmLabel() }}</button>
        <button class="btn secondary" type="button" (click)="closed.emit()">{{ i18n.t('common.cancel') }}</button>
      </div>
    </form>
  `,
  styleUrls: ['./profile-dialog.scss'],
})
export class PasswordPromptComponent {
  protected readonly i18n = inject(I18nService);

  readonly title = input.required<string>();
  readonly message = input.required<string>();
  readonly confirmLabel = input.required<string>();
  readonly danger = input(false);
  readonly busy = input(false);
  /** Shown under the field (wrong password…). */
  readonly error = input<string | null>(null);
  readonly submitted = output<string>();
  readonly closed = output<void>();

  protected readonly password = signal('');

  protected submit(event: Event): void {
    event.preventDefault();

    if (this.password()) {
      this.submitted.emit(this.password());
    }
  }
}
