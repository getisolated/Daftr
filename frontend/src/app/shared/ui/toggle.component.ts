import { ChangeDetectionStrategy, Component, input, output } from '@angular/core';

/** Themed on/off switch; the projected content is its label and is clickable too. Same API as the checkbox. */
@Component({
  selector: 'app-toggle',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    role: 'switch',
    '[attr.tabindex]': 'disabled() ? -1 : 0',
    '[attr.aria-checked]': 'checked()',
    '[attr.aria-disabled]': 'disabled() || null',
    '[class.checked]': 'checked()',
    '[class.disabled]': 'disabled()',
    '(click)': 'toggle()',
    '(keydown.space)': 'toggle(); $event.preventDefault()',
    '(keydown.enter)': 'toggle(); $event.preventDefault()',
  },
  template: `
    <span class="track"><span class="thumb"></span></span>
    <span class="label"><ng-content /></span>
  `,
  styleUrls: ['./toggle.component.scss'],
})
export class ToggleComponent {
  readonly checked = input(false);
  readonly disabled = input(false);
  readonly checkedChange = output<boolean>();

  protected toggle(): void {
    if (!this.disabled()) {
      this.checkedChange.emit(!this.checked());
    }
  }
}
