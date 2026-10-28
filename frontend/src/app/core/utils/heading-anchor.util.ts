/** Inline markdown of a heading that is no text of its own: images, HTML tags, the `[!NEW]` badge marker. */
const NOT_TEXT = /!\[[^\]]*\]\([^)]*\)|<[^>]+>|^\s*\[!NEW\]/gi;
const LINK = /\[(?<label>[^\]]*)\]\([^)]*\)/g;
/** `_` around words is emphasis; inside a word (snake_case) it is text. */
const UNDERSCORE_EMPHASIS = /(?<![\p{L}\p{N}])_+|_+(?![\p{L}\p{N}])/gu;

/** GitHub-like anchor of a heading: lowercase, punctuation dropped, spaces turned into dashes, accents kept. */
export function headingSlug(text: string): string {
  return text.trim().toLowerCase().replace(/[^\p{L}\p{M}\p{N}_\- ]/gu, '').replace(/ /g, '-');
}

/** Anchors of the headings of a page, in order: a repeated one gets `-1`, `-2`… as on GitHub. */
export function createHeadingSlugger(): (text: string) => string {
  const taken = new Set<string>();

  return text => {
    const base = headingSlug(text);
    let slug = base;

    for (let suffix = 1; taken.has(slug); suffix++) {
      slug = `${base}-${suffix}`;
    }

    taken.add(slug);
    return slug;
  };
}

/** Text of a heading as written in the source, as the preview shows it: links keep their label, markers are dropped. */
export function headingSourceText(source: string): string {
  return source.replace(NOT_TEXT, '').replace(LINK, '$<label>').replace(UNDERSCORE_EMPHASIS, '');
}

/** Fragment of a link target (after `#`), percent-decoded; `null` when it has none. */
export function linkAnchor(target: string): string | null {
  const cleaned = target.trim().replace(/^<|>$/g, '');
  const index = cleaned.indexOf('#');
  const fragment = index === -1 ? '' : cleaned.slice(index + 1);

  if (!fragment) {
    return null;
  }

  try {
    return decodeURIComponent(fragment);
  } catch {
    // Not percent-encoded: kept as written
    return fragment;
  }
}

/** Whether a link fragment designates the heading of anchor `slug` (`#Mon chapitre` as well as `#mon-chapitre`). */
export function anchorMatches(anchor: string, slug: string): boolean {
  return headingSlug(anchor) === slug;
}
