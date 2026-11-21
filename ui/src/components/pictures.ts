/**
 * An agent's picture: one outside shape, one pair of eyes and one line colour, drawn only with lines on no background.
 * Every shape is a path on a 48 box; `y` is where the eyes' middle sits in it and `s` their scale. `small` is the
 * shape's simpler drawing for 20 px and under, where it has one.
 */
export const SHAPES = [
  { key: 'circle', name: 'Circle', d: 'M24 6a18 18 0 1 1 0 36a18 18 0 1 1 0-36Z', y: 24, s: 1.1 },
  {
    key: 'tile',
    name: 'Tile',
    d: 'M6 18Q6 6 18 6L30 6Q42 6 42 18L42 30Q42 42 30 42L18 42Q6 42 6 30Z',
    y: 24,
    s: 1.1,
  },
  {
    key: 'cloud',
    name: 'Cloud',
    d:
      'M24 10.5A5.85 5.85 0 0 1 33.96 13.66A5.37 5.37 0 0 1 39.26 21.66A5.2 5.2 0 0 1 37.42 30.75A5.63 5.63 0 0 1 ' +
      '29.3 36.69A5.94 5.94 0 0 1 18.7 36.69A5.63 5.63 0 0 1 10.58 30.75A5.2 5.2 0 0 1 8.74 21.66A5.37 5.37 0 0 1 ' +
      '14.04 13.66A5.85 5.85 0 0 1 24 10.5Z',
    small:
      'M24 11A8.42 8.42 0 0 1 36.99 17.5A7.54 7.54 0 0 1 36.99 30.5A8.42 8.42 0 0 1 24 37A8.42 8.42 0 0 1 ' +
      '11.01 30.5A7.54 7.54 0 0 1 11.01 17.5A8.42 8.42 0 0 1 24 11Z',
    y: 24,
    s: 1,
  },
  {
    key: 'flower',
    name: 'Flower',
    d:
      'M24 9A8.25 8.25 0 1 1 36.84 18.33A8.25 8.25 0 1 1 31.94 33.42A8.25 8.25 0 1 1 16.06 33.42A8.25 8.25 0 1 1 ' +
      '11.16 18.33A8.25 8.25 0 1 1 24 9Z',
    y: 23,
    s: 0.85,
  },
  {
    key: 'hexagon',
    name: 'Hexagon',
    d:
      'M40 18.8Q43 24 40 29.2L36.5 35.26Q33.5 40.45 27.5 40.45L20.5 40.45Q14.5 40.45 11.5 35.26L8 29.2Q5 24 8 18.8' +
      'L11.5 12.74Q14.5 7.55 20.5 7.55L27.5 7.55Q33.5 7.55 36.5 12.74Z',
    small:
      'M42.5 22.27Q43.5 24 42.5 25.73L34.75 39.16Q33.75 40.89 31.75 40.89L16.25 40.89Q14.25 40.89 13.25 39.16' +
      'L5.5 25.73Q4.5 24 5.5 22.27L13.25 8.84Q14.25 7.11 16.25 7.11L31.75 7.11Q33.75 7.11 34.75 8.84Z',
    y: 24,
    s: 1.05,
  },
  {
    key: 'pebble',
    name: 'Pebble',
    d: 'M18 12.5C28 8.5 42.5 11 43.5 22C44.5 32 36.5 37.5 25 37.5C12.5 38 4 33 4.5 24.5C5 17.5 10.5 15 18 12.5Z',
    y: 25,
    s: 1,
  },
  {
    key: 'shield',
    name: 'Shield',
    d: 'M24 5C29 7.5 35 8.5 40 8.5V21.5C40 31.5 33 39 24 43C15 39 8 31.5 8 21.5V8.5C13 8.5 19 7.5 24 5Z',
    y: 21,
    s: 1,
  },
  {
    key: 'drop',
    name: 'Drop',
    d: 'M24 4.5C29 11.5 39 18.5 39 28.5A15 15 0 0 1 9 28.5C9 18.5 19 11.5 24 4.5Z',
    y: 29,
    s: 1,
  },
  {
    key: 'star',
    name: 'Star',
    d:
      'M21.08 7.43Q24 4 26.92 7.43L30.61 11.77Q32.23 13.67 34.54 14.63L39.81 16.8Q43.97 18.51 41.61 22.34' +
      'L38.63 27.2Q37.31 29.33 37.12 31.82L36.69 37.5Q36.34 41.99 31.97 40.93L26.43 39.59Q24 39 21.57 39.59' +
      'L16.03 40.93Q11.66 41.99 11.31 37.5L10.88 31.82Q10.69 29.33 9.37 27.2L6.39 22.34Q4.03 18.51 8.19 16.8' +
      'L13.46 14.63Q15.77 13.67 17.39 11.77Z',
    y: 25.5,
    s: 0.82,
  },
  {
    key: 'diamond',
    name: 'Diamond',
    d: 'M19.05 8.45Q24 3.5 28.95 8.45L39.55 19.05Q44.5 24 39.55 28.95L28.95 39.55Q24 44.5 19.05 39.55L8.45 28.95Q3.5 24 8.45 19.05Z',
    y: 24,
    s: 1.05,
  },
  {
    key: 'bubble',
    name: 'Bubble',
    d:
      'M6 17Q6 7 16 7L32 7Q42 7 42 17L42 25Q42 35 32 35L22.5 35Q21 35 19.88 36L13.12 42Q12 43 12.28 41.53' +
      'L13.22 36.47Q13.5 35 12 35L9.75 35Q6 35 6 31.25Z',
    y: 21,
    s: 1.05,
  },
  {
    key: 'leaf',
    name: 'Leaf',
    d: 'M7 24Q7 7 24 7L37 7Q41 7 41 11L41 24Q41 41 24 41L11 41Q7 41 7 37Z',
    y: 24,
    s: 1.05,
  },
] as const satisfies readonly {
  key: string;
  name: string;
  d: string;
  small?: string;
  y: number;
  s: number;
}[];

