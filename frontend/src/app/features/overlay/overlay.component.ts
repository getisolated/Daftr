import {
  ChangeDetectionStrategy, Component, ElementRef, OnDestroy, OnInit, ViewChild, computed, inject, signal
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import { formatNoteTimestamp } from '../../core/utils/note-timestamp.util';
import { Note } from '../../core/models/note.model';
import { QUICK_NOTE_TAG } from '../../core/models/settings.model';
import { DesktopBridgeService } from '../../core/services/desktop-bridge.service';
import { NotesService } from '../../core/services/notes.service';
import { SettingsService } from '../../core/services/settings.service';
import { PlatformService } from '../../core/services/platform.service';
import { I18nService } from '../../core/i18n/i18n.service';

const MAX_DISPLAYED_NOTES = 50;
const MAX_INPUT_HEIGHT_PX = 140;

interface OverlayMessage {
  note: Note;
  /** True for notes sent during this session: they play the entrance animation. */
  fresh: boolean;
}

@Component({
  selector: 'app-overlay',
  standalone: true,
  imports: [FormsModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="overlay" [class.platform-mac]="platform.isMac">
      <div class="drag-handle" data-tauri-drag-region [title]="i18n.t('workspace.overlay.dragToMove')"><span class="grip" data-tauri-drag-region></span></div>

      <!-- column-reverse: newest message first in the DOM, rendered at the bottom, scroll anchored to the bottom -->
      <div class="messages">
        @for (message of reversedMessages(); track message.note.id) {
          <div class="message" [class.enter]="message.fresh">
            <span>{{ message.note.content }}</span>
            <span class="message-time">{{ formatTime(message.note.createdAt) }}</span>
          </div>
        }
      </div>

      <textarea
        #input
        class="input"
        rows="1"
        [placeholder]="placeholder()"
        [ngModel]="draft()"
        (ngModelChange)="onDraftChange($event)"
        (keydown)="onKeydown($event)"
      ></textarea>
    </div>
  `,
  styleUrls: ['./overlay.component.scss'],
})
export class OverlayComponent implements OnInit, OnDestroy {
  protected readonly i18n = inject(I18nService);
  protected readonly platform = inject(PlatformService);
  private bridge = inject(DesktopBridgeService);
  private notesService = inject(NotesService);
  private settingsService = inject(SettingsService);

  @ViewChild('input', { static: true }) inputRef!: ElementRef<HTMLTextAreaElement>;

  readonly messages = signal<OverlayMessage[]>([]);
  readonly reversedMessages = computed(() => [...this.messages()].reverse());
  // Lives in the component: the window is only hidden (never destroyed), so the draft survives hide/show
  readonly draft = signal('');
  readonly placeholder = computed(() => this.i18n.t('workspace.overlay.placeholder'));

  private unsubscribers: (() => void)[] = [];

  async ngOnInit(): Promise<void> {
    this.unsubscribers.push(
      this.bridge.onOverlayShown(() => this.onShown()),
      this.bridge.onNotesChanged(() => this.loadMessages()),
    );
    await this.loadMessages();
    this.focusInput();
  }

  ngOnDestroy(): void {
    this.unsubscribers.forEach(unsubscribe => unsubscribe());
  }

  protected formatTime(createdAt: string): string {
    return formatNoteTimestamp(createdAt, this.i18n.language());
  }

  onDraftChange(value: string): void {
    this.draft.set(value);
    this.resizeInput();
  }

  onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      this.bridge.hideOverlay();
      return;
    }
    if (event.key !== 'Enter' || event.isComposing) return;

    // Ctrl+Enter (⌘↩ on macOS) always sends; plain Enter sends only when configured so (Shift+Enter keeps the new line)
    const shouldSend = (this.platform.isMac ? event.metaKey : event.ctrlKey) || (this.settingsService.settings().enterToSend && !event.shiftKey);
    if (shouldSend) {
      event.preventDefault();
      this.send();
    }
  }

  private async onShown(): Promise<void> {
    await this.loadMessages();
    this.focusInput();
  }

  private async send(): Promise<void> {
    const text = this.draft().trim();
    if (!text) return;

    // Cleared immediately so a fast second Enter cannot send the same text twice
    this.draft.set('');
    this.resizeInput();

    const note = await this.bridge.createNote(this.notesService.extractTitle(text), text, [QUICK_NOTE_TAG]);
    this.messages.update(list => [...list, { note, fresh: true }].slice(-MAX_DISPLAYED_NOTES));
  }

  private async loadMessages(): Promise<void> {
    const notes = await this.bridge.getNotesByTag(QUICK_NOTE_TAG);
    const recent = notes.sort((a, b) => a.id - b.id).slice(-MAX_DISPLAYED_NOTES);
    this.messages.set(recent.map(note => ({ note, fresh: false })));
  }

  private focusInput(): void {
    // Deferred so the window has actually received focus when the element is focused
    setTimeout(() => {
      this.inputRef.nativeElement.focus();
      this.resizeInput();
    });
  }

  private resizeInput(): void {
    const element = this.inputRef.nativeElement;
    element.style.height = 'auto';
    // border-box sizing: scrollHeight leaves the borders out, without them the field scrolls by a couple of pixels
    const borders = element.offsetHeight - element.clientHeight;
    const height = element.scrollHeight + borders;
    element.style.height = `${Math.min(height, MAX_INPUT_HEIGHT_PX)}px`;
    // Scrollable only once the text exceeds the maximum height
    element.style.overflowY = height > MAX_INPUT_HEIGHT_PX ? 'auto' : 'hidden';
  }
}
