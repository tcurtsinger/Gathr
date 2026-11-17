// The face generator (0074). Expected options come from the approved mock-up's own generator
// (Shared/designs/ui-pass/faces/nice-gen.js), run on the same seeds and inputs.
import { expect, test } from 'vitest';
import { type FaceOptions, faceOptions, type Gender, SKIN } from './options';

const seeds = (n: number) => Array.from({ length: n }, (_, i) => `Made-up ${i}`);

// Each line as the mock-up's generator printed it: gender, skin tone, ears, hair, hair colour, hat, hat colour,
// eyes, glasses, nose, mouth, shirt and shirt colour.
test.each([
  ['Mira', {}, 'man 1 big thick #F48150 none #E0DDFF circle none round laugh short #F4D150'],
  ['Ann', {}, 'woman 5 small normal #F4D150 none #77311D oval square short smile short #F48150'],
  ['Bo', {}, 'woman 5 big womanLong #fff beanie #77311D circle square short smile short #fff'],
  ['Kai', {}, 'man 4 big normal #000 beanie #E0DDFF oval none long laugh short #F48150'],
  ['Ann', { gender: 'man' }, 'man 5 small mohawk #F4D150 none #77311D oval square short smile short #F48150'],
  ['Ada', { skin: 1 }, 'man 1 small normal #F4D150 none #000 smile none round laugh polo #E0DDFF'],
] as const)('%s %o gives the approved generator’s options', (seed, set, line) => {
  const [gender, skin, earSize, hairStyle, hairColor, hatStyle, hatColor, eyeStyle, ...rest] =
    line.split(' ');
  const [glassesStyle, noseStyle, mouthStyle, shirtStyle, shirtColor] = rest;
  expect(faceOptions(seed, set)).toEqual({
    gender,
    skin: Number(skin),
    faceColor: SKIN[Number(skin) - 1],
    earSize,
    hairStyle,
    hairColor,
    hatStyle,
    hatColor,
    eyeBrowStyle: gender === 'man' ? 'up' : 'upWoman',
    eyeStyle,
    glassesStyle,
    noseStyle,
    mouthStyle,
    shirtStyle,
    shirtColor,
  });
});

test('the same seed and inputs always give the same options', () => {
  const draw = () => seeds(200).map((s) => faceOptions(s, { gender: 'woman' }));
  const first = draw();
  expect(draw()).toEqual(first);
});

test('no two options move together: over many seeds every pair of their values turns up', () => {
  const keys = [
    'gender',
    'skin',
    'earSize',
    'hatStyle',
    'eyeStyle',
    'glassesStyle',
    'noseStyle',
    'mouthStyle',
    'shirtStyle',
  ];
  const all = seeds(3000).map((s) => faceOptions(s) as unknown as Record<string, unknown>);
  const values = (k: string) => new Set(all.map((o) => o[k])).size;
  const missing: string[] = [];
  for (const a of keys)
    for (const b of keys.filter((k) => k > a)) {
      const seen = new Set(all.map((o) => `${o[a]}|${o[b]}`)).size;
      if (seen < values(a) * values(b)) missing.push(`${a} with ${b}: ${seen} of ${values(a) * values(b)}`);
    }
  expect(missing).toEqual([]);
});

test('a gender or skin tone the person set is never overridden', () => {
  const wrong: string[] = [];
  for (const s of seeds(300))
    for (const gender of ['man', 'woman'] as Gender[])
      for (let skin = 1; skin <= 6; skin++) {
        const o = faceOptions(s, { gender, skin });
        if (o.gender !== gender || o.skin !== skin || o.faceColor !== SKIN[skin - 1])
          wrong.push(`${s} ${gender} ${skin}`);
      }
  expect(wrong).toEqual([]);
});

test('a hair or hat colour is skipped on the tone it melts into, and only there', () => {
  const top = (o: FaceOptions) => (o.hatStyle === 'none' ? o.hairColor : o.hatColor);
  const tops = (skin: number) => new Set(seeds(2000).map((s) => top(faceOptions(s, { skin }))));
  const on = [1, 2, 3, 4, 5, 6].map(tops);
  expect({
    white: on.map((c) => c.has('#fff')),
    ginger: on.map((c) => c.has('#F48150')),
    brown: on.map((c) => c.has('#77311D')),
  }).toEqual({
    white: [false, true, true, true, true, true],
    ginger: [true, true, false, false, true, true],
    brown: [true, true, true, true, true, false],
  });
});
