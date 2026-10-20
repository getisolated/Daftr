import { ChangeDetectionStrategy, Component, input } from '@angular/core';

/**
 * Daftr mark alone (glyph and its shadow) on a transparent background, drawn in `currentColor`: the host
 * sets the colour (see `--daftr-brand`). The shadow keeps its 28 % opacity relative to that colour.
 */
@Component({
  selector: 'app-daftr-mark',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <svg [attr.width]="size()" [attr.height]="size()" viewBox="0 0 100 100" aria-hidden="true">
      <g fill="none" stroke="currentColor" stroke-width="13" stroke-linecap="round" stroke-linejoin="round">
        <path d="M 64 24 C 69 40 75 56 81 72 Q 84 83 72 83 L 38 83 C 31 83 26 78 26 70" stroke-opacity="0.28" />
        <path d="M 58 18 C 63 34 69 50 75 66 Q 78 77 66 77 L 32 77 C 25 77 20 72 20 64" />
      </g>
    </svg>
  `,
  styles: [':host { display: inline-flex; line-height: 0; }'],
})
export class DaftrMarkComponent {
  /** Side of the square, in pixels. */
  readonly size = input(28);
}

/** Daftr logotype (glyph + word mark) drawn in `currentColor`, so the host sets its colour (see `--daftr-brand`). */
@Component({
  selector: 'app-daftr-logotype',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <svg [attr.width]="width()" viewBox="0 0 591 168" aria-label="Daftr" role="img">
      <g transform="translate(1.43 4.77) scale(1.6722)" fill="none" stroke="currentColor" stroke-width="13" stroke-linecap="round" stroke-linejoin="round">
        <path d="M 58 18 C 63 34 69 50 75 66 Q 78 77 66 77 L 32 77 C 25 77 20 72 20 64" />
      </g>
      <path fill="currentColor" d="M149.312 144.4 180.96 24H220.52L252.168 144.4H228.776L222.24 117.912H179.24L172.704 144.4ZM184.572 96.928H216.908L202.288 38.276H199.192ZM270.056 144.4V24H345.736V44.64H292.76V73.708H341.608V94.348H292.76V144.4ZM393.208 144.4V44.64H358.12V24H451V44.64H415.912V144.4ZM469.92 144.4V24H522.208Q533.56 24 541.988 27.956Q550.416 31.912 555.06 39.136Q559.704 46.36 559.704 56.164V58.228Q559.704 69.064 554.544 75.772Q549.384 82.48 541.816 85.576V88.672Q548.696 89.016 552.48 93.402Q556.264 97.788 556.264 105.012V144.4H533.56V108.28Q533.56 104.152 531.41 101.572Q529.26 98.992 524.272 98.992H492.624V144.4ZM492.624 78.352H519.8Q527.884 78.352 532.442 73.966Q537 69.58 537 62.356V60.636Q537 53.412 532.528 49.026Q528.056 44.64 519.8 44.64H492.624Z" />
    </svg>
  `,
  styles: [':host { display: inline-flex; line-height: 0; }'],
})
export class DaftrLogotypeComponent {
  /** Width in pixels; the height follows the 591 × 168 aspect ratio. */
  readonly width = input(120);
}
