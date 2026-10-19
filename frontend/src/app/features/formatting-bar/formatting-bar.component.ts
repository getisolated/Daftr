import {
  ChangeDetectionStrategy, Component, DestroyRef, ElementRef, Injector, afterNextRender, computed, effect, inject, signal,
} from '@angular/core';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { I18nService } from '../../core/i18n/i18n.service';
import { FormattingBarService } from '../../core/services/formatting-bar.service';
import { SettingsService } from '../../core/services/settings.service';
import { TranslationKey } from '../../core/i18n/translations';
import { shouldCondense } from '../../core/utils/formatting-bar.util';
import { FORMATTING_BAR_GROUPS, FORMAT_ACTIONS, FormatAction } from '../../shared/codemirror/formatting-commands';
import {
  FormatId, FormattingBarGroup, FormattingBarGroupId, FormattingBarItem, HEADING_FORMATS, HeadingFormat,
} from '../../core/models/formatting-bar.model';
import { SelectComponent, SelectOption, SelectPlacement } from '../../shared/ui/select.component';

/** Shortcuts already bound in the editor, shown in the tooltips. */
const SHORTCUT_IDS: Partial<Record<FormatId, string>> = { bold: 'editor.bold', italic: 'editor.italic', link: 'editor.link' };

/** Labels of the heading drop-down once closed in a vertical bar, where only a few characters fit. */
const HEADING_SHORT_LABELS: Record<HeadingFormat, string> = {
  paragraph: 'P', h1: 'H1', h2: 'H2', h3: 'H3', h4: 'H4', h5: 'H5', h6: 'H6',
};

type CondensedGroupId = Exclude<FormattingBarGroupId, 'heading'>;

/** Name of each group once condensed into a drop-down. */
const GROUP_LABEL_KEYS: Record<CondensedGroupId, TranslationKey> = {
  text: 'editor.formatBar.group.text',
  insert: 'editor.formatBar.group.insert',
  blocks: 'editor.formatBar.group.blocks',
  lists: 'editor.formatBar.group.lists',
};

const STROKE = 'stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"';

/** Icon of each condensed group in a vertical bar, where only icons fit. */
const GROUP_ICONS: Record<CondensedGroupId, string> = {
  text: '<text x="1" y="18" font-size="15" font-weight="700" fill="currentColor">A</text>'
    + '<text x="13" y="18" font-size="12" font-style="italic" fill="currentColor">a</text>',
  insert: `<path d="M12 5v14M5 12h14" ${STROKE}/>`,
  blocks: `<g ${STROKE}><rect x="3" y="4" width="18" height="7" rx="1.5"/><rect x="3" y="13" width="18" height="7" rx="1.5"/></g>`,
  lists: `<path d="M9 6h12M9 12h12M9 18h12M4 6h.01M4 12h.01M4 18h.01" ${STROKE} stroke-width="2.4"/>`,
};

function isFormat(item: FormattingBarItem): item is FormatId {
  return item !== 'heading';
}

@Component({
  selector: 'app-formatting-bar',
  standalone: true,
  imports: [SelectComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    role: 'toolbar',
    '[attr.aria-label]': 'i18n.t("editor.formatBar.label")',
    '[attr.aria-orientation]': 'vertical() ? "vertical" : "horizontal"',
    '[attr.data-mode]': 'bar.mode()',
    '[attr.data-style]': 'bar.style()',
    '[attr.data-position]': 'bar.position()',
    '[class.condensed]': 'condensed()',
    '[class.disabled]': '!bar.canEdit()',
    // A click on the bar must not take the focus (nor the selection) away from the editor
    '(mousedown)': '$event.preventDefault()',
  },
  template: `
    @for (group of groups; track group.id) {
      @if ($index > 0 && !condensed()) {
        <span class="separator" aria-hidden="true"></span>
      }
      <div class="group" role="group">
        @if (condensed() && group.id !== 'heading') {
          <!-- Narrow window: the whole group in one drop-down, named (horizontal) or drawn as an icon (vertical) -->
          <app-select
            class="group-select"
            [title]="groupLabel(group)"
            [ariaLabel]="groupLabel(group)"
            value=""
            [options]="groupOptions(group)"
            [placement]="selectPlacement()"
            [triggerLabel]="groupLabel(group)"
            [triggerIcon]="vertical() ? groupIcons[group.id] : null"
            [triggerActive]="isGroupActive(group)"
            (valueChange)="applyFormat($event)"
          />
        } @else {
          @for (item of group.items; track item) {
            @if (item === 'heading') {
              <app-select
                class="heading-select"
                [title]="i18n.t('editor.formatBar.style')"
                [value]="currentHeading()"
                [options]="headingOptions()"
                [placement]="selectPlacement()"
                [triggerLabel]="vertical() || condensed() ? headingShortLabels[currentHeading()] : null"
                (valueChange)="applyHeading($event)"
              />
            } @else {
              @let action = actions[item];
              <button
                type="button"
                class="tool"
                [class.active]="bar.activeFormats().has(action.id)"
                [attr.aria-pressed]="bar.activeFormats().has(action.id)"
                [attr.aria-label]="i18n.t(action.labelKey)"
                [title]="tooltip(action)"
                [disabled]="!bar.canEdit()"
                tabindex="-1"
                (click)="apply(action)"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" [innerHTML]="icons[action.id]"></svg>
              </button>
            }
          }
        }
      </div>
    }
  `,
  styleUrls: ['./formatting-bar.component.scss'],
})
export class FormattingBarComponent {
  protected readonly bar = inject(FormattingBarService);
  protected readonly i18n = inject(I18nService);
  private readonly settingsService = inject(SettingsService);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  private readonly injector = inject(Injector);
  private readonly destroyRef = inject(DestroyRef);

