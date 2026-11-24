import { ChangeDetectionStrategy, Component, input, output } from '@angular/core';

/** What the stack needs of a notification (see `Toast` in the core models). */
export interface ToastItem {
  id: number;
  message: string;
  action: { label: string } | null;
}

/** Notifications stacked in the bottom right corner; each one has its action (optional) and a close button. */
@Component({
  selector: 'app-toast-stack',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @for (toast of toasts(); track toast.id) {
      <div class="toast" role="status">
        <span class="message">{{ toast.message }}</span>
        @if (toast.action; as action) {
          <button type="button" class="action" (click)="acted.emit(toast.id)">{{ action.label }}</button>
        }
        <button type="button" class="close" [title]="closeLabel()" [attr.aria-label]="closeLabel()" (click)="dismissed.emit(toast.id)">✕</button>
      </div>
    }
  `,
  styles: [`
    :host {
      position: fixed;
      right: 16px;
      bottom: 16px;
      z-index: 3500;
      display: flex;
      flex-direction: column;
      align-items: flex-end;
      gap: 8px;
      pointer-events: none;
      font-family: 'Segoe UI', system-ui, sans-serif;
    }
    .toast {
      display: flex;
      align-items: center;
      gap: 10px;
      max-width: 380px;
      padding: 8px 8px 8px 12px;
      background: var(--daftr-bg-panel);
      border: 1px solid var(--daftr-border);
      border-left: 3px solid var(--daftr-accent);
      border-radius: 6px;
      box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
      color: var(--daftr-text);
      font-size: 12.5px;
      line-height: 1.4;
      pointer-events: auto;
      animation: toast-in 0.15s ease-out;
    }
    .message { flex: 1; user-select: text; }
    .action {
      flex: none;
      padding: 3px 10px;
      border: 1px solid var(--daftr-accent-strong);
      border-radius: 4px;
      background: var(--daftr-accent-strong);
      color: #fff;
      font: inherit;
      cursor: pointer;
    }
    .action:hover { background: var(--daftr-accent-hover); }
    .close {
      flex: none;
      width: 22px;
      height: 22px;
      border: none;
      border-radius: 4px;
      background: transparent;
      color: var(--daftr-text-soft);
      cursor: pointer;
    }
    .close:hover { color: var(--daftr-text-bright); background: rgba(var(--daftr-hover-rgb), 0.08); }
    @keyframes toast-in { from { opacity: 0; transform: translateY(6px); } }
  `],
})
export class ToastStackComponent {
  readonly toasts = input.required<readonly ToastItem[]>();
  readonly closeLabel = input.required<string>();
  readonly acted = output<number>();
  readonly dismissed = output<number>();
}
