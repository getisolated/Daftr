import { EditorState } from '@codemirror/state';
import { syntaxTree } from '@codemirror/language';

export interface OutlineHeading {
  level: 1 | 2 | 3;
  text: string;
  /** Document position of the start of the heading line. */
  from: number;
}

/** h1, h2 and h3 headings of the document, read from the syntax tree so that `#` lines inside code blocks are ignored. */
export function extractHeadings(state: EditorState): OutlineHeading[] {
  const headings: OutlineHeading[] = [];

  syntaxTree(state).iterate({
    enter: node => {
      const match = /^(?:ATXHeading([123])|SetextHeading([12]))$/.exec(node.name);
      if (!match) return;

      const level = Number(match[1] ?? match[2]) as 1 | 2 | 3;
      const line = state.doc.lineAt(node.from);
      const text = line.text.replace(/^\s*#{1,3}\s+/, '').replace(/\s+#+\s*$/, '').trim();
      if (text) {
        headings.push({ level, text, from: line.from });
      }
      return false;
    },
  });

  return headings;
}

/** Index of the heading the reader is currently in: the last one whose top went past `offset` px from the viewport top. */
export function findActiveHeading(tops: readonly number[], scrollTop: number, atBottom: boolean, offset = 40): number {
  if (!tops.length) return -1;
  if (atBottom) return tops.length - 1;

  let active = 0;
  for (let i = 0; i < tops.length; i++) {
    if (tops[i]! <= scrollTop + offset) active = i;
  }
  return active;
}
