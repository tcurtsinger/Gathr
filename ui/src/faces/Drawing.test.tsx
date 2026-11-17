// Every colour a face is drawn with, read from the drawn faces: blue, cyan and green mean a state in Gathr.
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import { Drawing } from './Drawing';
import { faceOptions } from './options';

/** A drawn colour in OKLCH's chroma and hue, from #rgb, #rrggbb, rgba() or a named black or white. */
function chromaHue(colour: string): [number, number] {
  const named: Record<string, string> = { black: '#000', white: '#fff' };
  const c = named[colour] ?? colour;
  const rgba = /^rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(c);
  const hex = c.length === 4 ? [...c.slice(1)].map((x) => x + x).join('') : c.slice(1);
  const channels = rgba
    ? rgba.slice(1, 4).map(Number)
    : [0, 2, 4].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
  if (channels.some(Number.isNaN)) throw new Error(`not a colour: ${colour}`);
  const [r, g, b] = channels.map((v) =>
    v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4,
  );
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [Math.hypot(A, B), ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360];
}

test('no face is drawn in blue, cyan or green, whatever its seed, gender and skin tone', async () => {
  const faces = Array.from({ length: 600 }, (_, i) =>
    faceOptions(`Made-up ${i}`, { gender: i % 3 ? null : 'woman', skin: i % 7 || null }),
  );
  const screen = await render(
    <div>
      {faces.map((o, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: the faces never move
        <Drawing key={i} options={o} />
      ))}
    </div>,
  );
  const drawn = new Set<string>();
  for (const el of screen.container.querySelectorAll('[fill], [stroke]'))
    for (const attr of ['fill', 'stroke']) {
      const value = el.getAttribute(attr);
      if (value && value !== 'none' && !value.startsWith('url(')) drawn.add(value);
    }
  // Lavender (#E0DDFF, hue 289) and the warm colours stay; the library's blue, cyan and green sit at 138 to 271.
  const cool = [...drawn].filter((c) => {
    const [chroma, hue] = chromaHue(c);
    return chroma > 0.02 && hue >= 120 && hue < 280;
  });
  expect(cool).toEqual([]);
  expect(drawn.size).toBeGreaterThan(20);
});
