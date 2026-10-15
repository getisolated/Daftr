import { ChangeDetectionStrategy, Component, ElementRef, computed, inject, input, output, signal } from '@angular/core';

export interface SelectOption {
  value: string;
  label: string;
}

const PANEL_GAP_PX = 2;
const PANEL_MAX_HEIGHT_PX = 240;
const OPTION_HEIGHT_PX = 28;
const PANEL_PADDING_PX = 8;

/** Themed drop-down: a button showing the current option and a floating list (keyboard and mouse). */
@Component({
  selector: 'app-select',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '[attr.data-select-open]': 'isOpen() ? "" : null',
    '(document:mousedown)': 'onDocumentMousedown($event)',
    '(window:resize)': 'close()',
  },
  template: `
    <button
      type="button"
      class="trigger"
      aria-haspopup="listbox"
      [attr.aria-expanded]="isOpen()"
      [class.open]="isOpen()"
      (click)="toggle()"
      (keydown)="onKeydown($event)"
    >
      <span class="current">{{ currentLabel() }}</span>
      <svg class="chevron" width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="3 6 8 11 13 6" />
      </svg>
    </button>

    @if (isOpen()) {
      <div class="panel" role="listbox" [style.left.px]="panelRect().left" [style.top.px]="panelRect().top" [style.min-width.px]="panelRect().width">
        @for (option of options(); track option.value; let i = $index) {
          <div
            class="option"
            role="option"
            [attr.aria-selected]="option.value === value()"
            [class.selected]="option.value === value()"
            [class.highlighted]="i === highlightedIndex()"
            (mouseenter)="highlightedIndex.set(i)"
            (mousedown)="$event.preventDefault()"
            (click)="choose(option)"
          >
            <span class="option-label">{{ option.label }}</span>
            @if (option.value === value()) {
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="3 8.5 6.5 12 13 4.5" />
              </svg>
            }
          </div>
        }
      </div>
    }
  `,
  styleUrls: ['./select.component.scss'],
})
export class SelectComponent {
  private host = inject<ElementRef<HTMLElement>>(ElementRef);

  readonly value = input('');
  readonly options = input<SelectOption[]>([]);
  readonly valueChange = output<string>();

  protected readonly isOpen = signal(false);
  protected readonly highlightedIndex = signal(0);
  protected readonly panelRect = signal({ left: 0, top: 0, width: 0 });
  protected readonly currentLabel = computed(() => this.options().find(option => option.value === this.value())?.label ?? '');

  // Closing on any scroll keeps the fixed panel from drifting away from its trigger
  private readonly closeOnScroll = (): void => this.close();

  protected toggle(): void {
    if (this.isOpen()) {
      this.close();
    } else {
      this.open();
    }
  }

  protected close(): void {
    if (!this.isOpen()) return;
    this.isOpen.set(false);
    window.removeEventListener('scroll', this.closeOnScroll, true);
  }

  protected choose(option: SelectOption): void {
    this.close();
    if (option.value !== this.value()) {
      this.valueChange.emit(option.value);
    }
  }

  protected onDocumentMousedown(event: MouseEvent): void {
    if (this.isOpen() && !this.host.nativeElement.contains(event.target as Node)) {
      this.close();
    }
  }

  protected onKeydown(event: KeyboardEvent): void {
    const count = this.options().length;
    switch (event.key) {
      case 'Escape':
        if (this.isOpen()) {
          event.preventDefault();
          event.stopPropagation();
          this.close();
        }
        break;
      case 'ArrowDown':
      case 'ArrowUp': {
        event.preventDefault();
        if (!this.isOpen()) {
          this.open();
          break;
        }
        const step = event.key === 'ArrowDown' ? 1 : -1;
        this.highlightedIndex.update(index => (index + step + count) % count);
        break;
      }
      case 'Enter':
      case ' ': {
        event.preventDefault();
        const highlighted = this.options()[this.highlightedIndex()];
        if (this.isOpen() && highlighted) {
          this.choose(highlighted);
        } else {
          this.open();
        }
        break;
      }
      case 'Tab':
        this.close();
        break;
    }
  }

  private open(): void {
    const trigger = this.host.nativeElement.getBoundingClientRect();
    const spaceBelow = window.innerHeight - trigger.bottom;
    const panelHeight = Math.min(PANEL_MAX_HEIGHT_PX, this.options().length * OPTION_HEIGHT_PX + PANEL_PADDING_PX);
    // Opens upwards when there is no room below
    const top = spaceBelow >= panelHeight + PANEL_GAP_PX ? trigger.bottom + PANEL_GAP_PX : trigger.top - panelHeight - PANEL_GAP_PX;
    this.panelRect.set({ left: trigger.left, top, width: trigger.width });
    this.highlightedIndex.set(Math.max(0, this.options().findIndex(option => option.value === this.value())));
    this.isOpen.set(true);
    window.addEventListener('scroll', this.closeOnScroll, true);
  }
}
