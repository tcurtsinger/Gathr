/**
 * The ready backgrounds for a face (0074), in Settings' order: OKLCH lightness 0.80, chroma 0.11 (less where sRGB
 * can't reach it), round the wheel. The names are for screen readers only.
 */
export const BACKGROUNDS: readonly (readonly [name: string, hex: string])[] = [
  ['Rose', '#FCA0A7'],
  ['Coral', '#F9A782'],
  ['Apricot', '#EBB16C'],
  ['Gold', '#D4BD67'],
  ['Pistachio', '#B4C875'],
  ['Leaf', '#90D192'],
  ['Mint', '#6CD5B3'],
  ['Aqua', '#56D3DA'],
  ['Sky', '#6BCBF7'],
  ['Periwinkle', '#98BFFE'],
  ['Lavender', '#BCB2FF'],
  ['Orchid', '#E6A4E0'],
];

/** A face whose background the person hasn't picked. */
export const UNPICKED = '#CECECE';

/** The colour a face is drawn on: the person's pick as picked, in light and dark alike, or the grey. */
export const drawnOn = (picked: string | null | undefined) => picked ?? UNPICKED;