/** The eye types: a pair of eyes, about 20 wide and 10 tall around (0, 0), in the outline's own line. */
export const EYES = [
  { key: 'lines', name: 'Lines', d: ['M-6 -5v10', 'M6 -5v10'] },
  { key: 'glance', name: 'Glance', d: ['M-6 -5v10', 'M9 -5L4 0L9 5'] },
  { key: 'side', name: 'Side', d: ['M-8.5 -5L-3.5 0L-8.5 5', 'M3.5 -5L8.5 0L3.5 5'] },
  { key: 'squint', name: 'Squint', d: ['M-8.5 -5L-3.5 0L-8.5 5', 'M8.5 -5L3.5 0L8.5 5'] },
  {
    key: 'dots',
    name: 'Dots',
    d: [
      'M-7.3 0a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0 -2.6 0',
      'M4.7 0a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0 -2.6 0',
    ],
  },
  {
    key: 'rings',
    name: 'Rings',
    d: [
      'M-9.7 0a3.2 3.2 0 1 0 6.4 0a3.2 3.2 0 1 0 -6.4 0',
      'M3.3 0a3.2 3.2 0 1 0 6.4 0a3.2 3.2 0 1 0 -6.4 0',
    ],
  },
  {
    key: 'ovals',
    name: 'Ovals',
    d: ['M-9.5 0a3 5 0 1 0 6 0a3 5 0 1 0 -6 0', 'M3.5 0a3 5 0 1 0 6 0a3 5 0 1 0 -6 0'],
  },
  { key: 'happy', name: 'Happy', d: ['M-9.75 2.75Q-6 -7 -2.25 2.75', 'M2.25 2.75Q6 -7 9.75 2.75'] },
  { key: 'rest', name: 'Rest', d: ['M-9.75 -2.75Q-6 7 -2.25 -2.75', 'M2.25 -2.75Q6 7 9.75 -2.75'] },
  { key: 'wink', name: 'Wink', d: ['M-6 -5v10', 'M2.25 2.75Q6 -7 9.75 2.75'] },
  { key: 'calm', name: 'Calm', d: ['M-10 0h7', 'M3 0h7'] },
  { key: 'quirk', name: 'Quirk', d: ['M-6 -5v10', 'M6 -1.5v5'] },
] as const;

/** One hue each: a light line for the dark theme and a deep one for the light theme (OKLCH 0.80 and 0.50). */
export const COLOURS = [
  { key: 'red', name: 'Red', dark: '#ffa098', light: '#a83634' },
  { key: 'orange', name: 'Orange', dark: '#ffa56d', light: '#9a4901' },
  { key: 'amber', name: 'Amber', dark: '#e9b452', light: '#815b04' },
  { key: 'lime', name: 'Lime', dark: '#abcc6b', light: '#536f04' },
  { key: 'green', name: 'Green', dark: '#72d699', light: '#057743' },
  { key: 'teal', name: 'Teal', dark: '#2fd8d0', light: '#02736e' },
  { key: 'sky', name: 'Sky', dark: '#58ccff', light: '#066d90' },
  { key: 'blue', name: 'Blue', dark: '#9bbeff', light: '#305eb7' },
  { key: 'violet', name: 'Violet', dark: '#c3affe', light: '#6b4cae' },
  { key: 'plum', name: 'Plum', dark: '#e8a0ec', light: '#8c3f91' },
  { key: 'pink', name: 'Pink', dark: '#fe9ac2', light: '#a13567' },
  { key: 'grey', name: 'Grey', dark: '#bebebe', light: '#636363' },
] as const;

/** The three choices by name, each null until the person picks one. */
export interface Chosen {
  shape?: string | null;
  eyes?: string | null;
  colour?: string | null;
}

export interface Look {
  shape: (typeof SHAPES)[number];
  eyes: (typeof EYES)[number];
  colour: (typeof COLOURS)[number];
}

/** FNV-1a over the id: the same agent always gets the same three. */
function hash(id: string) {
  let h = 0x811c9dc5;
  for (let i = 0; i < id.length; i++) h = Math.imul(h ^ id.charCodeAt(i), 0x01000193);
  return h >>> 0;
}

/** What an agent is drawn with: each choice it has, and the rest worked out from its id. */
export function lookOf(id: string, picked: Chosen): Look {
  const h = hash(id);
  const find = <T extends { key: string }>(list: readonly T[], key: string | null | undefined, n: number) =>
    list.find((x) => x.key === key) ?? list[n % list.length];
  return {
    shape: find(SHAPES, picked.shape, h),
    eyes: find(EYES, picked.eyes, Math.floor(h / 12)),
    colour: find(COLOURS, picked.colour, Math.floor(h / 144)),
  };
}

/** Three new choices for Shuffle. */
export function shuffled(): Required<Chosen> {
  const any = <T extends { key: string }>(list: readonly T[]) =>
    list[Math.floor(Math.random() * list.length)].key;
  return { shape: any(SHAPES), eyes: any(EYES), colour: any(COLOURS) };
}

/** The line's width in the 48 box: 3, never thinner than 1.5 px at the size drawn. */
export const lineAt = (size: number) => Math.max(3, (1.5 * 48) / size);
