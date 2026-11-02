# Plan — Word count and reading time

## Approach
Add a boolean setting `showWordCount` (default `false`) through the exact same path as `showOutline` (domain → raw/sanitize → JSON → DTO/ts-rs → front model → settings definitions → i18n). Counting is a **pure front-end util** (`core/utils/word-count.util.ts`): the text lives in CodeMirror, so a Rust/IPC round-trip per keystroke would be pointless. The editor component keeps a `wordCount` signal, refreshed immediately on note load / setting toggle and with a 150 ms debounce while typing (same pattern as `schedulePreviewRefresh`), and renders a thin footer strip below `.editor-wrapper` (outside the editor scroller, so it never scrolls away and stays visible in source / split / preview and in zen mode).
Rejected: a CodeMirror panel (`.cm-panels` is hidden by CSS, and the editor host is `display:none` in preview mode); counting in Rust (IPC per keystroke, no benefit); no debounce (full recount on each key of a 50k-word note).

## Acceptance criteria mapping
| Criterion | Where it is implemented | How it is tested |
| --- | --- | --- |
| CA1 default off | `domain/src/settings.rs` (`Default`), `frontend/.../settings.model.ts` (`DEFAULT_SETTINGS`) | Rust `domain::settings::tests::word_count_is_off_by_default_and_kept_when_set`; Vitest `frontend/tests/word-count.test.ts` (`DEFAULT_SETTINGS.showWordCount === false`) |
| CA2 toggle live + persisted | setting plumbing (domain, raw, sanitize, `app/src/settings/json.rs`, DTO); editor `wordCountShown` computed + effect | Rust: sanitize keeps `true`, missing key falls back to base, JSON round trip (`app::settings::json::tests::round_trips_the_settings`), patch merge; Vitest `settings-definitions.test.ts` (entry exists in Editor category, checkbox). Live show/hide: manual (PM) — toggle the setting with a note open, then restart the app |
| CA3 always visible | `editor.component.ts` template footer `.word-count` outside `.editor-wrapper`, `flex-shrink: 0` | manual (PM): scroll a long note, the strip stays at the bottom |
| CA4 live update | `editor.component.ts` updateListener → `scheduleWordCountRefresh` | manual (PM): type / delete / paste, count changes within a fraction of a second without saving |
| CA5 empty note | `countWords('')` = 0, `readingMinutes(0)` = 0, label `0 mot · moins d'une minute de lecture` | Vitest `word-count.test.ts` |
| CA6 whitespace separation | `countWords` | Vitest `word-count.test.ts` |
| CA7 markdown symbols ignored | `countWords` (token without letter/number/emoji ignored; leading list/task/fence markers ignored) | Vitest `word-count.test.ts` |
| CA8 reading time | `readingMinutes` | Vitest `word-count.test.ts` (0, 1, 200, 201, 1000) |
| CA9 translations + plurals | i18n keys in `translations/{en,fr}/editor.ts` and `settings.ts`; `wordCountLabel` | Vitest `word-count.test.ts` (fr 0/1 singular, en 1 singular / 0 plural) + existing `i18n.test.ts` (same keys, placeholders, .one/.other) |
| CA10 special characters | `countWords` (Unicode regexes with `u` flag) | Vitest `word-count.test.ts` (`l'été`, `it's`, `porte-monnaie`, emoji, em dash) |
| CA11 long note stays fluid | debounce `WORD_COUNT_REFRESH_MS = 150`, linear single-pass count, no count when setting off | Vitest: 50 000-word input returns 50 000 (correctness only, no timing assertion). Fluidity: manual (PM) — paste a very long text and type |
| CA12 note/tab switch, no note | `loadNote` sets `wordCount` from the loaded content before `activeNote` is shown; `wordCountShown` false when `activeNote()` is null | manual (PM): switch tabs, close all tabs |
| CA13 zen mode | footer is not conditioned on `layout.zenMode()` | manual (PM): Ctrl+K Z with the setting on |
| CA14 three view modes | footer outside `.editor-wrapper` (whose editor host is hidden in preview); count reads the CodeMirror doc (Markdown source), alive in all modes | manual (PM): switch source / split / preview |

## Shared interfaces

### Rust
- `domain::settings::AppSettings`: new field, placed right after `show_outline`:
  ```rust
  /// Nombre de mots et temps de lecture sous la note.
  pub show_word_count: bool,
  ```
  `Default`: `show_word_count: false`.
