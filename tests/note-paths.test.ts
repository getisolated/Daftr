import { describe, it, expect } from 'vitest';
import {
  PathFolder, PathNote, createLinkResolver, listPathCompletions, needsAngleBrackets, normalizeLinkTarget, notePath,
} from '../src/app/core/utils/note-paths.util';
import { buildNoteGraph, extractLinkExcerpt, extractNoteLinks, findBacklinks } from '../src/app/core/utils/note-links.util';

// Work(1) > Projects(2) ; Perso(3)
const FOLDERS: PathFolder[] = [
  { id: 1, name: 'Work', parentId: null },
  { id: 2, name: 'Projects', parentId: 1 },
  { id: 3, name: 'Perso', parentId: null },
];

function note(id: number, title: string, folderId: number | null, content = ''): PathNote & { content: string } {
  return { id, title, folderId, content };
}

const NOTES = [
  note(10, 'Roadmap', 2),
  note(11, 'Meeting notes', 1),
  note(12, 'Todo', null),
  note(13, 'Roadmap perso', 3),
];

describe('notePath', () => {
  it('joins the folders and the title', () => {
    expect(notePath(NOTES[0]!, FOLDERS)).toBe('Work/Projects/Roadmap');
    expect(notePath(NOTES[2]!, FOLDERS)).toBe('Todo');
  });

  it('replaces the characters that would break a path', () => {
    expect(notePath(note(1, 'a/b <c>', null), FOLDERS)).toBe('a-b -c-');
  });
});

describe('normalizeLinkTarget', () => {
  it('unwraps angle brackets, decodes and drops ./ and .md', () => {
    expect(normalizeLinkTarget('<Work/Meeting notes>')).toBe('Work/Meeting notes');
    expect(normalizeLinkTarget('Work/Meeting%20notes.md')).toBe('Work/Meeting notes');
    expect(normalizeLinkTarget('./Todo')).toBe('Todo');
  });

  it('refuses what is not a path to a note', () => {
    expect(normalizeLinkTarget('https://x.io')).toBeNull();
    expect(normalizeLinkTarget('note://3')).toBeNull();
    expect(normalizeLinkTarget('#anchor')).toBeNull();
    expect(normalizeLinkTarget('')).toBeNull();
  });

  it('keeps a lone percent sign as typed', () => {
    expect(normalizeLinkTarget('100% done')).toBe('100% done');
  });
});

describe('createLinkResolver', () => {
  const resolve = createLinkResolver(NOTES, FOLDERS);

  it('finds a note by its path, whatever the case', () => {
    expect(resolve('work/projects/ROADMAP')).toBe(10);
    expect(resolve('<Work/Meeting notes>')).toBe(11);
    expect(resolve('Todo')).toBe(12);
  });

  it('does not take a title for a path it is not at', () => {
    expect(resolve('Roadmap')).toBeNull();
    expect(resolve('Work/Roadmap')).toBeNull();
  });
});

describe('listPathCompletions', () => {
  it('lists the root folders then the root notes', () => {
    expect(listPathCompletions('', NOTES, FOLDERS).map(c => c.label)).toEqual(['Perso/', 'Work/', 'Todo']);
  });

  it('walks down a typed directory and filters on the last segment', () => {
    expect(listPathCompletions('Work/', NOTES, FOLDERS).map(c => c.path)).toEqual(['Work/Projects/', 'Work/Meeting notes']);
    expect(listPathCompletions('work/pro', NOTES, FOLDERS).map(c => c.path)).toEqual(['Work/Projects/']);
  });

  it('offers nothing in a folder that does not exist', () => {
    expect(listPathCompletions('Nope/', NOTES, FOLDERS)).toEqual([]);
  });

  it('finds notes by title out of any directory, showing where they are', () => {
    const found = listPathCompletions('road', NOTES, FOLDERS);

    expect(found.map(c => [c.path, c.detail])).toEqual([
      ['Work/Projects/Roadmap', 'Work/Projects'],
      ['Perso/Roadmap perso', 'Perso'],
    ]);
  });
});

describe('needsAngleBrackets', () => {
  it('wants them for spaces and parentheses only', () => {
    expect(needsAngleBrackets('Work/Meeting notes')).toBe(true);
    expect(needsAngleBrackets('a(b)')).toBe(true);
    expect(needsAngleBrackets('Work/Todo')).toBe(false);
  });
});

describe('markdown links as links between notes', () => {
  const resolve = createLinkResolver(NOTES, FOLDERS);

  it('counts [label](path) and [label](<path with spaces>), but not images nor external links', () => {
    const content = '[a](Todo) ![img](Work/Projects/Roadmap) [b](<Work/Meeting notes>) [c](https://x.io) [d](note://13)';

    expect(extractNoteLinks(content, resolve)).toEqual([12, 11, 13]);
  });

  it('is ignored without a resolver', () => {
    expect(extractNoteLinks('[a](Todo)')).toEqual([]);
  });

  it('feeds the backlinks, with the excerpt of the link', () => {
    const notes = [note(10, 'Roadmap', 2), note(11, 'Meeting', 1, 'see [the plan](<Work/Projects/Roadmap>) now')];

    expect(findBacklinks(notes, 10, createLinkResolver(notes, FOLDERS))).toEqual([
      { noteId: 11, title: 'Meeting', excerpt: 'see [the plan](<Work/Projects/Roadmap>) now' },
    ]);
    expect(extractLinkExcerpt('x [p](Todo) y', 12, resolve)).toBe('x [p](Todo) y');
  });

  it('feeds the graph', () => {
    const notes = [note(10, 'Roadmap', 2), note(12, 'Todo', null, '[r](Work/Projects/Roadmap)')];
    const graph = buildNoteGraph(notes, { resolve: createLinkResolver(notes, FOLDERS) });

    expect(graph.links).toEqual([{ source: 12, target: 10 }]);
  });
});
