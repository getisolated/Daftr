import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';

/** Token colors come from the theme tokens (styles.scss), so switching theme needs no new CodeMirror extension. */
const editorHighlightStyle = HighlightStyle.define([
  // ── Comments ─────────────────────────────────────────────────
  { tag: t.comment,          color: 'var(--evn-syntax-comment)', fontStyle: 'italic' },
  { tag: t.lineComment,      color: 'var(--evn-syntax-comment)', fontStyle: 'italic' },
  { tag: t.blockComment,     color: 'var(--evn-syntax-comment)', fontStyle: 'italic' },
  { tag: t.docComment,       color: 'var(--evn-syntax-comment)', fontStyle: 'italic' },

  // ── Keywords ─────────────────────────────────────────────────
  { tag: t.keyword,          color: 'var(--evn-syntax-keyword)' },
  { tag: t.controlKeyword,   color: 'var(--evn-syntax-control)' },  // if, else, return, for, while
  { tag: t.operatorKeyword,  color: 'var(--evn-syntax-keyword)' },   // typeof, instanceof, in, of
  { tag: t.definitionKeyword,color: 'var(--evn-syntax-keyword)' },   // const, let, var, function, class
  { tag: t.moduleKeyword,    color: 'var(--evn-syntax-control)' },   // import, export, from

  // ── Variables & properties ───────────────────────────────────
  { tag: t.variableName,     color: 'var(--evn-syntax-variable)' },
  { tag: t.propertyName,     color: 'var(--evn-syntax-variable)' },
  { tag: t.definition(t.variableName), color: 'var(--evn-syntax-variable)' },

  // ── Functions ────────────────────────────────────────────────
  { tag: t.function(t.variableName),    color: 'var(--evn-syntax-function)' },
  { tag: t.function(t.propertyName),    color: 'var(--evn-syntax-function)' },
  { tag: t.definition(t.function(t.variableName)), color: 'var(--evn-syntax-function)' },

  // ── Types, classes ───────────────────────────────────────────
  { tag: t.typeName,         color: 'var(--evn-syntax-type)' },
  { tag: t.className,        color: 'var(--evn-syntax-type)' },
  { tag: t.namespace,        color: 'var(--evn-syntax-type)' },
  { tag: t.macroName,        color: 'var(--evn-syntax-function)' },

  // ── Strings ──────────────────────────────────────────────────
  { tag: t.string,           color: 'var(--evn-syntax-string)' },
  { tag: t.special(t.string),color: 'var(--evn-syntax-escape)' },  // template literals, regex
  { tag: t.escape,           color: 'var(--evn-syntax-escape)' },

  // ── Numbers & booleans ───────────────────────────────────────
  { tag: t.number,           color: 'var(--evn-syntax-number)' },
  { tag: t.integer,          color: 'var(--evn-syntax-number)' },
  { tag: t.float,            color: 'var(--evn-syntax-number)' },
  { tag: t.bool,             color: 'var(--evn-syntax-keyword)' },
  { tag: t.null,             color: 'var(--evn-syntax-keyword)' },

  // ── Constants ────────────────────────────────────────────────
  { tag: t.constant(t.variableName), color: 'var(--evn-syntax-constant)' },

  // ── Operators & punctuation ──────────────────────────────────
  { tag: t.operator,         color: 'var(--evn-syntax-plain)' },
  { tag: t.punctuation,      color: 'var(--evn-syntax-plain)' },
  { tag: t.paren,            color: 'var(--evn-syntax-plain)' },
  { tag: t.squareBracket,    color: 'var(--evn-syntax-plain)' },
  { tag: t.brace,            color: 'var(--evn-syntax-plain)' },
  { tag: t.derefOperator,    color: 'var(--evn-syntax-plain)' },
  { tag: t.separator,        color: 'var(--evn-syntax-plain)' },

  // ── Tags (HTML/JSX) ─────────────────────────────────────────
  { tag: t.tagName,          color: 'var(--evn-syntax-keyword)' },
  { tag: t.attributeName,    color: 'var(--evn-syntax-variable)' },
  { tag: t.attributeValue,   color: 'var(--evn-syntax-string)' },
  { tag: t.angleBracket,     color: 'var(--evn-syntax-punctuation)' },

  // ── Regex ────────────────────────────────────────────────────
  { tag: t.regexp,           color: 'var(--evn-syntax-regexp)' },

  // ── Labels, annotations ──────────────────────────────────────
  { tag: t.labelName,        color: 'var(--evn-syntax-variable)' },
  { tag: t.annotation,       color: 'var(--evn-syntax-function)' },

  // ── Markdown-specific tokens ─────────────────────────────────
  { tag: t.heading,          color: 'var(--evn-syntax-heading)', fontWeight: 'bold' },
  { tag: t.emphasis,         color: 'var(--evn-syntax-italic)', fontStyle: 'italic' },
  { tag: t.strong,           color: 'var(--evn-syntax-bold)', fontWeight: 'bold' },
  { tag: t.strikethrough,    color: 'var(--evn-syntax-strike)', textDecoration: 'line-through' },
  { tag: t.link,             color: 'var(--evn-syntax-link)', textDecoration: 'underline' },
  { tag: t.url,              color: 'var(--evn-syntax-link)' },
  { tag: t.quote,            color: 'var(--evn-syntax-quote)', fontStyle: 'italic' },

  // ── Misc / fallback ──────────────────────────────────────────
  { tag: t.meta,             color: 'var(--evn-syntax-keyword)' },
  { tag: t.atom,             color: 'var(--evn-syntax-keyword)' },
  { tag: t.self,             color: 'var(--evn-syntax-keyword)' },
  { tag: t.invalid,          color: 'var(--evn-syntax-invalid)' },
]);

export const editorHighlight = syntaxHighlighting(editorHighlightStyle);
