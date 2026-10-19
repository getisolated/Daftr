import { EditorState, Facet } from '@codemirror/state';
import { Translator, translate } from '../../core/i18n/translate';

/**
 * Translator the CodeMirror extensions use for the texts they draw or insert. The editor provides one reading the
 * current language at each call; without one (tests, other hosts) the texts are English.
 */
export const translatorFacet = Facet.define<Translator, Translator>({
  combine: values => values[0] ?? ((key, params) => translate('en', key, params)),
});

export function translatorOf(state: EditorState): Translator {
  return state.facet(translatorFacet);
}
