import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { FormattingBarService } from '../../core/services/formatting-bar.service';
import { SettingsService } from '../../core/services/settings.service';
import { FORMATTING_BAR_GROUPS, FORMAT_ACTIONS, FormatAction } from '../../shared/codemirror/formatting-commands';
import { FormatId } from '../../core/models/formatting-bar.model';

/** Shortcuts already bound in the editor, shown in the tooltips. */
const SHORTCUT_IDS: Partial<Record<FormatId, string>> = { bold: 'editor.bold', italic: 'editor.italic' };

@Component({
  selector: 'app-formatting-bar',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    role: 'toolbar',
    'aria-label': 'Formatting',
    '[attr.data-mode]': 'bar.mode()',
    '[attr.data-style]': 'bar.style()',
    '[attr.data-position]': 'bar.position()',
    '[class.disabled]': '!bar.canEdit()',
    // A click on the bar must not take the focus (nor the selection) away from the editor
    '(mousedown)': '$event.preventDefault()',
  },
  template: `
    @for (group of groups; track $index) {
      @if ($index > 0) {
        <span class="separator" aria-hidden="true"></span>
      }
      <div class="group" role="group">
        @for (item of group; track item.id) {
          <button
            type="button"
            class="tool"
            [class.active]="bar.activeFormats().has(item.id)"
            [attr.aria-pressed]="bar.activeFormats().has(item.id)"
            [attr.aria-label]="item.label"
            [title]="tooltip(item)"
            [disabled]="!bar.canEdit()"
            tabindex="-1"
            (click)="apply(item)"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" [innerHTML]="icons[item.id]"></svg>
          </button>
        }
      </div>
    }
  `,
  styleUrls: ['./formatting-bar.component.scss'],
})
export class FormattingBarComponent {
  protected readonly bar = inject(FormattingBarService);
  private readonly settingsService = inject(SettingsService);

  private readonly sanitizer = inject(DomSanitizer);

  /** The icons are the app's own constant SVG markup (no user input), which Angular's sanitizer would strip. */
  protected readonly icons = Object.fromEntries(
    Object.values(FORMAT_ACTIONS).map(action => [action.id, this.sanitizer.bypassSecurityTrustHtml(action.icon)]),
  ) as Record<FormatId, SafeHtml>;

  protected readonly groups: FormatAction[][] = FORMATTING_BAR_GROUPS.map(ids => ids.map(id => FORMAT_ACTIONS[id]));

  protected tooltip(item: FormatAction): string {
    const shortcutId = SHORTCUT_IDS[item.id];
    return shortcutId ? `${item.label} (${this.settingsService.shortcutLabel(shortcutId)})` : item.label;
  }

  protected apply(item: FormatAction): void {
    this.bar.run(view => item.toggle(view));
  }
}
