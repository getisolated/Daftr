import { describe, it, expect } from 'vitest';
import { EditorState } from '@codemirror/state';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { ensureSyntaxTree } from '@codemirror/language';
import { linkTargetAt } from '../src/app/shared/codemirror/link-navigation';

function stateOf(doc: string): EditorState {
  const state = EditorState.create({ doc, extensions: [markdown({ base: markdownLanguage })] });
  ensureSyntaxTree(state, state.doc.length, 5000);
  return state;
}

describe('linkTargetAt', () => {
  const doc = 'see [the plan](Work/Roadmap) and [other](<My folder/Note>) or note://12 then https://x.io end';

  it('gives the target of a markdown link, from its label or from its target', () => {
    expect(linkTargetAt(stateOf(doc), doc.indexOf('plan'))).toBe('Work/Roadmap');
    expect(linkTargetAt(stateOf(doc), doc.indexOf('Roadmap'))).toBe('Work/Roadmap');
  });

  it('keeps the angle brackets of a target with spaces, for the resolver to unwrap', () => {
    expect(linkTargetAt(stateOf(doc), doc.indexOf('other'))).toBe('<My folder/Note>');
  });

  it('finds a note:// link written as plain text', () => {
    expect(linkTargetAt(stateOf(doc), doc.indexOf('note://12') + 3)).toBe('note://12');
  });

  it('finds a bare web address', () => {
    expect(linkTargetAt(stateOf(doc), doc.indexOf('https://x.io') + 4)).toBe('https://x.io');
  });

  it('gives nothing out of a link', () => {
    expect(linkTargetAt(stateOf(doc), doc.indexOf('and'))).toBeNull();
  });
});
