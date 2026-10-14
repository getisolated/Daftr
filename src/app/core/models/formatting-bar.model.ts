export type FormattingBarMode = 'full' | 'compact';
export type FormattingBarStyle = 'pinned' | 'floating';
export type FormattingBarPosition = 'top' | 'bottom';

/** Formatting an editor command can apply; also the ids reported as active under the caret. */
export type FormatId =
  | 'paragraph' | 'h1' | 'h2' | 'h3' | 'h4'
  | 'bold' | 'italic' | 'strikethrough' | 'inlineCode'
  | 'codeBlock' | 'quote' | 'hr'
  | 'bulletList' | 'orderedList' | 'taskList';
