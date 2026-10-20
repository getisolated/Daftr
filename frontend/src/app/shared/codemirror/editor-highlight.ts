import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';

/** Token colors come from the theme tokens (styles.scss), so switching theme needs no new CodeMirror extension. */
const editorHighlightStyle = HighlightStyle.define([
  // ── Comments ─────────────────────────────────────────────────
  { tag: t.comment,          color: 'var(--daftr-syntax-comment)', fontStyle: 'italic' },
  { tag: t.lineComment,      color: 'var(--daftr-syntax-comment)', fontStyle: 'italic' },
  { tag: t.blockComment,     color: 'var(--daftr-syntax-comment)', fontStyle: 'italic' },
  { tag: t.docComment,       color: 'var(--daftr-syntax-comment)', fontStyle: 'italic' },

  // ── Keywords ─────────────────────────────────────────────────
  { tag: t.keyword,          color: 'var(--daftr-syntax-keyword)' },
  { tag: t.controlKeyword,   color: 'var(--daftr-syntax-control)' },  // if, else, return, for, while
  { tag: t.operatorKeyword,  color: 'var(--daftr-syntax-keyword)' },   // typeof, instanceof, in, of
  { tag: t.definitionKeyword,color: 'var(--daftr-syntax-keyword)' },   // const, let, var, function, class
  { tag: t.moduleKeyword,    color: 'var(--daftr-syntax-control)' },   // import, export, from

  // ── Variables & properties ───────────────────────────────────
  { tag: t.variableName,     color: 'var(--daftr-syntax-variable)' },
  { tag: t.propertyName,     color: 'var(--daftr-syntax-variable)' },
  { tag: t.definition(t.variableName), color: 'var(--daftr-syntax-variable)' },

  // ── Functions ────────────────────────────────────────────────
  { tag: t.function(t.variableName),    color: 'var(--daftr-syntax-function)' },
  { tag: t.function(t.propertyName),    color: 'var(--daftr-syntax-function)' },
  { tag: t.definition(t.function(t.variableName)), color: 'var(--daftr-syntax-function)' },

  // ── Types, classes ───────────────────────────────────────────
  { tag: t.typeName,         color: 'var(--daftr-syntax-type)' },
  { tag: t.className,        color: 'var(--daftr-syntax-type)' },
  { tag: t.namespace,        color: 'var(--daftr-syntax-type)' },
  { tag: t.macroName,        color: 'var(--daftr-syntax-function)' },

  // ── Strings ──────────────────────────────────────────────────
  { tag: t.string,           color: 'var(--daftr-syntax-string)' },
  { tag: t.special(t.string),color: 'var(--daftr-syntax-escape)' },  // template literals, regex
  { tag: t.escape,           color: 'var(--daftr-syntax-escape)' },

  // ── Numbers & booleans ───────────────────────────────────────
  { tag: t.number,           color: 'var(--daftr-syntax-number)' },
  { tag: t.integer,          color: 'var(--daftr-syntax-number)' },
  { tag: t.float,            color: 'var(--daftr-syntax-number)' },
  { tag: t.bool,             color: 'var(--daftr-syntax-keyword)' },
  { tag: t.null,             color: 'var(--daftr-syntax-keyword)' },

  // ── Constants ────────────────────────────────────────────────
  { tag: t.constant(t.variableName), color: 'var(--daftr-syntax-constant)' },

  // ── Operators & punctuation ──────────────────────────────────
  { tag: t.operator,         color: 'var(--daftr-syntax-plain)' },
  { tag: t.punctuation,      color: 'var(--daftr-syntax-plain)' },
  { tag: t.paren,            color: 'var(--daftr-syntax-plain)' },
  { tag: t.squareBracket,    color: 'var(--daftr-syntax-plain)' },
  { tag: t.brace,            color: 'var(--daftr-syntax-plain)' },
  { tag: t.derefOperator,    color: 'var(--daftr-syntax-plain)' },
  { tag: t.separator,        color: 'var(--daftr-syntax-plain)' },

  // ── Tags (HTML/JSX) ─────────────────────────────────────────
  { tag: t.tagName,          color: 'var(--daftr-syntax-keyword)' },
  { tag: t.attributeName,    color: 'var(--daftr-syntax-variable)' },
  { tag: t.attributeValue,   color: 'var(--daftr-syntax-string)' },
  { tag: t.angleBracket,     color: 'var(--daftr-syntax-punctuation)' },

  // ── Regex ────────────────────────────────────────────────────
  { tag: t.regexp,           color: 'var(--daftr-syntax-regexp)' },

  // ── Labels, annotations ──────────────────────────────────────
  { tag: t.labelName,        color: 'var(--daftr-syntax-variable)' },
  { tag: t.annotation,       color: 'var(--daftr-syntax-function)' },

  // ── Markdown-specific tokens ─────────────────────────────────
  { tag: t.heading,          color: 'var(--daftr-syntax-heading)', fontWeight: 'bold' },
  { tag: t.emphasis,         color: 'var(--daftr-syntax-italic)', fontStyle: 'italic' },
  { tag: t.strong,           color: 'var(--daftr-syntax-bold)', fontWeight: 'bold' },
  { tag: t.strikethrough,    color: 'var(--daftr-syntax-strike)', textDecoration: 'line-through' },
  { tag: t.link,             color: 'var(--daftr-syntax-link)', textDecoration: 'underline' },
  { tag: t.url,              color: 'var(--daftr-syntax-link)' },
  { tag: t.quote,            color: 'var(--daftr-syntax-quote)', fontStyle: 'italic' },

  // ── Misc / fallback ──────────────────────────────────────────
  { tag: t.meta,             color: 'var(--daftr-syntax-keyword)' },
  { tag: t.atom,             color: 'var(--daftr-syntax-keyword)' },
  { tag: t.self,             color: 'var(--daftr-syntax-keyword)' },
  { tag: t.invalid,          color: 'var(--daftr-syntax-invalid)' },
]);

export const editorHighlight = syntaxHighlighting(editorHighlightStyle);
