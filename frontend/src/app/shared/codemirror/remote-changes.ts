import { ChangeSet, EditorState, Transaction, TransactionSpec } from '@codemirror/state';
import { diffText } from '../../core/utils/text-diff.util';

/**
 * Changes to apply to the editor so that it shows `remote` (the note as stored, with the edits of another device or of
 * the AI) without losing what the user typed since `base` (the text last loaded or saved): three-way merge, the remote
 * changes being rebased over the local ones. `null` when there is nothing to apply.
 */
export function remoteChanges(base: string, local: string, remote: string): ChangeSet | null {
  if (remote === base || remote === local) {
    return null;
  }

  const toRemote = ChangeSet.of(diffText(base, remote), base.length);

  if (local === base) {
    return toRemote;
  }

  const toLocal = ChangeSet.of(diffText(base, local), base.length);
  const rebased = toRemote.map(toLocal);

  return rebased.empty ? null : rebased;
}

/**
 * Minimal transaction bringing the remote changes into the editor: the selections and cursors are mapped through it.
 * It stays out of the undo history (`addToHistory: false`): Ctrl+Z must undo the user's own edits (their positions are
 * mapped through this change by the history), never the other device's, which would then be saved and propagated as a
 * deletion. `Transaction.remote` marks it for the extensions that tell the user's input apart.
 */
export function remoteTransaction(state: EditorState, base: string, remote: string): TransactionSpec | null {
  const changes = remoteChanges(base, state.doc.toString(), remote);

  if (!changes) {
    return null;
  }

  return { changes, annotations: [Transaction.addToHistory.of(false), Transaction.remote.of(true)] };
}
