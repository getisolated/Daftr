import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { FormattingBarService } from '../../core/services/formatting-bar.service';
import { SettingsService } from '../../core/services/settings.service';
import { FORMATTING_BAR_GROUPS, FORMAT_ACTIONS, FormatAction } from '../../shared/codemirror/formatting-commands';
import { FormatId, HEADING_FORMATS, HeadingFormat } from '../../core/models/formatting-bar.model';
import { SelectComponent, SelectOption, SelectPlacement } from '../../shared/ui/select.component';

/** Shortcuts already bound in the editor, shown in the tooltips. */
const SHORTCUT_IDS: Partial<Record<FormatId, string>> = { bold: 'editor.bold', italic: 'editor.italic', link: 'editor.link' };

/** Labels of the heading drop-down once closed in a vertical bar, where only a few characters fit. */
const HEADING_SHORT_LABELS: Record<HeadingFormat, string> = {
  paragraph: 'P', h1: 'H1', h2: 'H2', h3: 'H3', h4: 'H4', h5: 'H5', h6: 'H6',
};

@Component({
  selector: 'app-formatting-bar',
  standalone: true,
  imports: [SelectComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    role: 'toolbar',
    'aria-label': 'Formatting',
    '[attr.aria-orientation]': 'vertical() ? "vertical" : "horizontal"',
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
        @for (item of group; track item) {
          @if (item === 'heading') {
            <app-select
              class="heading-select"
              title="Style"
              [value]="currentHeading()"
              [options]="headingOptions"
              [placement]="selectPlacement()"
              [triggerLabel]="vertical() ? headingShortLabels[currentHeading()] : null"
              (valueChange)="applyHeading($event)"
            />
          } @else {
            @let action = actions[item];
            <button
              type="button"
              class="tool"
              [class.active]="bar.activeFormats().has(action.id)"
              [attr.aria-pressed]="bar.activeFormats().has(action.id)"
              [attr.aria-label]="action.label"
              [title]="tooltip(action)"
              [disabled]="!bar.canEdit()"
              tabindex="-1"
              (click)="apply(action)"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" [innerHTML]="icons[action.id]"></svg>
            </button>
          }
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

  protected readonly actions = FORMAT_ACTIONS;
  protected readonly groups = FORMATTING_BAR_GROUPS;
  protected readonly headingShortLabels = HEADING_SHORT_LABELS;
  protected readonly headingOptions: SelectOption[] = HEADING_FORMATS.map(id => ({ value: id, label: FORMAT_ACTIONS[id].label }));

  protected readonly vertical = computed(() => this.bar.position() === 'left' || this.bar.position() === 'right');

  /** The list opens towards the page: away from the edge the bar sits on. */
  protected readonly selectPlacement = computed<SelectPlacement>(() => {
    switch (this.bar.position()) {
      case 'left': return 'right';
      case 'right': return 'left';
      default: return 'below';
    }
  });

  protected readonly currentHeading = computed<HeadingFormat>(() => {
    const active = this.bar.activeFormats();
    return HEADING_FORMATS.find(id => id !== 'paragraph' && active.has(id)) ?? 'paragraph';
  });

  protected tooltip(item: FormatAction): string {
    const shortcutId = SHORTCUT_IDS[item.id];
    return shortcutId ? `${item.label} (${this.settingsService.shortcutLabel(shortcutId)})` : item.label;
  }

  protected apply(item: FormatAction): void {
    this.bar.run(view => item.toggle(view));
  }

  protected applyHeading(value: string): void {
    const heading = HEADING_FORMATS.find(id => id === value);

    if (heading) {
      this.apply(FORMAT_ACTIONS[heading]);
    }
  }
}
