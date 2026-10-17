/** Where a document sits on the dial, relative to the dial center (px, y going down). */
export interface DialSlot {
  x: number;
  y: number;
}

/** Documents per ring of the dial, from the inner one outward; beyond, every document lands on the last ring. */
const DIAL_RINGS: readonly { capacity: number; radius: number }[] = [
  { capacity: 7, radius: 120 },
  { capacity: 11, radius: 200 },
  { capacity: 15, radius: 280 },
];

/** The dial is a half circle hanging below the title bar, kept away from its horizontal edges. */
const DIAL_START_DEGREES = 15;
const DIAL_END_DEGREES = 165;

/** Up to two letters standing for a title: the initials of its first two words, or its first two letters. */
export function monogram(title: string): string {
  const words = title.trim().split(/\s+/).filter(word => word.length > 0);

  if (words.length === 0) {
    return '·';
  }

  const letters = words.length > 1 ? `${words[0][0]}${words[1][0]}` : words[0].slice(0, 2);
  return letters.toUpperCase();
}

/** A stable hue for a document, so that it keeps its color wherever it shows up. */
export function documentHue(noteId: number): number {
  // Golden angle: consecutive ids get hues far apart
  const hue = (noteId * 137.508) % 360;
  return Math.round(hue < 0 ? hue + 360 : hue);
}

/**
 * Slots of `count` documents on the dial, most recent first: each ring is filled from its bottom (straight below
 * the center, the easiest place to reach) outward, alternating right and left.
 */
export function dialSlots(count: number): DialSlot[] {
  const slots: DialSlot[] = [];
  let remaining = count;

  for (let ring = 0; ring < DIAL_RINGS.length && remaining > 0; ring++) {
    const { capacity, radius } = DIAL_RINGS[ring];
    const isLast = ring === DIAL_RINGS.length - 1;
    const onRing = isLast ? remaining : Math.min(capacity, remaining);
    slots.push(...ringSlots(onRing, radius));
    remaining -= onRing;
  }

  return slots;
}

function ringSlots(count: number, radius: number): DialSlot[] {
  const step = count > 1 ? (DIAL_END_DEGREES - DIAL_START_DEGREES) / (count - 1) : 0;
  const angles = Array.from({ length: count }, (_, index) => DIAL_START_DEGREES + index * step);
  // From the bottom outward: sort the angles by their distance to 90° (straight down)
  const byReach = [...angles].sort((a, b) => Math.abs(a - 90) - Math.abs(b - 90));
  const center = count === 1 ? [90] : byReach;

  return center.map(degrees => {
    const radians = (degrees * Math.PI) / 180;
    return { x: Math.round(Math.cos(radians) * radius), y: Math.round(Math.sin(radians) * radius) };
  });
}
