import { ChangeDetectionStrategy, Component, computed, ElementRef, inject, input, signal, viewChild } from '@angular/core';

import { Tab } from '../../core/models/note.model';
import { TabsStyle } from '../../core/models/settings.model';
import { TabsService } from '../../core/services/tabs.service';
import { PaletteService } from '../../core/services/palette.service';
import { SettingsService } from '../../core/services/settings.service';
import { dialSlots, documentHue, monogram } from '../../core/utils/docs-navigator.util';

/** Older documents shown by the trail; the others stay reachable with Ctrl+E. */
const TRAIL_DEPTH = 8;
/** Cards in the deck; the others stay reachable with Ctrl+E. */
const DECK_SIZE = 12;

/** A document as drawn by the navigators. */
interface NavigatorDoc {
  noteId: number;
  title: string;
  isDirty: boolean;
  monogram: string;
  hue: number;
}

/**
 * Experimental ways to move between the open documents without a row of tabs (Settings → Experimental → Tabs Style):
 * - trail: the documents along the time they were used, the current one in the middle, older ones fading away;
 * - deck: a pile of cards, the current one on top, fanned out on hover;
 * - dial: a half circle of documents unfolding under the title bar, the most recent closest to the hand.
 */
@Component({
  selector: 'app-docs-navigator',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown.escape)': 'closeDial()',
  },
  template: `
    @switch (mode()) {
      @case ('trail') {
        <div class="trail">
          <div class="trail-past">
            @for (doc of trailPast(); track doc.noteId; let depth = $index) {
              <button
                type="button"
                class="trail-item"
                [style.--depth]="depth + 1"
                [class.dirty]="doc.isDirty"
                (click)="open(doc.noteId)"
                (auxclick)="onAuxClick($event, doc.noteId)"
                (dblclick)="$event.stopPropagation()"
                [title]="doc.title"
              >{{ doc.title }}</button>
            }
          </div>
          <button
            type="button"
            class="trail-current"
            [class.dirty]="current()?.isDirty"
            (click)="openRecents()"
            (dblclick)="$event.stopPropagation()"
            [title]="'Recent documents (' + shortcutLabel('tab.recent') + ')'"
          >{{ currentTitle() }}</button>
          <div class="trail-future"></div>
        </div>
      }

      @case ('deck') {
        <div class="deck" [style.--count]="deck().length" (dblclick)="$event.stopPropagation()">
          @for (doc of deck(); track doc.noteId; let i = $index) {
            <button
              type="button"
              class="card"
              [class.top]="i === 0"
              [class.dirty]="doc.isDirty"
              [style.--i]="i"
              [style.--hue]="doc.hue"
              [style.z-index]="deck().length - i"
              (click)="open(doc.noteId)"
              (auxclick)="onAuxClick($event, doc.noteId)"
              [title]="doc.title"
            >
              <span class="card-title">{{ doc.title }}</span>
            </button>
          } @empty {
            <span class="card top empty">Home</span>
          }
        </div>
      }

      @case ('dial') {
        <button
          #orb
          type="button"
          class="orb"
          [class.open]="dialOpen()"
          (click)="toggleDial()"
          (dblclick)="$event.stopPropagation()"
          [title]="'Open documents (' + shortcutLabel('tab.recent') + ' to search them)'"
        >
          <span class="monogram" [style.--hue]="current()?.hue ?? 0" [class.home]="!current()">{{ current()?.monogram ?? '⌂' }}</span>
          <span class="orb-title">{{ currentTitle() }}</span>
          @if (dialDocs().length > 0) {
            <span class="orb-count">{{ dialDocs().length }}</span>
          }
        </button>

        @if (dialOpen()) {
          <div class="dial-backdrop" (click)="closeDial()" (contextmenu)="closeDial()"></div>
          <div class="dial" [style.left.px]="dialCenter().x" [style.top.px]="dialCenter().y">
            @for (doc of dialDocs(); track doc.noteId; let i = $index) {
              <button
                type="button"
                class="dial-item"
                [class.dirty]="doc.isDirty"
                [style.--x.px]="slots()[i].x"
                [style.--y.px]="slots()[i].y"
                [style.--i]="i"
                [style.--hue]="doc.hue"
                (click)="open(doc.noteId)"
                (auxclick)="onAuxClick($event, doc.noteId)"
                [title]="doc.title"
              >
                <span class="monogram">{{ doc.monogram }}</span>
                <span class="dial-title">{{ doc.title }}</span>
              </button>
            }
          </div>
        }
      }
    }
  `,
  styleUrls: ['./docs-navigator.component.scss'],
})
export class DocsNavigatorComponent {
  private readonly tabs = inject(TabsService);
  private readonly settings = inject(SettingsService);
  private readonly palette = inject(PaletteService);

  public readonly mode = input.required<TabsStyle>();

  private readonly orb = viewChild<ElementRef<HTMLElement>>('orb');

  /** Open documents, most recently used first. */
  private readonly recent = computed<NavigatorDoc[]>(() => this.tabs.recentTabs().map(tab => this.toDoc(tab)));

  protected readonly current = computed<NavigatorDoc | null>(() => {
    const active = this.tabs.activeTab();
    return active ? this.toDoc(active) : null;
  });

  protected readonly currentTitle = computed(() => this.current()?.title ?? 'Home');

  /** Every open document but the current one, most recent first. */
  private readonly others = computed(() => this.recent().filter(doc => doc.noteId !== this.current()?.noteId));

  protected readonly trailPast = computed(() => this.others().slice(0, TRAIL_DEPTH));

  /** The current document on top of the pile, then the others by recency. */
  protected readonly deck = computed(() => {
    const current = this.current();
    const cards = current ? [current, ...this.others()] : this.others();
    return cards.slice(0, DECK_SIZE);
  });

  protected readonly dialDocs = this.others;
  protected readonly slots = computed(() => dialSlots(this.dialDocs().length));
  protected readonly dialOpen = signal(false);
  protected readonly dialCenter = signal({ x: 0, y: 0 });

  protected shortcutLabel(id: string): string {
    return this.settings.shortcutLabel(id);
  }

  protected open(noteId: number): void {
    const index = this.tabs.tabs().findIndex(tab => tab.noteId === noteId);
    this.tabs.setActiveIndex(index);
    this.dialOpen.set(false);
  }

  protected openRecents(): void {
    this.palette.openRecents();
  }

  /** Middle click closes the document, as on a tab. */
  protected onAuxClick(event: MouseEvent, noteId: number): void {
    if (event.button === 1) {
      event.preventDefault();
      this.tabs.closeByNoteId(noteId);
    }
  }

  protected toggleDial(): void {
    const orb = this.orb()?.nativeElement;

    if (this.dialOpen() || !orb) {
      this.dialOpen.set(false);
      return;
    }

    // The dial hangs from the orb; fixed coordinates because the orb is centered with a transform
    const rect = orb.getBoundingClientRect();
    this.dialCenter.set({ x: rect.left + rect.width / 2, y: rect.bottom });
    this.dialOpen.set(true);
  }

  protected closeDial(): void {
    this.dialOpen.set(false);
  }

  private toDoc(tab: Tab): NavigatorDoc {
    const title = tab.title || 'Untitled';
    return { noteId: tab.noteId, title, isDirty: tab.isDirty, monogram: monogram(title), hue: documentHue(tab.noteId) };
  }
}