- `domain::settings::RawSettings`: `/// Voir [`AppSettings::show_word_count`].` `pub show_word_count: Option<bool>,` (after `show_outline`); `from_settings`: `Some(settings.show_word_count)`; `overridden_by`: `patch.show_word_count.or(self.show_word_count)`.
- `sanitize.rs` `apply_editor`: `settings.show_word_count = raw.show_word_count.unwrap_or(settings.show_word_count);`
- `app/src/settings/json.rs`: JSON key **`showWordCount`** — `show_word_count: flag("showWordCount")` in `raw_from_json`, `"showWordCount": settings.show_word_count` in `settings_to_json`.
- `presentation/src/dto/settings.rs` `SettingsDto`: `/// Compteur de mots sous la note.` `pub show_word_count: bool,` (after `show_outline`) and `show_word_count: settings.show_word_count` in `From<&AppSettings>`. `cargo test` regenerates `frontend/src/app/core/bindings/SettingsDto.ts` (adds `showWordCount: boolean`) — commit it with the Rust change.

### Front-end
- `AppSettings` (`frontend/src/app/core/models/settings.model.ts`): after `showOutline`:
  ```ts
  /** true: the word count and reading time of the open note are shown under it. */
  showWordCount: boolean;
  ```
  `DEFAULT_SETTINGS.showWordCount = false`.
- New file `frontend/src/app/core/utils/word-count.util.ts`:
  ```ts
  import { Language } from '../i18n/language.model';
  import { translate, translatePlural } from '../i18n/translate';

  /** Average silent reading speed used for the estimate. */
  export const WORDS_PER_MINUTE = 200;

  /** Words of a Markdown text: runs of non-whitespace holding a letter, a digit or an emoji, leading block markers excluded. */
  export function countWords(text: string): number;

  /** Reading time in whole minutes, rounded up: 0 for no word, at least 1 otherwise. */
  export function readingMinutes(words: number): number;

  /** "1,234 words · 7 min read" / "1 234 mots · 7 min de lecture"; "less than 1 min" wording for 0 words. */
  export function wordCountLabel(language: Language, words: number): string;
  ```
- i18n keys (add to `en/editor.ts` **and** `fr/editor.ts`, next to the other `editor.*` keys):
  | Key | en | fr |
  | --- | --- | --- |
  | `editor.wordCount.label` | `Word count and reading time` | `Nombre de mots et temps de lecture` |
  | `editor.wordCount.words.one` | `{count} word` | `{count} mot` |
  | `editor.wordCount.words.other` | `{count} words` | `{count} mots` |
  | `editor.wordCount.readingTime` | `{minutes} min read` | `{minutes} min de lecture` |
  | `editor.wordCount.readingTimeEmpty` | `less than 1 min read` | `moins d'une minute de lecture` |
  (fr string in the TS file: `'moins d\'une minute de lecture'`, matching the escaping style of `fr/settings.ts`.)
- i18n keys in `en/settings.ts` **and** `fr/settings.ts`, right after the `settings.showOutline.*` pair:
  | Key | en | fr |
  | --- | --- | --- |
  | `settings.showWordCount.title` | `Word Count and Reading Time` | `Compteur de mots et temps de lecture` |
  | `settings.showWordCount.description` | `Show the number of words of the open note and its estimated reading time (about 200 words per minute) at the bottom of the editor.` | `Afficher le nombre de mots de la note ouverte et son temps de lecture estimé (environ 200 mots par minute) en bas de l'éditeur.` |
- Settings definition (`features/settings/settings-definitions.ts`), inserted right after the `showOutline` entry:
  ```ts
  {
    key: 'showWordCount',
    categoryKey: 'settings.category.editor',
    titleKey: 'settings.showWordCount.title',
    descriptionKey: 'settings.showWordCount.description',
    control: 'checkbox',
  },
  ```

## Steps

### Step 1 — Setting plumbing (Rust + bindings)
- Files:
  - `src-tauri/crates/domain/src/settings.rs` — modify — field + doc + default `false`.
  - `src-tauri/crates/domain/src/settings/raw.rs` — modify — field + `from_settings` + `overridden_by`.
  - `src-tauri/crates/domain/src/settings/sanitize.rs` — modify — one line in `apply_editor`, after `show_outline`.
  - `src-tauri/crates/app/src/settings/json.rs` — modify — read/write `showWordCount`.
  - `src-tauri/crates/presentation/src/dto/settings.rs` — modify — DTO field + `From`.
  - `frontend/src/app/core/bindings/SettingsDto.ts` — regenerated by `cargo test`, never hand-edited.
