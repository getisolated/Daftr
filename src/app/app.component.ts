import {
  Component, OnInit, OnDestroy, ViewChild, inject, ChangeDetectionStrategy
} from '@angular/core';
import { CommonModule } from '@angular/common';
import { TabsBarComponent } from './features/tabs/tabs-bar.component';
import { EditorComponent } from './features/editor/editor.component';
import { PaletteComponent } from './features/palette/palette.component';
import { HomeComponent } from './features/home/home.component';
import { SettingsComponent } from './features/settings/settings.component';
import { NotesService } from './core/services/notes.service';
import { TabsService } from './core/services/tabs.service';
import { PaletteService } from './core/services/palette.service';
import { ShortcutsService } from './core/services/shortcuts.service';
import { ElectronBridgeService } from './core/services/electron-bridge.service';
import { LayoutService } from './core/services/layout.service';
import { SettingsService } from './core/services/settings.service';
import { SAMPLE_NOTES } from './core/data/sample-notes';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [CommonModule, TabsBarComponent, EditorComponent, PaletteComponent, HomeComponent, SettingsComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="app-shell">
      <app-tabs-bar />
      <div class="app-content" [class.wide]="wideMode()">
        <!-- Always keep both in DOM to avoid editor teardown on home navigation -->
        <app-home [style.display]="activeNoteId() !== null ? 'none' : 'flex'" />
        <app-editor
          #editor
          [noteId]="activeNoteId()"
          [style.display]="activeNoteId() === null ? 'none' : 'flex'"
        />
      </div>
      <app-palette />
      <app-settings />
    </div>
  `,
  styles: [`
    :host { display: contents; }
    .app-shell {
      display: flex;
      flex-direction: column;
      height: 100vh;
      overflow: hidden;
      background: #1e1e1e;
    }
    .app-content {
      display: flex;
      flex: 1;
      overflow: hidden;
      min-height: 0;
    }
    .app-content.wide ::ng-deep .cm-content {
      max-width: 100% !important;
    }
    .app-content.wide ::ng-deep .home-inner {
      max-width: 100%;
    }
  `],
})
export class AppComponent implements OnInit, OnDestroy {
  @ViewChild('editor') editorRef!: EditorComponent;

  private notesService = inject(NotesService);
  private tabsService = inject(TabsService);
  private paletteService = inject(PaletteService);
  private shortcutsService = inject(ShortcutsService);
  private bridge = inject(ElectronBridgeService);
  private layoutService = inject(LayoutService);
  private settingsService = inject(SettingsService);

  readonly activeNoteId = this.tabsService.activeNoteId;
  readonly wideMode = this.layoutService.wideMode;

  private unregisterFns: (() => void)[] = [];
  private unsubscribeNotesChanged: () => void = () => undefined;

  async ngOnInit(): Promise<void> {
    try {
      await this.notesService.loadAll();

      // Seed sample notes on first launch
      if (this.notesService.notes().length === 0) {
        await this.seedSampleNotes();
      }

      const noteIds = new Set(this.notesService.notes().map(n => n.id));
      this.tabsService.restoreSession(noteIds);
    } catch (err) {
      console.error('[App] Failed to load notes:', err);
    }
    // Notes sent from the quick-note overlay show up without restarting the app
    this.unsubscribeNotesChanged = this.bridge.onNotesChanged(() => { this.notesService.loadAll(); });
    this.registerShortcuts();
  }

  private async seedSampleNotes(): Promise<void> {
    for (const sample of SAMPLE_NOTES) {
      await this.bridge.createNote(sample.title, sample.content);
    }
    await this.notesService.loadAll();
  }

  ngOnDestroy(): void {
    this.unregisterFns.forEach(fn => fn());
    this.unsubscribeNotesChanged();
  }

  private registerShortcuts(): void {
    const reg = (def: Parameters<ShortcutsService['register']>[0]) => {
      this.unregisterFns.push(this.shortcutsService.register(def));
    };

    // Prevent Ctrl+R from reloading the app
    reg({ key: 'r', ctrl: true, handler: () => { /* noop — block browser reload */ } });

    reg({ key: 'F1', handler: () => this.paletteService.toggle('command') });
    reg({ key: 'n', ctrl: true, handler: () => this.createNote() });
    reg({ key: 'w', ctrl: true, handler: () => this.tabsService.closeActiveTab() });
    reg({ key: 'Tab', ctrl: true, handler: () => this.tabsService.nextTab(), preventDefault: true });
    reg({ key: 'Tab', ctrl: true, shift: true, handler: () => this.tabsService.prevTab(), preventDefault: true });
    reg({ key: 'f', ctrl: true, shift: true, handler: () => this.paletteService.open('search') });
    reg({
      key: 'f', ctrl: true,
      handler: () => { if (this.activeNoteId() !== null) this.editorRef?.openSearch(); }
    });
    reg({
      key: 'Escape', preventDefault: false,
      handler: () => {
        if (this.settingsService.isOpen()) this.settingsService.close();
        else if (this.paletteService.isOpen()) this.paletteService.close();
        else this.editorRef?.closeSearch();
      }
    });

    reg({ key: ',', ctrl: true, handler: () => this.settingsService.open() });
    reg({ key: 'z', alt: true, handler: () => this.layoutService.toggle() });
    reg({ key: 'p', alt: true, handler: () => this.tabsService.togglePinActiveTab() });

    // Alt+Ctrl+1-9: switch to tab by index
    for (let i = 1; i <= 9; i++) {
      const idx = i - 1;
      reg({ key: String(i), ctrl: true, alt: true, handler: () => this.tabsService.setActiveIndex(idx) });
    }
  }

  private async createNote(): Promise<void> {
    const note = await this.notesService.create();
    this.tabsService.openNote(note.id, 'Untitled');
  }
}