  private readonly sanitizer = inject(DomSanitizer);

  /** The icons are the app's own constant SVG markup (no user input), which Angular's sanitizer would strip. */
  protected readonly icons = Object.fromEntries(
    Object.values(FORMAT_ACTIONS).map(action => [action.id, this.sanitizer.bypassSecurityTrustHtml(action.icon)]),
  ) as Record<FormatId, SafeHtml>;

  protected readonly groupIcons = Object.fromEntries(
    Object.entries(GROUP_ICONS).map(([id, icon]) => [id, this.sanitizer.bypassSecurityTrustHtml(icon)]),
  ) as Record<CondensedGroupId, SafeHtml>;

  /** Length of the full bar along its axis (width, or height when vertical), `null` until measured. */
  private readonly fullLength = signal<number | null>(null);
  /** Room the page gives the bar along that axis. */
  private readonly available = signal(Number.POSITIVE_INFINITY);

  /** The full bar does not fit: one drop-down per group, to keep it short. */
  protected readonly condensed = computed(() => shouldCondense(this.fullLength(), this.available()));

  protected readonly actions = FORMAT_ACTIONS;
  protected readonly groups = FORMATTING_BAR_GROUPS;
  protected readonly headingShortLabels = HEADING_SHORT_LABELS;
  protected readonly headingOptions = computed<SelectOption[]>(() => HEADING_FORMATS.map(id => ({ value: id, label: this.i18n.t(FORMAT_ACTIONS[id].labelKey) })));

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

  constructor() {
    afterNextRender(() => this.observeRoom());

    // Another size, side or language changes the length of the full bar: shown full again to be measured
    effect(() => {
      this.bar.mode();
      this.bar.position();
      this.i18n.language();
      this.fullLength.set(null);
      afterNextRender(() => this.measure(), { injector: this.injector });
    });
  }

  /** Measures again whenever the page (sidebar, window, panels) or the bar itself is resized. */
  private observeRoom(): void {
    const observer = new ResizeObserver(() => this.measure());
    const page = this.host.nativeElement.parentElement;

    observer.observe(this.host.nativeElement);
    if (page) {
      observer.observe(page);
    }
    this.destroyRef.onDestroy(() => observer.disconnect());
  }

  /** Room along the axis of the bar (the page's content box, minus the bar's own margins) and, while full, its length. */
  private measure(): void {
    const bar = this.host.nativeElement;
    const page = bar.parentElement;
    if (!page) {
      return;
    }

    const vertical = this.vertical();
    const own = getComputedStyle(bar);
    const box = getComputedStyle(page);
    const px = (value: string): number => Number.parseFloat(value) || 0;
    const room = vertical
      ? page.clientHeight - px(box.paddingTop) - px(box.paddingBottom) - px(own.marginTop) - px(own.marginBottom)
      : page.clientWidth - px(box.paddingLeft) - px(box.paddingRight) - px(own.marginLeft) - px(own.marginRight);

    this.available.set(room);
    if (!this.condensed()) {
      this.fullLength.set(vertical ? bar.scrollHeight : bar.scrollWidth);
    }
  }

  protected tooltip(item: FormatAction): string {
    const shortcutId = SHORTCUT_IDS[item.id];
    const label = this.i18n.t(item.labelKey);
    return shortcutId ? `${label} (${this.settingsService.shortcutLabel(shortcutId)})` : label;
  }

  protected groupLabel(group: FormattingBarGroup): string {
    return group.id === 'heading' ? this.i18n.t('editor.formatBar.style') : this.i18n.t(GROUP_LABEL_KEYS[group.id]);
  }

  /** The toggles of a condensed group; the ones in effect under the caret are checked. */
  protected groupOptions(group: FormattingBarGroup): SelectOption[] {
    const active = this.bar.activeFormats();
    return group.items.filter(isFormat).map(id => ({
      value: id,
      label: this.i18n.t(FORMAT_ACTIONS[id].labelKey),
      icon: this.icons[id],
      active: active.has(id),
    }));
  }

  protected isGroupActive(group: FormattingBarGroup): boolean {
    const active = this.bar.activeFormats();
    return group.items.some(item => isFormat(item) && active.has(item));
  }

  protected applyFormat(value: string): void {
    const action = Object.values(FORMAT_ACTIONS).find(candidate => candidate.id === value);

    if (action) {
      this.apply(action);
    }
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