- Details: every `AppSettings`, `RawSettings`, `SettingsDto` literal listing all fields must get the new field (the ones above are the only exhaustive ones; the compiler flags any other). No migration needed: a `settings.json` without the key yields `None` → base value `false` (constraint "existing settings load with the counter off").
- Commit: `feat(settings): add showWordCount setting`
- Done when: `cargo test --manifest-path src-tauri/Cargo.toml --workspace` passes and `SettingsDto.ts` contains `showWordCount: boolean`.

### Step 2 — Front setting, i18n
- Files:
  - `frontend/src/app/core/models/settings.model.ts` — modify — `showWordCount` in `AppSettings` + `DEFAULT_SETTINGS`.
  - `frontend/src/app/features/settings/settings-definitions.ts` — modify — checkbox entry after `showOutline`.
  - `frontend/src/app/core/i18n/translations/en/settings.ts`, `fr/settings.ts` — modify — `settings.showWordCount.*`.
  - `frontend/src/app/core/i18n/translations/en/editor.ts`, `fr/editor.ts` — modify — `editor.wordCount.*` keys (table above).
- Commit: `feat(settings): word count setting and translations`
- Done when: `npm test` i18n tests pass (same keys in both languages, same placeholders, `.one`/`.other` pairs).

### Step 3 — Pure counting util
- File: `frontend/src/app/core/utils/word-count.util.ts` — create.
- `countWords(text)`, single pass, line by line:
  ```ts
  const WORD_CHAR = /[\p{L}\p{N}\p{Extended_Pictographic}]/u;
  const TOKEN = /\S+/g;
  /** Leading block markers that hold a digit or letter but are markup: `1.` `2)` ordered items, `[x]` task boxes, ```` ```ts ```` fences. */
  const LEADING_MARKUP = /^(?:\d+[.)]|\[[ xX]?\]|`{3,}\S*|~{3,}\S*)$/;
  ```
  For each line of `text.split('\n')`: iterate `line.match(TOKEN) ?? []`; keep a `wordSeen` flag reset per line. A token is skipped if `!WORD_CHAR.test(token)` (pure symbols: `#`, `-`, `>`, `---`, `*`, `|`, `=`, `—`, `!`…) or if `!wordSeen && LEADING_MARKUP.test(token)`. Otherwise `count++` and `wordSeen = true`. No other parsing (links like `[Daftr](https://…)` count as their tokens; inline code is counted). `\r` is whitespace, so CRLF texts work. Never throws for any string.
- `readingMinutes(words)`: `words <= 0 ? 0 : Math.ceil(words / WORDS_PER_MINUTE)`.
- `wordCountLabel(language, words)`:
  ```ts
  const count = new Intl.NumberFormat(language).format(words);
  const wordsText = translatePlural(language, 'editor.wordCount.words', words, { count });
  const minutes = readingMinutes(words);
  const time = minutes === 0
    ? translate(language, 'editor.wordCount.readingTimeEmpty')
    : translate(language, 'editor.wordCount.readingTime', { minutes });
  return `${wordsText} · ${time}`;
  ```
  (`translatePlural` picks the form from the numeric `words` — `Intl.PluralRules('fr')` gives `one` for 0 and 1, `en` gives `one` for 1 only — and the `count` param overrides the raw number with the formatted one.)
- Commit: `feat(editor): word counting and reading time helpers`
- Done when: `frontend/tests/word-count.test.ts` passes.

