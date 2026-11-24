import {
  ChangeDetectionStrategy, Component, DestroyRef, ElementRef, afterNextRender, afterRenderEffect, computed, effect, inject, input,
  output,
} from '@angular/core';
import { PreviewScrollRequest } from '../../core/models/editor.model';
import { PreviewFind } from '../../core/models/find.model';
import { findRanges, rankInBlock } from '../../shared/markdown/preview-find';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { LinkStatus } from '../../core/utils/note-paths.util';
import { headingSlug, linkAnchor } from '../../core/utils/heading-anchor.util';
import { findActiveHeading, headingBlockIndexes } from '../../core/utils/outline-tracking.util';
import { ImageResolver, createMarkdownRenderer, renderMarkdown } from '../../shared/markdown/markdown-renderer';
import { highlightCodeBlocks, renderMermaidBlocks } from '../../shared/markdown/preview-enhancers';
import { I18nService } from '../../core/i18n/i18n.service';
import { ThemeService } from '../../core/services/theme.service';

/** Space kept above the block a scroll sync brings to the top. */
const SYNC_TOP_MARGIN_PX = 16;

/** Top-level blocks of the rendering, numbered by their source line (see sourceLines in markdown-renderer). */
const BLOCK_SELECTOR = '.markdown-body > [data-line]';

/** Names of the CSS highlights of the search (styles.scss): every match, and the current one. */
const FIND_HIGHLIGHT = 'daftr-find';
const FIND_CURRENT_HIGHLIGHT = 'daftr-find-current';

/** GitHub-like, read-only rendering of a note (preview mode, or beside the editor). */
@Component({
  selector: 'app-markdown-preview',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(click)': 'onClick($event)',
    '(scroll)': 'onScroll()',
  },
  template: `<article class="markdown-body" [innerHTML]="html()"></article>`,
  styleUrls: ['./markdown-preview.component.scss'],
})
export class MarkdownPreviewComponent {
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  private readonly sanitizer = inject(DomSanitizer);
  private readonly theme = inject(ThemeService);
  private readonly i18n = inject(I18nService);

  readonly content = input('');
  /** Whether a link target exists (same rule as the editor): broken local links are drawn in red. */
  readonly linkStatus = input<(target: string) => LinkStatus>(() => 'note');
  /** Rewrites the image addresses (relative images of a collection file); `null` keeps them as written. */
  readonly resolveImage = input<ImageResolver | null>(null);
  /** The editor scrolled (side by side): the preview follows. */
  readonly scrollRequest = input<PreviewScrollRequest | null>(null);
  /** The search of the find panel, highlighted in the rendering; `null` while the panel is closed. */
  readonly find = input<PreviewFind | null>(null);
  /** Source lines of the outline headings whose chapter is followed while reading; empty: not followed. */
  readonly headingLines = input<readonly number[]>([]);

  /** A link was clicked: a note path, `note://` link or web address, to open. */
  readonly linkClicked = output<string>();
  /** The user scrolled the preview: first source line now at the top (1-based, fractional). */
  readonly scrolledToLine = output<number>();
  /** Chapter being read (index in `headingLines`), each time it changes. */
  readonly activeHeadingChange = output<number>();

  /** Set while the preview scrolls because of the editor: that scroll must not be echoed back. */
  private syncing = false;
  /** Last search request scrolled to: the highlights are redrawn after each rendering, the scroll happens once. */
  private revealed: PreviewFind | null = null;
  /** Offsets of the headings in the scrolled content: measured once, kept until the rendering or its size changes. */
  private headingTops: number[] | null = null;
  /** Last chapter told; null forces the next one (new rendering or headings: the indexes mean something else). */
  private toldHeading: number | null = null;
  /** Frame in which the chapter is looked up: at most once per frame, however many scroll events. */
  private headingFrame: number | null = null;

  private readonly renderer = computed(() => createMarkdownRenderer({ linkStatus: this.linkStatus(), t: this.i18n.t }));

  /** Sanitized by DOMPurify in renderMarkdown, so Angular's own (stricter) sanitizer must not strip the classes and inputs. */
  protected readonly html = computed<SafeHtml>(() => this.sanitizer.bypassSecurityTrustHtml(renderMarkdown(this.renderer(), this.content(), this.resolveImage() ?? undefined)));

