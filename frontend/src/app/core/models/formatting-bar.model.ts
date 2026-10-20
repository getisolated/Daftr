export type FormattingBarMode = 'full' | 'compact';
export type FormattingBarStyle = 'pinned' | 'floating';
export type FormattingBarPosition = 'top' | 'bottom' | 'left' | 'right';

/** Formatting an editor command can apply; also the ids reported as active under the caret. */
export type FormatId =
  | 'paragraph' | 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6'
  | 'bold' | 'italic' | 'underline' | 'strikethrough' | 'inlineCode' | 'superscript' | 'subscript' | 'clearFormatting'
  | 'link' | 'image'
  | 'codeBlock' | 'quote' | 'hr' | 'table' | 'draft'
  | 'bulletList' | 'orderedList' | 'taskList';

/** Block styles offered by the heading drop-down of the bar, in display order. */
export const HEADING_FORMATS = ['paragraph', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6'] as const;

export type HeadingFormat = typeof HEADING_FORMATS[number];

/** One slot of the bar: a formatting button, or the heading drop-down. */
export type FormattingBarItem = FormatId | 'heading';

/** Groups of the bar; once condensed (narrow window), each one becomes a single drop-down. */
export type FormattingBarGroupId = 'heading' | 'text' | 'insert' | 'blocks' | 'lists';

export interface FormattingBarGroup {
  id: FormattingBarGroupId;
  items: FormattingBarItem[];
}