### Step 4 — Editor footer
- Files: `frontend/src/app/features/editor/editor.component.ts`, `editor.component.scss` — modify.
- Component additions:
  ```ts
  import { countWords, wordCountLabel } from '../../core/utils/word-count.util';
  /** Delay before the word count follows the text being typed (a full recount on every key of a long note is wasted). */
  const WORD_COUNT_REFRESH_MS = 150;

  /** Words of the open note; only kept up to date while the setting `showWordCount` is on. */
  protected readonly wordCount = signal(0);
  private wordCountDebounce: ReturnType<typeof setTimeout> | null = null;
  /** Never on the quick notes list: it is a list of short notes, not one text. */
  protected readonly wordCountShown = computed(() => {
    const note = this.activeNote();
    return this.settingsService.settings().showWordCount && note !== null && note.id !== QUICK_NOTES_TAB_ID;
  });
  protected readonly wordCountText = computed(() => wordCountLabel(this.i18n.language(), this.wordCount()));
  ```
  Methods:
  - `private recountWords(text: string): void` — clears `wordCountDebounce`; if `settings().showWordCount` is false, returns; else `this.wordCount.set(countWords(text))`.
  - `private scheduleWordCountRefresh(view: EditorView): void` — returns if `!this.wordCountShown()`; clears and re-arms `wordCountDebounce` with `setTimeout(() => this.recountWords(view.state.doc.toString()), WORD_COUNT_REFRESH_MS)`.
- Wiring:
  - `updateListener`: inside the existing `if (update.docChanged)` block, after `schedulePreviewRefresh`, call `this.scheduleWordCountRefresh(update.view)`.
  - `loadNote`: right **before** `this.activeNote.set(note)`, call `this.recountWords(note?.content ?? '')` so a newly shown note never shows the previous count (CA12).
  - Constructor: new effect so that turning the setting on counts at once (CA2):
    ```ts
    // Turned on while a note is open: counted at once, not at the next key
    effect(() => {
      const on = this.settingsService.settings().showWordCount;
      untracked(() => {
        if (on) this.recountWords(this.editorView?.state.doc.toString() ?? this.activeNote()?.content ?? '');
      });
    });
    ```
  - `ngOnDestroy`: `if (this.wordCountDebounce) clearTimeout(this.wordCountDebounce);`
- Template: as the last child of the host, right after the closing `</div>` of `.editor-wrapper` and before the `@if (ctxMenu()…)` block:
  ```html
  @if (wordCountShown()) {
    <div class="word-count" role="status" aria-live="off" [attr.aria-label]="i18n.t('editor.wordCount.label')">{{ wordCountText() }}</div>
  }
  ```
  (`aria-live="off"`: screen readers do not announce every keystroke; the text stays readable on demand.)
- SCSS (`editor.component.scss`, new section near the file banner):
  ```scss
  // ── Word count and reading time (setting `showWordCount`) ─────────────────────
  .word-count {
    flex-shrink: 0;
    padding: 2px 16px 4px;
    text-align: right;
    font-family: 'Segoe UI', system-ui, sans-serif;
    font-size: 11.5px;
    color: var(--daftr-text-soft);
    font-variant-numeric: tabular-nums;
    user-select: none;
    border-top: 1px solid var(--daftr-border-subtle);
  }
  ```
  `--daftr-text-soft` (not `-faint`) for sufficient contrast in light and dark themes. No zen-specific rule: the strip stays (CA13).
- Commit: `feat(editor): show word count and reading time under the note`
- Done when: `npm run lint` and `npm test` pass; the component compiles (`npm test` / lint cover the TS).

## Test strategy
- **Rust (agent tests)**:
  - `src-tauri/crates/domain/src/settings/tests.rs`: `word_count_is_off_by_default_and_kept_when_set` — `!AppSettings::default().show_word_count`; `sanitize(RawSettings { show_word_count: Some(true), .. })` is `true`; with `base.show_word_count = true` and an empty raw, stays `true` (falls back to the base, not to the default); `RawSettings::from_settings(&s).overridden_by(RawSettings { show_word_count: Some(true), ..Default })` sanitizes to `true`.
  - `src-tauri/crates/app/src/settings/json.rs` tests: add `show_word_count: true` to `round_trips_the_settings`; new test `reads_the_word_count_flag` — `raw_from_json(&json!({"showWordCount": true})).show_word_count == Some(true)`, `json!({"showWordCount": "yes"})` → `None`, `json!({})` → `None`; `settings_to_json(&AppSettings::default())["showWordCount"] == json!(false)`.
  - Fail before code: compile errors (unknown field) — expected.
