import { describe, it, expect } from 'vitest';
import { TranslationKey } from '../src/app/core/i18n/translations';
import { TranslationParams, translate } from '../src/app/core/i18n/translate';
import {
  buildNoteGraph,
  extractLinkExcerpt,
  extractNoteLinks,
  findBacklinks,
  GRAPH_LARGE_THRESHOLD,
  LinkableNote,
  replaceLinkTarget,
  retargetNoteLinks,
} from '../src/app/core/utils/note-links.util';

const t = (key: TranslationKey, params?: TranslationParams): string => translate('en', key, params);
const tFr = (key: TranslationKey, params?: TranslationParams): string => translate('fr', key, params);

function note(id: number, content = '', folderId: number | null = null): LinkableNote {
  return { id, title: `Note ${id}`, content, folderId };
}

describe('retargetNoteLinks', () => {
  const moves = new Map([['work/plan', 'Archive/Work/Plan'], ['ideas', 'Work/My ideas']]);

  it('points the links to the moved notes at their new path, whatever their case or form', () => {
    const content = 'See [plan](Work/Plan), [again](./work/plan.md "Plan"), [ideas](<Ideas>) and [encoded](Work/Plan%20B).';

    expect(retargetNoteLinks(content, moves))
      .toBe('See [plan](Archive/Work/Plan), [again](./Archive/Work/Plan.md "Plan"), [ideas](<Work/My ideas>) and [encoded](Work/Plan%20B).');
  });

  it('is null when no link leads to a moved note', () => {
    expect(retargetNoteLinks('[x](Other) ![img](Work/Plan) https://example.com', moves)).toBeNull();
  });
});

describe('replaceLinkTarget', () => {
  it('replaces the target, never a label reading like it', () => {
    expect(replaceLinkTarget('[Work/Plan](Work/Plan)', 'Work/Plan', 'Archive/Plan')).toBe('[Work/Plan](Archive/Plan)');
  });

  it('wraps a target holding spaces in angle brackets', () => {
    expect(replaceLinkTarget('[a](b "t")', 'b', 'My note')).toBe('[a](<My note> "t")');
  });
});

describe('extractNoteLinks', () => {
  it('finds the linked ids once each, in order', () => {
    expect(extractNoteLinks('a note://3 b note://1 c note://3')).toEqual([3, 1]);
  });

  it('ignores other links and text', () => {
    expect(extractNoteLinks('https://x.io note://abc note:// plain')).toEqual([]);
  });
});

describe('extractLinkExcerpt', () => {
  it('keeps the line around the link', () => {
    expect(extractLinkExcerpt('first\nsee note://2 for more\nlast', 2)).toBe('see note://2 for more');
  });

  it('cuts long lines with an ellipsis on each side', () => {
    const line = `${'a'.repeat(100)} note://2 ${'b'.repeat(100)}`;
    const excerpt = extractLinkExcerpt(line, 2);

    expect(excerpt.startsWith('…')).toBe(true);
    expect(excerpt.endsWith('…')).toBe(true);
    expect(excerpt).toContain('note://2');
  });

  it('does not take note://12 for note://1', () => {
    expect(extractLinkExcerpt('note://12 then note://1', 1)).toBe('note://12 then note://1');
    expect(extractLinkExcerpt('only note://12', 1)).toBe('');
  });
});

describe('findBacklinks', () => {
  it('lists the notes linking to the target, sorted by title, with an excerpt', () => {
    const notes = [note(1), note(3, 'to note://1 here'), note(2, 'also note://1')];

    expect(findBacklinks(notes, 1, t)).toEqual([
      { noteId: 2, title: 'Note 2', excerpt: 'also note://1' },
      { noteId: 3, title: 'Note 3', excerpt: 'to note://1 here' },
    ]);
  });

  it('never lists the note itself, nor a note linking to an id that only starts like the target', () => {
    const notes = [note(1, 'self note://1'), note(2, 'note://10')];

    expect(findBacklinks(notes, 1, t)).toEqual([]);
  });
});

describe('titles without a name', () => {
  it('are shown with the placeholder of the language', () => {
    const untitled: LinkableNote[] = [{ id: 1, title: '', content: '', folderId: null }, { id: 2, title: '', content: 'note://1', folderId: null }];
    expect(findBacklinks(untitled, 1, tFr).map(b => b.title)).toEqual(['Sans titre']);
    expect(buildNoteGraph(untitled, t).nodes.map(n => n.title)).toEqual(['Untitled', 'Untitled']);
  });
});

describe('buildNoteGraph', () => {
  it('links the notes and counts degrees', () => {
    const graph = buildNoteGraph([note(1, 'note://2'), note(2), note(3)], t);

    expect(graph.links).toEqual([{ source: 1, target: 2 }]);
    expect(graph.nodes.map(n => [n.id, n.degree])).toEqual([[1, 1], [2, 1], [3, 0]]);
  });

  it('keeps both directions of a cycle and ignores self links', () => {
    const graph = buildNoteGraph([note(1, 'note://2 note://1'), note(2, 'note://1')], t);

    expect(graph.links).toEqual([{ source: 1, target: 2 }, { source: 2, target: 1 }]);
  });

  it('draws a missing node for a link to a deleted note', () => {
    const graph = buildNoteGraph([note(1, 'note://99')], t);

    expect(graph.nodes.find(n => n.id === 99)).toMatchObject({ missing: true, degree: 1 });
    expect(graph.links).toEqual([{ source: 1, target: 99 }]);
  });

  it('filters by folders without turning outside notes into missing ones', () => {
    const notes = [note(1, 'note://2 note://3 note://4', 10), note(2, '', 20), note(3, '', 10), note(4, '', 11), note(5)];
    const graph = buildNoteGraph(notes, t, { folderIds: new Set([10, 11]) });

    expect(graph.nodes.map(n => n.id)).toEqual([1, 3, 4]);
    expect(graph.links).toEqual([{ source: 1, target: 3 }, { source: 1, target: 4 }]);
  });

  it('can hide the orphans', () => {
    const graph = buildNoteGraph([note(1, 'note://2'), note(2), note(3)], t, { hideOrphans: true });

    expect(graph.nodes.map(n => n.id)).toEqual([1, 2]);
  });

  describe('large collections', () => {
    // A chain 1→2→…→n: the notes far from the focus are cut
    const chain = Array.from({ length: GRAPH_LARGE_THRESHOLD + 10 }, (_, i) => note(i + 1, `note://${i + 2}`));
    const orphans = [note(9001), note(9002)];

    it('keeps the notes within two links of the focus', () => {
      const graph = buildNoteGraph([...chain, ...orphans], t, { focusId: 100 });

      expect(graph.nodes.map(n => n.id).sort((a, b) => a - b)).toEqual([98, 99, 100, 101, 102]);
    });

    it('keeps only the connected notes without focus', () => {
      const graph = buildNoteGraph([...chain, ...orphans], t);

      expect(graph.nodes.some(n => n.id === 9001)).toBe(false);
      expect(graph.nodes.some(n => n.id === 1)).toBe(true);
    });
  });
});
