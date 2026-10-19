import { LanguageDescription } from '@codemirror/language';
import { languages } from '@codemirror/language-data';
import { classHighlighter, highlightCode } from '@lezer/highlight';
import type { Mermaid } from 'mermaid';

const HIGHLIGHTED = 'highlighted';
const MAX_CACHED_DIAGRAMS = 50;

/**
 * Colors the fenced code blocks of the preview with the same Lezer parsers as the editor (`tok-*` classes, styled
 * from the theme tokens). Languages load on demand; unknown ones stay plain.
 */
export async function highlightCodeBlocks(root: HTMLElement): Promise<void> {
  const blocks = root.querySelectorAll<HTMLElement>('pre > code[class*="language-"]');

  for (const code of Array.from(blocks)) {
    if (code.dataset[HIGHLIGHTED]) {
      continue;
    }

    const name = /language-(?<name>\S+)/.exec(code.className)?.groups?.['name'];
    const description = name ? LanguageDescription.matchLanguageName(languages, name, true) : null;

    if (!description) {
      continue;
    }

    const support = description.support ?? await description.load();
    const text = code.textContent ?? '';
    const fragment = document.createDocumentFragment();

    highlightCode(
      text,
      support.language.parser.parse(text),
      classHighlighter,
      (token, classes) => {
        const span = document.createElement('span');
        span.className = classes;
        span.textContent = token;
        fragment.appendChild(classes ? span : document.createTextNode(token));
      },
      () => fragment.appendChild(document.createTextNode('\n')),
    );

    code.replaceChildren(fragment);
    code.dataset[HIGHLIGHTED] = '1';
  }
}

/** Same font as the preview text, so that mermaid measures the labels with the font they are drawn in. */
const DIAGRAM_FONT = '-apple-system, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif';

const DARK_DIAGRAM = {
  fontFamily: DIAGRAM_FONT,
  background: 'transparent',
  primaryColor: '#2d333b',
  primaryBorderColor: '#3d6a8f',
  primaryTextColor: '#c9d1d9',
  secondaryColor: '#263a4d',
  tertiaryColor: '#22272e',
  lineColor: '#4b86b4',
  textColor: '#c9d1d9',
  edgeLabelBackground: '#22272e',
  noteBkgColor: '#3b3a2a',
  noteTextColor: '#e3d9a8',
};

const LIGHT_DIAGRAM = {
  fontFamily: DIAGRAM_FONT,
  background: 'transparent',
  primaryColor: '#ddf4ff',
  primaryBorderColor: '#54aeff',
  primaryTextColor: '#1f2328',
  secondaryColor: '#eaeef2',
  tertiaryColor: '#f6f8fa',
  lineColor: '#0969da',
  textColor: '#1f2328',
  edgeLabelBackground: '#ffffff',
  noteBkgColor: '#fff8c5',
  noteTextColor: '#1f2328',
};

let mermaidLoader:Promise<Mermaid> | null = null;
let diagramCounter = 0;
/** SVG of the diagrams already drawn, by theme and source: the preview re-renders on every edit. */
const diagramCache = new Map<string, string>();

/** mermaid is heavy (~1 MB): only loaded once a note actually holds a diagram. */
function loadMermaid(): Promise<Mermaid> {
  mermaidLoader ??= import('mermaid').then(module => module.default);
  return mermaidLoader;
}

async function drawDiagram(mermaid: Mermaid, source: string, dark: boolean): Promise<string> {
  const key = `${dark ? 'dark' : 'light'}\n${source}`;
  const cached = diagramCache.get(key);

  if (cached !== undefined) {
    return cached;
  }

  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    theme: 'base',
    themeVariables: dark ? DARK_DIAGRAM : LIGHT_DIAGRAM,
    flowchart: { curve: 'basis' },
  });
  // Labels are measured with the preview font: without it they get clipped once rendered with another one
  await document.fonts.ready;
  // Validated first: a failed render leaves error markup behind in the page
  await mermaid.parse(source);
  const { svg } = await mermaid.render(`evn-mermaid-${++diagramCounter}`, source);

  if (diagramCache.size >= MAX_CACHED_DIAGRAMS) {
    const oldest = diagramCache.keys().next().value;
    if (oldest !== undefined) {
      diagramCache.delete(oldest);
    }
  }
  diagramCache.set(key, svg);
  return svg;
}

/** Diagrams are drawn one at a time: mermaid keeps its theme in a global configuration. */
let drawQueue: Promise<void> = Promise.resolve();

function createDiagram(source: string, line: string | undefined): HTMLElement {
  const diagram = document.createElement('div');
  diagram.className = 'mermaid-diagram';
  diagram.dataset['mermaidSource'] = source;

  if (line) {
    diagram.dataset['line'] = line;
  }

  return diagram;
}

/**
 * Replaces the mermaid sources of the preview with their diagram (a loading indicator until it is drawn), or with the
 * syntax error. Diagrams already drawn are redrawn when the theme changed.
 */
export async function renderMermaidBlocks(root: HTMLElement, dark: boolean, loadingLabel = ''): Promise<void> {
  const theme = dark ? 'dark' : 'light';
  const pending: HTMLElement[] = [];

  for (const element of Array.from(root.querySelectorAll<HTMLElement>('pre[data-mermaid], .mermaid-diagram[data-mermaid-source]'))) {
    const isSource = element instanceof HTMLPreElement;
    const source = isSource ? element.textContent ?? '' : element.dataset['mermaidSource'] ?? '';
    let diagram = element;

    if (!isSource && element.dataset['mermaidTheme'] === theme) {
      continue;
    }

    if (isSource) {
      diagram = createDiagram(source, element.dataset['line']);
      const cached = diagramCache.get(`${theme}
${source}`);

      if (cached === undefined) {
        diagram.classList.add('mermaid-loading');
        diagram.textContent = loadingLabel;
      } else {
        diagram.innerHTML = cached;
        diagram.dataset['mermaidTheme'] = theme;
      }

      element.replaceWith(diagram);

      if (cached !== undefined) {
        continue;
      }
    }

    // Claimed now: a concurrent call must not draw it again
    diagram.dataset['mermaidTheme'] = theme;
    pending.push(diagram);
  }

  if (pending.length === 0) {
    return;
  }

  const mermaid = await loadMermaid();

  for (const diagram of pending) {
    drawQueue = drawQueue.then(async () => {
      if (!diagram.isConnected) {
        return;
      }

      try {
        // mermaid's strict security level sanitizes the SVG it produces
        diagram.innerHTML = await drawDiagram(mermaid, diagram.dataset['mermaidSource'] ?? '', dark);
        diagram.classList.remove('mermaid-error', 'mermaid-loading');
      } catch (error) {
        diagram.classList.remove('mermaid-loading');
        diagram.classList.add('mermaid-error');
        diagram.textContent = `Mermaid: ${error instanceof Error ? error.message : String(error)}`;
      }
    });
  }

  await drawQueue;
}
