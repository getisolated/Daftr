import { Extension } from '@codemirror/state';
import { EditorView, ViewPlugin } from '@codemirror/view';

/** Width of the band along the right (and bottom) edge of the editor that shows the hidden scrollbar on hover. */
export const SCROLLBAR_HOVER_ZONE_PX = 12;

interface Box {
  right: number;
  bottom: number;
}

/** True when the pointer is over the band of a scrollbar: the right edge (vertical one) or the bottom edge (horizontal one). */
export function isOverScrollbarZone(x: number, y: number, box: Box, zone = SCROLLBAR_HOVER_ZONE_PX): boolean {
  const overVertical = x >= box.right - zone && x <= box.right;
  const overHorizontal = y >= box.bottom - zone && y <= box.bottom;
  return overVertical || overHorizontal;
}

const VISIBLE_CLASS = 'cm-scrollbar-visible';

/** Shows the scrollbar while the pointer is over its band, and while its thumb is being dragged. */
const scrollbarHover = ViewPlugin.fromClass(class {
  private dragging = false;
  private overZone = false;

  private readonly onMove = (event: MouseEvent): void => {
    this.overZone = isOverScrollbarZone(event.clientX, event.clientY, this.scroller.getBoundingClientRect());
    this.refresh();
  };

  private readonly onLeave = (): void => {
    this.overZone = false;
    this.refresh();
  };

  private readonly onDown = (): void => {
    this.dragging = this.overZone;
  };

  private readonly onUp = (): void => {
    this.dragging = false;
    this.refresh();
  };

  constructor(private readonly view: EditorView) {
    this.scroller.addEventListener('mousemove', this.onMove);
    this.scroller.addEventListener('mouseleave', this.onLeave);
    this.scroller.addEventListener('mousedown', this.onDown);
    // The drag may end outside the editor
    window.addEventListener('mouseup', this.onUp);
  }

  private get scroller(): HTMLElement {
    return this.view.scrollDOM;
  }

  private refresh(): void {
    this.scroller.classList.toggle(VISIBLE_CLASS, this.overZone || this.dragging);
  }

  destroy(): void {
    this.scroller.removeEventListener('mousemove', this.onMove);
    this.scroller.removeEventListener('mouseleave', this.onLeave);
    this.scroller.removeEventListener('mousedown', this.onDown);
    window.removeEventListener('mouseup', this.onUp);
    this.scroller.classList.remove(VISIBLE_CLASS);
  }
});

/** Editor scrollbar kept invisible (scrolling still works) until the pointer reaches it: see editor.component.scss. */
export const hiddenScrollbar: Extension = [
  EditorView.editorAttributes.of({ class: 'cm-scrollbar-hidden' }),
  scrollbarHover,
];