  constructor() {
    effect(() => {
      const request = this.scrollRequest();

      if (request) {
        this.scrollToLine(request.line);
      }
    });

    afterRenderEffect(() => {
      this.html();
      this.theme.theme();
      const root = this.host.nativeElement;
      // Every theme block of styles.scss sets color-scheme: the diagrams follow it
      const dark = getComputedStyle(document.documentElement).colorScheme.includes('dark');
      // The code coloring replaces the text nodes of the blocks: their highlights are drawn again
      void highlightCodeBlocks(root).then(() => this.paintFind(this.find()));
      void renderMermaidBlocks(root, dark, this.i18n.t('editor.preview.diagramLoading'));
    });

    afterRenderEffect(() => {
      this.html();
      this.paintFind(this.find());
    });

    afterRenderEffect(() => {
      this.html();
      this.headingLines();
      this.headingTops = null;
      this.toldHeading = null;
      this.scheduleHeadingLookup();
    });

    const root = this.host.nativeElement;
    const onScroll = (): void => this.scheduleHeadingLookup();
    // Images loaded, diagrams drawn, width changed: the headings moved
    const resize = new ResizeObserver(() => {
      this.headingTops = null;
      this.scheduleHeadingLookup();
    });
    root.addEventListener('scroll', onScroll, { passive: true });
    afterNextRender(() => {
      const article = root.querySelector('.markdown-body');

      if (article) {
        resize.observe(article);
      }
    });

    inject(DestroyRef).onDestroy(() => {
      this.paintFind(null);
      root.removeEventListener('scroll', onScroll);
      resize.disconnect();

      if (this.headingFrame !== null) {
        cancelAnimationFrame(this.headingFrame);
      }
    });
  }

  /** Scrolls so that `line` (1-based, may be fractional) of the source is at the top, between the blocks around it. */
  scrollToLine(line: number): void {
    const blocks = this.blocks();
    let index = blocks.findIndex(block => block.line > line) - 1;

    if (index === -2) {
      index = blocks.length - 1;
    }

    const current = blocks[Math.max(0, index)];

    if (!current) {
      return;
    }

    const next = blocks[index + 1];
    const ratio = next && index >= 0 ? (line - current.line) / (next.line - current.line) : 0;
    const top = next ? current.top + ratio * (next.top - current.top) : current.top;

    this.syncing = true;
    this.host.nativeElement.scrollTop = Math.max(0, top - SYNC_TOP_MARGIN_PX);
  }

  /** Text selected in the rendering, when it can be a search term (on one line); empty otherwise. */
  selectedText(): string {
    const selection = document.getSelection();

    if (!selection || selection.isCollapsed || !this.host.nativeElement.contains(selection.anchorNode)) {
      return '';
    }

    const text = selection.toString().trim();
    return text.includes('\n') ? '' : text;
  }

  /** Current scroll of the preview (px), to come back to it later with `restoreScroll`. */
  scrollTop(): number {
    return this.host.nativeElement.scrollTop;
  }

  restoreScroll(top: number): void {
    this.syncing = true;
    this.host.nativeElement.scrollTop = top;
  }

  protected onScroll(): void {
    if (this.syncing) {
      this.syncing = false;
      return;
    }

    const scrollTop = this.host.nativeElement.scrollTop + SYNC_TOP_MARGIN_PX;
    const blocks = this.blocks();
    const index = blocks.findIndex(block => block.top > scrollTop) - 1;
    const current = blocks[index >= 0 ? index : blocks.length - 1];

    if (!current) {
      return;
    }

    const next = blocks[index + 1];
    const ratio = next && index >= 0 ? (scrollTop - current.top) / Math.max(1, next.top - current.top) : 0;
    this.scrolledToLine.emit(next && index >= 0 ? current.line + ratio * (next.line - current.line) : current.line);
  }

  protected onClick(event: MouseEvent): void {
    const link = (event.target as HTMLElement).closest<HTMLAnchorElement>('a[href]');

    if (!link) {
      return;
    }

    // Never a navigation of the app page itself: links are opened by the app (footnote links have no data-href)
    event.preventDefault();
    const href = link.dataset['href'] ?? link.getAttribute('href') ?? '';

    // Anchors (footnotes, headings) stay inside the preview
    if (href.startsWith('#')) {
      this.scrollToAnchor(linkAnchor(href) ?? '');
      return;
    }

    this.linkClicked.emit(href);
  }

