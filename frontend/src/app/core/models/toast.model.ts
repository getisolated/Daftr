/** A short in-app notification, in the bottom right corner of the window. */
export interface Toast {
  id: number;
  message: string;
  /** Button next to the message (e.g. "Choose" on a proposed profile). */
  action: ToastAction | null;
}

export interface ToastAction {
  label: string;
  run: () => void;
}
