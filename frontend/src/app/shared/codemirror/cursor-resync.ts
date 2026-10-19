import { ViewPlugin, type EditorView } from '@codemirror/view';

/**
 * Redraws the cursor and the selection when the editor width changes. The content is centered
 * with a max width: a resize moves it sideways without changing its geometry for CodeMirror,
 * so its cursor layer kept its old position until the next keystroke.
 */
export const cursorResync = ViewPlugin.fromClass(class {
  private readonly observer: ResizeObserver;
  private frame = 0;
  private width: number;

  constructor(view: EditorView) {
    this.width = view.dom.clientWidth;
    this.observer = new ResizeObserver(() => {
      if (view.dom.clientWidth === this.width) return;
      this.width = view.dom.clientWidth;
      cancelAnimationFrame(this.frame);
      // Re-setting the same selection is enough for the layers to measure again
      this.frame = requestAnimationFrame(() => view.dispatch({ selection: view.state.selection }));
    });
    this.observer.observe(view.dom);
  }

  public destroy(): void {
    this.observer.disconnect();
    cancelAnimationFrame(this.frame);
  }
});