  private scheduleHeadingLookup(): void {
    if (this.headingFrame === null) {
      this.headingFrame = requestAnimationFrame(() => this.lookUpHeading());
    }
  }

  /** Tells the chapter being read, by the same rule as the editor (see findActiveHeading). */
  private lookUpHeading(): void {
    this.headingFrame = null;
    const lines = this.headingLines();

    if (!lines.length) {
      return;
    }

    const root = this.host.nativeElement;
    this.headingTops ??= this.measureHeadingTops(lines);
    const atBottom = root.scrollTop > 0 && root.scrollTop + root.clientHeight >= root.scrollHeight - 2;
    const active = findActiveHeading(this.headingTops, root.scrollTop, atBottom);

    if (active !== this.toldHeading) {
      this.toldHeading = active;
      this.activeHeadingChange.emit(active);
    }
  }

  /** Offset in the scrolled content of the block of each heading: only those blocks are measured. */
  private measureHeadingTops(lines: readonly number[]): number[] {
    const root = this.host.nativeElement;
    const origin = root.getBoundingClientRect().top - root.scrollTop;
    const blocks = this.blockElements();
    const indexes = headingBlockIndexes(blocks.map(block => Number(block.dataset['line'])), lines);

    return indexes.map(index => {
      const block = blocks[index];
      return block ? block.getBoundingClientRect().top - origin : 0;
    });
  }

  /** Highlights the matches of the search, then brings the current one into view once per request. */
  private paintFind(find: PreviewFind | null): void {
    const article = this.host.nativeElement.querySelector<HTMLElement>('.markdown-body');
    const ranges = find && article ? findRanges(article, find.options) : [];
    const current = find ? this.currentRange(find, ranges) : null;

    // CSS Custom Highlight API: no element added to the rendering, which Angular owns (innerHTML); older webviews only scroll
    if ('highlights' in CSS) {
      CSS.highlights.set(FIND_HIGHLIGHT, new Highlight(...ranges.filter(range => range !== current)));
      CSS.highlights.set(FIND_CURRENT_HIGHLIGHT, current ? new Highlight(current) : new Highlight());
    }

    if (!find?.reveal || find === this.revealed || find.current === -1) {
      return;
    }

    this.revealed = find;

    if (current) {
      const root = this.host.nativeElement;
      const offset = current.getBoundingClientRect().top - root.getBoundingClientRect().top;
      root.scrollTop += offset - root.clientHeight / 2;
    }
    else {
      this.scrollToLine(find.lines[find.current]!);
    }
  }

  /** Occurrence of the preview standing for the current match: same rank in the block of its source line. */
  private currentRange(find: PreviewFind, ranges: readonly Range[]): Range | null {
    const line = find.lines[find.current];

    if (line === undefined) {
      return null;
    }

    const blocks = this.blockElements();
    // The blocks are in source order: the last one starting at or before the line holds it
    const index = blocks.filter(block => Number(block.dataset['line']) <= line).length - 1;
    const block = blocks[index];

    if (!block) {
      return null;
    }

    const end = Number(blocks[index + 1]?.dataset['line'] ?? Infinity);
    const inBlock = ranges.filter(range => block.contains(range.startContainer));
    const rank = rankInBlock(find.lines, find.current, Number(block.dataset['line']), end);
    return inBlock[Math.min(rank, inBlock.length - 1)] ?? null;
  }

  /** Brings a heading (by its anchor or its text, see headingSlug) or a footnote (by its id) to the top. */
  private scrollToAnchor(anchor: string): void {
    const root = this.host.nativeElement;
    const heading = root.querySelector(`[data-anchor="${CSS.escape(headingSlug(anchor))}"]`);
    (heading ?? root.querySelector(`[id="${CSS.escape(anchor)}"]`))?.scrollIntoView({ block: 'start' });
  }

  private blockElements(): HTMLElement[] {
    return Array.from(this.host.nativeElement.querySelectorAll<HTMLElement>(BLOCK_SELECTOR));
  }

  /** Top-level blocks with their source line and their offset in the scrolled content. */
  private blocks(): { line: number; top: number }[] {
    const root = this.host.nativeElement;
    const origin = root.getBoundingClientRect().top - root.scrollTop;

    return this.blockElements().map(element => ({
      line: Number(element.dataset['line']),
      top: element.getBoundingClientRect().top - origin,
    }));
  }
}
