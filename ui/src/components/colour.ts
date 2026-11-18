/** A colour as hue (0 to 360), saturation and brightness (0 to 1): the picker's square and bar. */
export type Hsv = [number, number, number];

const byte = (n: number) =>
  Math.round(Math.max(0, Math.min(1, n)) * 255)
    .toString(16)
    .padStart(2, '0')
    .toUpperCase();
const rgbHex = (r: number, g: number, b: number) => `#${byte(r)}${byte(g)}${byte(b)}`;

/** A typed colour, 3 or 6 hex digits with or without #, as #RRGGBB; null when it isn't one. */
export function cleanHex(text: string): string | null {
  const m = /^#?([0-9a-f]{6}|[0-9a-f]{3})$/i.exec(text.trim());
  if (!m) return null;
  const digits = m[1].length === 3 ? [...m[1]].map((d) => d + d).join('') : m[1];
  return `#${digits.toUpperCase()}`;
}

export function hexHsv(hex: string): Hsv {
  const [r, g, b] = [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16) / 255);
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  let h = 0;
  if (d) h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [(h * 60 + 360) % 360, max ? d / max : 0, max];
}

export function hsvHex([h, s, v]: Hsv): string {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return v - v * s * Math.max(0, Math.min(k, 4 - k, 1));
  };
  return rgbHex(f(5), f(3), f(1));
}

/** A grey, or black, has no hue of its own: the bar stays where it was. */
export const keepHue = (old: Hsv, next: Hsv): Hsv =>
  next[1] === 0 || next[2] === 0 ? [old[0], next[1], next[2]] : next;
