import { describe, it, expect } from 'vitest';
import { EditorState } from '@codemirror/state';
import { ensureSyntaxTree } from '@codemirror/language';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import {
  anchorMatches, createHeadingSlugger, headingSlug, headingSourceText, linkAnchor,
} from '../src/app/core/utils/heading-anchor.util';
import { findHeadingByAnchor } from '../src/app/shared/codemirror/outline';

function stateOf(doc: string): EditorState {
  const state = EditorState.create({ doc, extensions: [markdown({ base: markdownLanguage })] });
  ensureSyntaxTree(state, state.doc.length, 5000);
  return state;
}

describe('headingSlug', () => {
  it('lowercases, drops punctuation and turns spaces into dashes like GitHub', () => {
    expect(headingSlug('Getting Started!')).toBe('getting-started');
    expect(headingSlug('API (v2) — usage')).toBe('api-v2--usage');
    expect(headingSlug('snake_case-and-dash')).toBe('snake_case-and-dash');
  });

  it('keeps accented letters and digits', () => {
    expect(headingSlug('Étape 2 : Démarrage')).toBe('étape-2--démarrage');
  });
});

describe('createHeadingSlugger', () => {
  it('suffixes the repeated anchors', () => {
    const slugOf = createHeadingSlugger();
    expect([slugOf('Usage'), slugOf('Usage'), slugOf('usage'), slugOf('Usage 1')]).toEqual(['usage', 'usage-1', 'usage-2', 'usage-1-1']);
  });
});

describe('headingSourceText', () => {
  it('keeps the label of the links and drops images, tags, emphasis underscores and the [!NEW] marker', () => {
    expect(headingSourceText('See [the guide](guide.md) ![logo](a.png)<br>')).toBe('See the guide ');
    expect(headingSourceText('_Intro_ to snake_case')).toBe('Intro to snake_case');
    expect(headingSourceText('[!NEW] Export')).toBe(' Export');
  });
});

describe('linkAnchor', () => {
  it('returns the fragment, percent-decoded', () => {
    expect(linkAnchor('#install')).toBe('install');
    expect(linkAnchor('guide.md#d%C3%A9marrage')).toBe('démarrage');
    expect(linkAnchor('<my guide.md#step 1>')).toBe('step 1');
  });

  it('returns null without a fragment', () => {
    expect(linkAnchor('guide.md')).toBeNull();
    expect(linkAnchor('guide.md#')).toBeNull();
  });

  it('keeps a fragment that is not percent-encoded', () => {
    expect(linkAnchor('#100%')).toBe('100%');
  });
});

describe('anchorMatches', () => {
  it('accepts the anchor as well as the text of the heading', () => {
    expect(anchorMatches('getting-started', 'getting-started')).toBe(true);
    expect(anchorMatches('Getting Started', 'getting-started')).toBe(true);
    expect(anchorMatches('getting', 'getting-started')).toBe(false);
  });
});

describe('findHeadingByAnchor', () => {
  const doc = '# Title\n\nText\n\n## Getting **started**\n\n```\n# not a heading\n```\n\n#### Deep `code`\n\nSetext\n------\n\n## Title\n';

  it('finds a heading of any level by its anchor', () => {
    const state = stateOf(doc);
    expect(findHeadingByAnchor(state, 'getting-started')).toBe(doc.indexOf('## Getting'));
    expect(findHeadingByAnchor(state, 'deep-code')).toBe(doc.indexOf('#### Deep'));
    expect(findHeadingByAnchor(state, 'setext')).toBe(doc.indexOf('Setext'));
  });

  it('reaches a repeated heading by its suffixed anchor', () => {
    const state = stateOf(doc);
    expect(findHeadingByAnchor(state, 'title')).toBe(0);
    expect(findHeadingByAnchor(state, 'title-1')).toBe(doc.lastIndexOf('## Title'));
  });

  it('ignores the code blocks and unknown anchors', () => {
    const state = stateOf(doc);
    expect(findHeadingByAnchor(state, 'not-a-heading')).toBeNull();
    expect(findHeadingByAnchor(state, 'missing')).toBeNull();
  });
});
