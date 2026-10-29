import { AfterViewInit, ChangeDetectionStrategy, Component, ElementRef, inject, input, output, viewChildren } from '@angular/core';
import { I18nService } from '../../core/i18n/i18n.service';

export interface ConfirmDialogAction {
  id: string;
  label: string;
  danger?: boolean;
}

/** Modal asking the user to pick one action; closing it (Escape, backdrop) emits `null`, i.e. "cancel". */
@Component({
  selector: 'app-confirm-dialog',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'chosen.emit(null)',
  },
  template: `
    <div class="backdrop" (click)="chosen.emit(null)"></div>
    <div class="dialog" role="alertdialog" aria-modal="true" [attr.aria-label]="title()">
      <h2 class="title">{{ title() }}</h2>
      <p class="message">{{ message() }}</p>
      <div class="actions">
        @for (action of actions(); track action.id) {
          <button #action class="btn" [class.danger]="action.danger" (click)="chosen.emit(action.id)">{{ action.label }}</button>
        }
        <button class="btn cancel" (click)="chosen.emit(null)">{{ cancelLabel() ?? i18n.t('common.cancel') }}</button>
      </div>
    </div>
  `,
  styleUrls: ['./confirm-dialog.component.scss'],
})
export class ConfirmDialogComponent implements AfterViewInit {
  protected readonly i18n = inject(I18nService);
  private readonly actionButtons = viewChildren<ElementRef<HTMLButtonElement>>('action');

  readonly title = input.required<string>();
  readonly message = input.required<string>();
  readonly actions = input.required<ConfirmDialogAction[]>();
  /** Text of the cancel button; the translated "Cancel" when not set. */
  readonly cancelLabel = input<string | null>(null);
  readonly chosen = output<string | null>();

  ngAfterViewInit(): void {
    // Takes the focus from the element that opened the dialog (sidebar row, editor): Enter then confirms the first action
    this.actionButtons()[0]?.nativeElement.focus();
  }
}
