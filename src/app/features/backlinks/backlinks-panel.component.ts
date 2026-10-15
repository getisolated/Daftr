import { Component, ChangeDetectionStrategy, inject, signal } from '@angular/core';

import { BacklinksService } from '../../core/services/backlinks.service';
import { TabsService } from '../../core/services/tabs.service';
import { Backlink } from '../../core/utils/note-links.util';

const STORAGE_KEY = 'evnote-backlinks-open';

/** Notes linking to the active note, under the editor. */
@Component({
  selector: 'app-backlinks-panel',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="backlinks">
      <button type="button" class="backlinks-header" [attr.aria-expanded]="open()" (click)="toggle()">
        <svg class="chevron" [class.open]="open()" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
          <polyline points="9 18 15 12 9 6"/>
        </svg>
        <span>Backlinks</span>
        <span class="count">{{ backlinks().length }}</span>
      </button>

      @if (open()) {
        <div class="backlinks-list">
          @for (link of backlinks(); track link.noteId) {
            <div class="backlink" tabindex="0" (click)="openNote(link)" (keydown.enter)="openNote(link)">
              <span class="backlink-title">{{ link.title }}</span>
              @if (link.excerpt) {
                <span class="backlink-excerpt">{{ link.excerpt }}</span>
              }
            </div>
          } @empty {
            <p class="backlinks-empty">No note links here yet.</p>
          }
        </div>
      }
    </section>
  `,
  styleUrl: './backlinks-panel.component.scss',
})
export class BacklinksPanelComponent {
  private readonly service = inject(BacklinksService);
  private readonly tabs = inject(TabsService);

  readonly backlinks = this.service.backlinks;
  readonly open = signal(this.loadOpen());

  toggle(): void {
    this.open.update(open => !open);

    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.open()));
    } catch {
      // Storage unavailable: the choice only lasts for this session
    }
  }

  openNote(link: Backlink): void {
    this.tabs.openNote(link.noteId, link.title);
  }

  private loadOpen(): boolean {
    try {
      return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'true') !== false;
    } catch {
      return true;
    }
  }
}
