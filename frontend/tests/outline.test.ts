import { describe, it, expect } from 'vitest';
import { EditorState } from '@codemirror/state';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { extractHeadings, findHeadingByAnchor } from '../src/app/shared/codemirror/outline';

describe('extractHeadings / findHeadingByAnchor', () => {
  // Far beyond what CodeMirror parses when a state is created (a few thousand characters): the headings of the end of a long
  // note must be found anyway, the editor being hidden behind the preview or not scrolled there yet.
  const filler = 'Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n\n'.repeat(5000);
  const doc = `# Start\n\n${filler}## Middle\n\n${filler}### End\n`;
  const state = EditorState.create({ doc, extensions: [markdown({ base: markdownLanguage })] });

  it('lists the headings of the whole document, parsed slice by slice', () => {
    let extraction = extractHeadings(state, 5);
    let slices = 1;

    while (!extraction.complete && slices < 1000) {
      extraction = extractHeadings(state, 5);
      slices++;
    }

    expect(extraction.complete).toBe(true);
    expect(extraction.headings.map(heading => [heading.level, heading.text])).toEqual([
      [1, 'Start'],
      [2, 'Middle'],
      [3, 'End'],
    ]);
    // Numbered as the preview numbers its blocks
    expect(extraction.headings.map(heading => heading.line)).toEqual([1, 10003, 20005]);
  });

  it('finds a heading at the end of a long document', () => {
    expect(findHeadingByAnchor(state, 'end')).toBe(doc.lastIndexOf('### End'));
  });
});
