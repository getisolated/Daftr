import { ChangeDetectionStrategy, Component, input, output } from '@angular/core';

/** Themed checkbox; the projected content is its label and is clickable too. */
@Component({
  selector: 'app-checkbox',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    role: 'checkbox',
    tabindex: '0',
    '[attr.aria-checked]': 'checked()',
    '[class.checked]': 'checked()',
    '(click)': 'toggle()',
    '(keydown.space)': 'toggle(); $event.preventDefault()',
    '(keydown.enter)': 'toggle(); $event.preventDefault()',
  },
  template: `
    <span class="box">
      @if (checked()) {
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="3 8.5 6.5 12 13 4.5" />
        </svg>
      }
    </span>
    <span class="label"><ng-content /></span>
  `,
  styleUrls: ['./checkbox.component.scss'],
})
export class CheckboxComponent {
  readonly checked = input(false);
  readonly checkedChange = output<boolean>();

  protected toggle(): void {
    this.checkedChange.emit(!this.checked());
  }
}
