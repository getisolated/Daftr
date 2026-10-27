import createMarkdownIt from 'markdown-it';
import type { MarkdownIt, RendererRule, StateCore, Token } from 'markdown-it';
import footnote from 'markdown-it-footnote';
import DOMPurify from 'dompurify';
import { LinkStatus } from '../../core/utils/note-paths.util';
import { createHeadingSlugger } from '../../core/utils/heading-anchor.util';
import { Translator } from '../../core/i18n/translate';
import { TranslationKey } from '../../core/i18n/translations';

/** Everything the preview needs to know about the app to render a note. */
export interface MarkdownRenderContext {
  /** Whether a link target exists: local links pointing to nothing are drawn as broken. */
  linkStatus: (target: string) => LinkStatus;
  /** Texts generated in the page (alert titles, tooltips), in the language of the app. */
  t: Translator;
}

type AlertType = 'note' | 'tip' | 'important' | 'warning' | 'caution';
const ALERT_MARKER = /^\[!(?<type>note|tip|important|warning|caution)\]\s*/i;
const TASK_MARKER = /^\[(?<state>[ xX])\]\s+/;

/** Source line (1-based) of each block, so the preview can follow the editor scroll and the other way round. */
function sourceLines(md: MarkdownIt): void {
  md.core.ruler.push('source_lines', (state: StateCore) => {
    for (const token of state.tokens) {
      if (token.map && token.nesting === 1 && token.level === 0) {
        token.attrSet('data-line', String(token.map[0] + 1));
      }
    }
  });
}

/** GitHub task lists: `- [ ]` / `- [x]` items get a (read-only) checkbox. */
function taskLists(md: MarkdownIt): void {
  md.core.ruler.push('task_lists', (state: StateCore) => {
    const tokens = state.tokens;

    for (let i = 2; i < tokens.length; i++) {
      const inline = tokens[i] as Token;
      const item = tokens[i - 2] as Token;
      const match = inline.type === 'inline' && item.type === 'list_item_open' ? TASK_MARKER.exec(inline.content) : null;

      if (!match?.groups) {
        continue;
      }

      const checked = match.groups['state'] !== ' ';
      item.attrJoin('class', 'task-list-item');
      const first = inline.children?.[0];

      if (first?.type === 'text') {
        first.content = first.content.replace(TASK_MARKER, '');
      }

      const checkbox = new state.Token('html_inline', '', 0);
      checkbox.content = `<input type="checkbox" class="task-list-item-checkbox" disabled${checked ? ' checked' : ''}> `;
      inline.children?.unshift(checkbox);
    }
  });
}

/** GitHub alerts: a quote starting with `[!NOTE]` (TIP, IMPORTANT, WARNING, CAUTION) becomes a titled callout. */
function alerts(md: MarkdownIt, context: MarkdownRenderContext): void {
  md.core.ruler.push('alerts', (state: StateCore) => {
    const tokens = state.tokens;

    for (let i = 0; i < tokens.length - 2; i++) {
      const open = tokens[i] as Token;
      const inline = tokens[i + 2] as Token;
      const match = open.type === 'blockquote_open' && inline.type === 'inline' ? ALERT_MARKER.exec(inline.content) : null;
      const type = match?.groups?.['type']?.toLowerCase() as AlertType | undefined;

      if (!type) {
        continue;
      }

      open.attrJoin('class', `markdown-alert markdown-alert-${type}`);
      inline.content = inline.content.replace(ALERT_MARKER, '');
      const first = inline.children?.[0];

      if (first?.type === 'text') {
        first.content = first.content.replace(ALERT_MARKER, '');
      }

      // The marker may sit alone on its line: drop the soft break that followed it
      if (inline.children?.[0]?.content === '' && inline.children[1]?.type === 'softbreak') {
        inline.children.splice(0, 2);
      }

      const title = new state.Token('html_block', '', 0);
      title.content = `<p class="markdown-alert-title">${md.utils.escapeHtml(context.t(`editor.alert.${type}` as TranslationKey))}</p>\n`;
      tokens.splice(i + 1, 0, title);
    }
  });
}

/**
 * Headings get their GitHub anchor, so that `#chapter` links reach them. `data-anchor` too: DOMPurify drops an `id`
 * that names a property of `document` (`title`, `images`…), the preview looks the anchors up by both.
 */
function headingAnchors(md: MarkdownIt): void {
  md.core.ruler.push('heading_anchors', (state: StateCore) => {
    const slugOf = createHeadingSlugger();
    const tokens = state.tokens;

    for (let i = 0; i < tokens.length - 1; i++) {
      const open = tokens[i] as Token;
      const inline = tokens[i + 1] as Token;

      if (open.type !== 'heading_open' || inline.type !== 'inline') {
        continue;
      }

      // The text as shown: markup, images and the badge of `[!NEW]` (already replaced) are no part of it
      const text = (inline.children ?? []).filter(child => child.type === 'text' || child.type === 'code_inline').map(child => child.content).join('');
      const slug = slugOf(text);
      open.attrSet('id', slug);
      open.attrSet('data-anchor', slug);
    }
  });
}