- **Vitest** `frontend/tests/word-count.test.ts` (new):
  - `countWords` nominal: `'Bonjour   le monde'` → 3; `'a\tb\nc'` → 3; `'Hello, world!'` → 2; CRLF `'one\r\ntwo'` → 2.
  - empty: `''` → 0; `'   \n\t\n  '` → 0.
  - Markdown (CA7): `'# Title'` → 1; `'## Sub title'` → 2; `'- item'` → 1; `'> quote'` → 1; `'---'` → 0; `'***'` → 0; `'1. first'` → 1; `'- [ ] task'` → 1; `'- [x] done'` → 1; `'[Daftr](https://daftr.app)'` → 1; `` '`code` here' `` → 2; `` '```ts\nconst x = 1;\n```' `` → 3 (`const`, `x`, `1;`); `'| a | b |'` → 2; `'Chapter 1.'` → 2 (a number after a word is a word).
  - special characters (CA10): `"l'été"` → 1; `"it's"` → 1; `'porte-monnaie'` → 1; `'l’été'` (typographic apostrophe) → 1; `'café 😀'` → 2; `'a — b'` → 2; no throw on lone surrogates `'\uD800'` (→ 0).
  - long note (CA11): `'word '.repeat(50_000)` → 50 000 (no timing assertion).
  - `readingMinutes`: 0→0, 1→1, 200→1, 201→2, 1000→5.
  - `wordCountLabel` (CA5, CA9): `('fr', 0)` → `"0 mot · moins d'une minute de lecture"`; `('fr', 1)` → `'1 mot · 1 min de lecture'`; `('fr', 2)` → `'2 mots · 1 min de lecture'`; `('en', 0)` → `'0 words · less than 1 min read'`; `('en', 1)` → `'1 word · 1 min read'`; `('en', 201)` → `'201 words · 2 min read'`; `('fr', 1000)` → `` `${new Intl.NumberFormat('fr').format(1000)} mots · 5 min de lecture` `` (build the expected number with `Intl` — the French group separator is a non-breaking space whose exact code point depends on ICU).
  - `DEFAULT_SETTINGS.showWordCount` is `false` (CA1).
  - Fail before code: module `word-count.util` missing — expected.
- **Vitest** `frontend/tests/settings-definitions.test.ts` (extend): `SETTING_DEFINITIONS` has an entry `showWordCount` with `control: 'checkbox'` and `categoryKey: 'settings.category.editor'`, placed right after `showOutline`; `filterSettings(SETTING_DEFINITIONS, 'mots', fr)` contains `showWordCount`.
- Existing `i18n.test.ts` covers key parity, placeholders and plural pairs for the new keys without change.
- Not automatable (manual, PM, on the test build): CA2 live toggle + restart, CA3 scroll, CA4 typing/paste, CA11 fluidity, CA12 tab switch / no note, CA13 zen, CA14 the three view modes, contrast in light and dark themes.

## Verification
```
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace
npm ci
npm run lint
npm test
```

## Documentation impact
- `README.md`: in the features list (near the zen mode / outline bullets, ~line 58), add a bullet: **Word count** — with **Settings → Word Count and Reading Time**, the number of words of the open note and its reading time (about 200 words per minute) show at the bottom of the editor.
- CHANGELOG: none (release tooling generates it).

## Risks
- **Bindings**: forgetting to commit the regenerated `SettingsDto.ts` breaks the front type check — run `cargo test` before committing step 1.
- **Exhaustive struct literals**: any missed `AppSettings`/`RawSettings`/`SettingsDto` literal fails to compile — the compiler lists them; `missing_docs` requires the `///` on each new pub field.
- **Performance (CA11)**: counting only runs when the setting is on, debounced 150 ms, linear. Do not count in a `computed` reading the whole doc on every transaction.
- **Stale count on switch**: `initEditor` runs in a `setTimeout`; counting from `note.content` in `loadNote` before `activeNote.set` avoids one frame with the previous note's count.
- **Plural forms**: rely on `Intl.PluralRules` (fr: 0 and 1 singular; en: 1 singular) through `translatePlural`; the formatted `count` param must be passed after the numeric count so the plural form is chosen on the number, not on the formatted string.
- **Unicode regex**: `\p{Extended_Pictographic}` needs the `u` flag (supported by WebView2 / WKWebView and Node); keep all three regexes with `u` where `\p{}` is used.
- **Layout**: `:host` is a flex column; the strip must be a direct child of the host with `flex-shrink: 0`, otherwise it may be pushed out in short windows. In zen mode nothing overlaps the bottom edge (the floating title bar is at the top).
- Platforms: no OS-specific code; behaviour identical on Windows and macOS.
