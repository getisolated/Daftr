import {
  ChangeDetectionStrategy, Component, ElementRef, OnDestroy, OnInit, ViewChild, computed, inject, signal
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import { Note } from '../../core/models/note.model';
import { OVERLAY_NOTE_TAG } from '../../core/models/settings.model';
import { ElectronBridgeService } from '../../core/services/electron-bridge.service';
import { NotesService } from '../../core/services/notes.service';
import { SettingsService } from '../../core/services/settings.service';

const MAX_DISPLAYED_NOTES = 100;
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
    <div class="overlay">
      <div class="drag-handle" title="Drag to move"><span class="grip"></span></div>

      <!-- column-reverse: newest message first in the DOM, rendered at the bottom, scroll anchored to the bottom -->
      <div class="messages">
        @for (message of reversedMessages(); track message.note.id) {
          <div class="message" [class.enter]="message.fresh">{{ message.note.content }}</div>
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
  private bridge = inject(ElectronBridgeService);
  private notesService = inject(NotesService);
  private settingsService = inject(SettingsService);

  @ViewChild('input', { static: true }) inputRef!: ElementRef<HTMLTextAreaElement>;

  readonly messages = signal<OverlayMessage[]>([]);
  readonly reversedMessages = computed(() => [...this.messages()].reverse());
  // Lives in the component: the window is only hidden (never destroyed), so the draft survives hide/show
  readonly draft = signal('');
  readonly placeholder = computed(() =>
    this.settingsService.settings().enterToSend
      ? 'Write a note… (Enter to send)'
      : 'Write a note… (Ctrl+Enter to send)');

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

    // Ctrl+Enter always sends; plain Enter sends only when configured so (Shift+Enter keeps the new line)
    const shouldSend = event.ctrlKey || (this.settingsService.settings().enterToSend && !event.shiftKey);
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

    const note = await this.bridge.createNote(this.notesService.extractTitle(text), text, [OVERLAY_NOTE_TAG]);
    this.messages.update(list => [...list, { note, fresh: true }].slice(-MAX_DISPLAYED_NOTES));
  }

  private async loadMessages(): Promise<void> {
    const notes = await this.bridge.getNotesByTag(OVERLAY_NOTE_TAG);
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
    element.style.height = `${Math.min(element.scrollHeight, MAX_INPUT_HEIGHT_PX)}px`;
  }
}