/** Links keep their raw target (the preview follows them itself) and get a class telling what they point to. */
function linkTargets(md: MarkdownIt, context: MarkdownRenderContext): void {
  const fallback: RendererRule = md.renderer.rules['link_open'] ?? ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));

  md.renderer.rules['link_open'] = (tokens, idx, options, env, self) => {
    const token = tokens[idx] as Token;
    const href = String(token.attrGet('href') ?? '');
    const status = context.linkStatus(href);

    token.attrSet('data-href', href);
    token.attrJoin('class', status === 'broken' ? 'link-broken' : `link-${status}`);

    if (status === 'broken') {
      token.attrSet('title', context.t('editor.link.broken'));
    }

    return fallback(tokens, idx, options, env, self);
  };
}

const NEW_MARKER = /^\s*\[!NEW\][ \t]*/;

/**
 * `[!NEW]` at the start of a block becomes a « New » badge (convention of the CAP Vision documentation). A quote it
 * opens (`> [!NEW]`) or a paragraph of its own turns into a framed box with the badge on its border; in a heading, a
 * list item or a table cell, the badge stays inline.
 */
function newBadges(md: MarkdownIt, context: MarkdownRenderContext): void {
  md.core.ruler.push('new_badges', (state: StateCore) => {
    const tokens = state.tokens;

    for (let i = 1; i < tokens.length; i++) {
      const inline = tokens[i] as Token;
      const first = inline.type === 'inline' ? inline.children?.[0] : undefined;

      if (!inline.children || first?.type !== 'text' || !NEW_MARKER.test(first.content)) {
        continue;
      }

      first.content = first.content.replace(NEW_MARKER, '');
      // `[!NEW]` alone on its line, the content below: the line break goes with the marker
      if (first.content === '' && inline.children[1]?.type === 'softbreak') {
        inline.children.splice(0, 2);
      }

      const badge = new state.Token('html_inline', '', 0);
      badge.content = `<span class="badge-new">${md.utils.escapeHtml(context.t('editor.alert.new'))}</span>`;
      inline.children.unshift(badge);
      frameNewBlock(tokens, i);
    }
  });
}

/** The box around a `[!NEW]` block: the quote it opens, or its own paragraph (not in a list item nor a cell). */
function frameNewBlock(tokens: Token[], inlineIndex: number): void {
  const open = tokens[inlineIndex - 1];
  const parent = tokens[inlineIndex - 2];

  if (open?.type !== 'paragraph_open' || open.hidden) {
    return;
  }
  if (parent?.type === 'blockquote_open') {
    parent.attrJoin('class', 'new-block');
  } else if (parent?.type !== 'list_item_open') {
    open.attrJoin('class', 'new-block');
  }
}

/** Mermaid blocks are left as their source, flagged: the preview draws them once mermaid is loaded. */
function mermaidFences(md: MarkdownIt): void {
  const fallback: RendererRule = md.renderer.rules['fence'] ?? ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));

  md.renderer.rules['fence'] = (tokens, idx, options, env, self) => {
    const token = tokens[idx] as Token;

    if (token.info.trim().toLowerCase() !== 'mermaid') {
      return fallback(tokens, idx, options, env, self);
    }

    const line = token.map ? ` data-line="${token.map[0] + 1}"` : '';
    return `<pre class="mermaid-source" data-mermaid${line}>${md.utils.escapeHtml(token.content)}</pre>\n`;
  };
}

/** A configured markdown-it: GFM-like (tables, strikethrough, autolinks, task lists, footnotes, alerts). */
export function createMarkdownRenderer(context: MarkdownRenderContext): MarkdownIt {
  const md = createMarkdownIt({ html: true, linkify: true, breaks: false });

  md.use(footnote as unknown as (md: MarkdownIt) => void);
  md.use(sourceLines);
  md.use(taskLists);
  md.use(alerts, context);
  md.use(newBadges, context);
  // After the badges: the `[!NEW]` marker is no part of the anchor
  md.use(headingAnchors);
  md.use(mermaidFences);
  linkTargets(md, context);

  return md;
}

/** Address of an image of the page (`src` as written), `null` to keep it as written. */
export type ImageResolver = (src: string) => string | null;

/**
 * HTML of a note, sanitized: the markdown may come from anywhere (pasted, imported, a file on disk), and raw HTML
 * is allowed in it, so scripts, event handlers and `javascript:` links are stripped before it reaches the page.
 * `resolveImage` rewrites the image addresses (markdown and `<img>` alike), e.g. the relative ones of a collection file.
 */
export function renderMarkdown(md: MarkdownIt, source: string, resolveImage?: ImageResolver): string {
  const rewrite = (node: Element): void => {
    const src = node.tagName === 'IMG' ? node.getAttribute('src') : null;
    const resolved = src && resolveImage ? resolveImage(src) : null;
    if (resolved) {
      node.setAttribute('src', resolved);
    }
  };

  // Hooks are global to DOMPurify: this one only lives for this call
  DOMPurify.addHook('afterSanitizeAttributes', rewrite);
  try {
    return DOMPurify.sanitize(md.render(source), {
      ADD_ATTR: ['data-line', 'data-href', 'data-mermaid', 'data-anchor'],
      FORBID_TAGS: ['style', 'form'],
    });
  } finally {
    DOMPurify.removeHook('afterSanitizeAttributes', rewrite);
  }
}
