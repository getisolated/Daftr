import { ChangeDetectionStrategy, Component, input, output } from '@angular/core';

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
          <button class="btn" [class.danger]="action.danger" (click)="chosen.emit(action.id)">{{ action.label }}</button>
        }
        <button class="btn cancel" (click)="chosen.emit(null)">Cancel</button>
      </div>
    </div>
  `,
  styleUrls: ['./confirm-dialog.component.scss'],
})
export class ConfirmDialogComponent {
  readonly title = input.required<string>();
  readonly message = input.required<string>();
  readonly actions = input.required<ConfirmDialogAction[]>();
  readonly chosen = output<string | null>();
}
