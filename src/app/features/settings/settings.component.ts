import { ChangeDetectionStrategy, Component, ElementRef, ViewChild, computed, effect, inject, signal } from '@angular/core';
import { SettingsService } from '../../core/services/settings.service';
import { DEFAULT_SETTINGS, SettingsPatch } from '../../core/models/settings.model';
import { formatAccelerator, keyEventToAccelerator } from '../../core/utils/accelerator.util';
import { SETTING_DEFINITIONS, SettingDefinition, filterSettings, groupByCategory } from './settings-definitions';

@Component({
  selector: 'app-settings',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (isOpen()) {
      <div class="backdrop" (mousedown)="close()">
        <div class="modal" role="dialog" aria-label="Settings" (mousedown)="$event.stopPropagation()">
          <div class="header">
            <div class="title-row">
              <span class="title">Settings</span>
              <button class="icon-btn" (click)="close()" title="Close (Esc)">✕</button>
            </div>
            <div class="search">
              <input
                #search
                type="text"
                placeholder="Search settings"
                spellcheck="false"
                [value]="query()"
                (input)="query.set(search.value)"
              />
              <span class="count">{{ countLabel() }}</span>
            </div>
          </div>

          <div class="body">
            <nav class="toc">
              @for (section of sections(); track section.category) {
                <button
                  class="toc-item"
                  [class.active]="section.category === activeCategory()"
                  (click)="scrollToCategory(section.category)"
                >{{ section.category }}</button>
              }
            </nav>

            <div class="list" #list (scroll)="onListScroll()">
              @for (section of sections(); track section.category) {
                <section [attr.data-category]="section.category">
                  <h2>{{ section.category }}</h2>
                  @for (def of section.settings; track def.key) {
                    <div class="setting" [class.modified]="isModified(def)">
                      <div class="setting-title">
                        <span class="category">{{ section.category }}: </span><b>{{ def.title }}</b>
                        @if (isModified(def)) {
                          <button class="reset" (click)="reset(def)" title="Reset to default">↺</button>
                        }
                      </div>

                      @switch (def.control) {
                        @case ('checkbox') {
                          <label class="checkbox">
                            <input type="checkbox" [checked]="settings()[def.key] === true" (change)="onCheckboxChange(def, $event)" />
                            <span class="description">{{ def.description }}</span>
                          </label>
                        }
                        @case ('number') {
                          <div class="description">{{ def.description }}</div>
                          <input
                            class="control"
                            type="number"
                            [min]="def.min ?? null"
                            [max]="def.max ?? null"
                            [step]="def.step ?? 1"
                            [value]="settings()[def.key]"
                            (change)="onNumberChange(def, $event)"
                          />
                        }
                        @case ('select') {
                          <div class="description">{{ def.description }}</div>
                          <select class="control" [value]="settings()[def.key]" (change)="onSelectChange(def, $event)">
                            @for (option of def.options ?? []; track option.value) {
                              <option [value]="option.value">{{ option.label }}</option>
                            }
                          </select>
                        }
                        @case ('shortcut') {
                          <div class="description">{{ def.description }}</div>
                          <button
                            class="control shortcut"
                            data-shortcut-recorder
                            [class.recording]="recording()"
                            (click)="startRecording()"
                            (keydown)="onShortcutKeydown($event)"
                            (blur)="recording.set(false)"
                          >{{ recording() ? 'Press the new shortcut… (Esc to cancel)' : shortcutLabel() }}</button>
                          @if (shortcutError()) {
                            <div class="error">{{ shortcutError() }}</div>
                          }
                        }
                      }
                    </div>
                  }
                </section>
              } @empty {
                <div class="empty">No settings found.</div>
              }
            </div>
          </div>
        </div>
      </div>
    }
  `,
  styleUrls: ['./settings.component.scss'],
})
export class SettingsComponent {
  private settingsService = inject(SettingsService);

  @ViewChild('search') searchRef?: ElementRef<HTMLInputElement>;
  @ViewChild('list') listRef?: ElementRef<HTMLElement>;

  readonly isOpen = this.settingsService.isOpen;
  readonly settings = this.settingsService.settings;
  readonly query = signal('');
  readonly recording = signal(false);
  readonly shortcutError = signal('');
  readonly sections = computed(() => groupByCategory(filterSettings(SETTING_DEFINITIONS, this.query())));
  private readonly scrolledCategory = signal<string | null>(null);
  // Falls back to the first visible section when the scrolled one has been filtered out
  readonly activeCategory = computed(() => {
    const categories = this.sections().map(s => s.category);
    const scrolled = this.scrolledCategory();
    return scrolled && categories.includes(scrolled) ? scrolled : categories[0] ?? null;
  });
  readonly countLabel = computed(() => {
    if (!this.query().trim()) return '';
    const count = this.sections().reduce((total, s) => total + s.settings.length, 0);
    return `${count} setting${count === 1 ? '' : 's'} found`;
  });

  constructor() {
    effect(() => {
      if (!this.isOpen()) return;
      // Fresh state on every opening, search focused like VS Code
      this.query.set('');
      this.scrolledCategory.set(null);
      this.shortcutError.set('');
      setTimeout(() => this.searchRef?.nativeElement.focus());
    });
  }

  close(): void { this.settingsService.close(); }

  shortcutLabel(): string {
    return formatAccelerator(this.settings().globalShortcut);
  }

  isModified(def: SettingDefinition): boolean {
    return this.settings()[def.key] !== DEFAULT_SETTINGS[def.key];
  }

  async reset(def: SettingDefinition): Promise<void> {
    await this.save(def, DEFAULT_SETTINGS[def.key]);
  }

  scrollToCategory(category: string): void {
    const section = this.listRef?.nativeElement.querySelector(`[data-category="${CSS.escape(category)}"]`);
    section?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    this.scrolledCategory.set(category);
  }

  onListScroll(): void {
    const list = this.listRef?.nativeElement;
    if (!list) return;
    // Active section = last one whose top has reached the top of the list
    let current: string | null = null;
    for (const section of Array.from(list.querySelectorAll<HTMLElement>('section'))) {
      if (section.offsetTop - list.offsetTop <= list.scrollTop + 8) current = section.dataset['category'] ?? null;
    }
    this.scrolledCategory.set(current);
  }

  async onCheckboxChange(def: SettingDefinition, event: Event): Promise<void> {
    await this.save(def, (event.target as HTMLInputElement).checked);
  }

  async onNumberChange(def: SettingDefinition, event: Event): Promise<void> {
    const input = event.target as HTMLInputElement;
    const value = Number(input.value);
    if (input.value !== '' && Number.isFinite(value)) await this.save(def, value);
    // Re-sync with the value actually applied (the main process clamps to its bounds)
    input.value = String(this.settings()[def.key]);
  }

  async onSelectChange(def: SettingDefinition, event: Event): Promise<void> {
    await this.save(def, (event.target as HTMLSelectElement).value);
  }

  startRecording(): void {
    this.shortcutError.set('');
    this.recording.set(true);
  }

  async onShortcutKeydown(event: KeyboardEvent): Promise<void> {
    if (!this.recording()) return;
    event.preventDefault();
    event.stopPropagation();

    if (event.key === 'Escape') {
      this.recording.set(false);
      return;
    }

    const accelerator = keyEventToAccelerator(event);
    if (!accelerator) return;
    this.recording.set(false);

    const result = await this.settingsService.update({ globalShortcut: accelerator });
    if (result && !result.ok) {
      this.shortcutError.set(`${formatAccelerator(accelerator)} is unavailable (already used by another application?).`);
    }
  }

  private async save(def: SettingDefinition, value: unknown): Promise<void> {
    // The main process validates every value (type, bounds) before applying it
    await this.settingsService.update({ [def.key]: value } as SettingsPatch);
  }
}
