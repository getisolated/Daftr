import { ChangeDetectionStrategy, Component, input, output } from '@angular/core';

/**
 * Small "+" button at the end of a sidebar row (a folder), creating an item inside it. Hidden by default: the row
 * shows it on hover and focus-within, in place of its metadata. Clicks and keys stay on the button, so the row is
 * neither toggled, selected nor dragged.
 */
@Component({
  selector: 'app-row-add-button',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <button
      type="button"
      [attr.aria-label]="label()"
      [title]="label()"
      (mousedown)="onMouseDown($event)"
      (click)="onClick($event)"
      (keydown)="$event.stopPropagation()"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 5v14M5 12h14"/>
      </svg>
    </button>
  `,
  styles: `
    :host {
      display: none;
      flex-shrink: 0;
      margin-left: auto;
    }

    button {
      display: flex;
      align-items: center;
      justify-content: center;
      width: 20px;
      height: 20px;
      padding: 0;
      border: none;
      border-radius: 4px;
      background: transparent;
      color: var(--daftr-text-soft);
      cursor: pointer;

      &:hover {
        color: var(--daftr-text-bright);
        background: rgba(var(--daftr-hover-rgb), 0.12);
      }

      &:focus-visible {
        outline: 1px solid var(--daftr-accent-strong);
        outline-offset: -1px;
      }
    }
  `,
})
export class RowAddButtonComponent {
  /** Accessible name and tooltip of the button. */
  readonly label = input.required<string>();
  readonly pressed = output<void>();

  /** Starting a press here must not start dragging the (draggable) row, nor move the focus away from the tree. */
  protected onMouseDown(event: MouseEvent): void {
    event.preventDefault();
    event.stopPropagation();
  }

  protected onClick(event: MouseEvent): void {
    event.stopPropagation();
    this.pressed.emit();
  }
}
