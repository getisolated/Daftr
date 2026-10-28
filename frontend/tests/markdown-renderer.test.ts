import { describe, it, expect } from 'vitest';
import { createMarkdownRenderer } from '../src/app/shared/markdown/markdown-renderer';
import { translate } from '../src/app/core/i18n/translate';
import { LinkStatus } from '../src/app/core/utils/note-paths.util';

const statuses: Record<string, LinkStatus> = { 'Folder/Here': 'note', 'Gone': 'broken', 'https://x.dev': 'external' };
const md = createMarkdownRenderer({ linkStatus: target => statuses[target] ?? 'note', t: (key, params) => translate('en', key, params) });
const mdFr = createMarkdownRenderer({ linkStatus: target => statuses[target] ?? 'note', t: (key, params) => translate('fr', key, params) });
const render = (source: string): string => md.render(source);

describe('createMarkdownRenderer', () => {
  it('gives the headings their GitHub anchor, suffixed when repeated, without the markup nor the [!NEW] badge', () => {
    const html = render('# Getting **started**\n\n## [!NEW] Use `cli`\n\n## Getting started');
    expect(html).toContain('<h1 data-line="1" id="getting-started" data-anchor="getting-started">');
    expect(html).toContain('id="use-cli" data-anchor="use-cli"');
    expect(html).toContain('id="getting-started-1" data-anchor="getting-started-1"');
  });

  it('tags each top-level block with its 1-based source line', () => {
    const html = render('# Title\n\nParagraph\n\n- item');
    expect(html).toContain('<h1 data-line="1" id="title" data-anchor="title">');
    expect(html).toContain('<p data-line="3">');
    expect(html).toContain('<ul data-line="5">');
  });

  it('renders GFM tables and strikethrough', () => {
    expect(render('| a | b |\n| - | - |\n| 1 | 2 |')).toContain('<table');
    expect(render('~~gone~~')).toContain('<s>gone</s>');
  });

  it('turns task items into disabled checkboxes', () => {
    const html = render('- [ ] todo\n- [x] done');
    expect(html).toContain('class="task-list-item"');
    expect(html).toContain('<input type="checkbox" class="task-list-item-checkbox" disabled> todo');
    expect(html).toContain('disabled checked> done');
  });

  it('titles the alerts and the broken links in the language of the renderer', () => {
    expect(mdFr.render('> [!WARNING]\n> Careful')).toContain('<p class="markdown-alert-title">Avertissement</p>');
    expect(mdFr.render('> [!TIP]\n> x')).toContain('<p class="markdown-alert-title">Astuce</p>');
    expect(mdFr.render('[x](Gone)')).toContain('title="Cible du lien introuvable"');
  });

  it('turns GitHub alerts into titled callouts', () => {
    const html = render('> [!WARNING]\n> Careful');
    expect(html).toContain('class="markdown-alert markdown-alert-warning"');
    expect(html).toContain('<p class="markdown-alert-title">Warning</p>');
    expect(html).not.toContain('[!WARNING]');
  });

  it('flags links by status and keeps their raw target', () => {
    expect(render('[ok](Folder/Here)')).toContain('class="link-note"');
    expect(render('[x](Gone)')).toMatch(/class="link-broken".*title="Link target not found"/);
    expect(render('[web](https://x.dev)')).toContain('data-href="https://x.dev"');
  });

  it('leaves mermaid blocks as escaped source for the preview to draw', () => {
    const html = render('```mermaid\ngraph TD; A-->B<\n```');
    expect(html).toContain('<pre class="mermaid-source" data-mermaid data-line="1">graph TD; A--&gt;B&lt;\n</pre>');
  });

  it('renders footnotes', () => {
    expect(render('Text[^1]\n\n[^1]: Note')).toContain('class="footnotes"');
  });
});

describe('[!NEW] badges', () => {
  const badge = '<span class="badge-new">New</span>';

  it('frames a quote it opens, with the badge first', () => {
    const html = render('> [!NEW]\n> Synchronised in the background.');
    expect(html).toMatch(/<blockquote[^>]* class="new-block"/);
    expect(html).toContain(`${badge}Synchronised in the background.`);
    expect(html).not.toContain('[!NEW]');
  });

  it('frames a paragraph of its own, marker on the line above or in front', () => {
    expect(render('[!NEW]\nThe app prints by itself.')).toMatch(new RegExp(`<p[^>]* class="new-block"[^>]*>${badge}The app prints by itself.`));
    expect(render('[!NEW] The app prints by itself.')).toMatch(/<p[^>]* class="new-block"/);
  });

  it('keeps the badge inline in headings, list items and table cells', () => {
    expect(render('### [!NEW] Language')).toContain(`${badge}Language</h3>`);
    expect(render('- [!NEW] the **turnover**')).toContain(`<li>${badge}the <strong>turnover</strong></li>`);
    expect(render('| a |\n| - |\n| [!NEW] Amount |')).toContain(`<td>${badge}Amount</td>`);
    expect(render('- item\n\n- [!NEW] loose')).not.toContain('new-block');
  });

  it('translates the badge', () => {
    expect(mdFr.render('### [!NEW] Langue')).toContain('<span class="badge-new">Nouveau</span>');
  });
});
