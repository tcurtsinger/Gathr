// What a face is drawn with, from its seed, gender and skin tone (0074): every option takes its own hash of the
// seed, so no two options move together. Gender and skin tone are the person's; left unset, the seed picks them.
// The lists are fixed: Gathr stores the seed, not the options, so a change to a list would re-pick that feature
// for existing agents. No blue, cyan or green anywhere: those colours mean a state in Gathr.

export type Gender = 'man' | 'woman';
export type HairStyle = 'normal' | 'thick' | 'mohawk' | 'womanLong' | 'womanShort';

/** Six skin tones, light to deep (skin tone 1 to 6), evenly spread in lightness. */
export const SKIN = ['#FFE1CC', '#EDC1A7', '#D8A184', '#C28266', '#AA654B', '#904A34'];
const HAIR = ['#000', '#77311D', '#F4D150', '#fff', '#F48150'];
const HAT = ['#000', '#77311D', '#F4D150', '#fff', '#E0DDFF'];
const SHIRT = ['#F4D150', '#FC909F', '#F48150', '#77311D', '#E0DDFF', '#fff'];
/** A hair or hat colour skipped on the tone it melts into, by tone. */
const NOT_ON = [['#fff'], [], ['#F48150'], ['#F48150'], [], ['#77311D']];

const HAIR_STYLE: Record<Gender, HairStyle[]> = {
  man: ['normal', 'normal', 'thick', 'thick', 'mohawk'],
  // Lined up with the man's list, so a seed never gets the same hair as a man and as a woman.
  woman: ['womanLong', 'womanShort', 'womanLong', 'womanShort', 'normal'],
};
const HAT_STYLE = ['none', 'none', 'none', 'none', 'none', 'none', 'beanie', 'turban'] as const;
const GLASSES = ['none', 'none', 'none', 'none', 'round', 'square'] as const;

export interface FaceOptions {
  gender: Gender;
  /** Skin tone 1 to 6. */
  skin: number;
  faceColor: string;
  earSize: 'small' | 'big';
  hairStyle: HairStyle;
  hairColor: string;
  hatStyle: (typeof HAT_STYLE)[number];
  hatColor: string;
  /** A woman always has lashes, so her choice shows even under a hat, which hides the hair. */
  eyeBrowStyle: 'up' | 'upWoman';
  eyeStyle: 'circle' | 'oval' | 'smile';
  glassesStyle: (typeof GLASSES)[number];
  noseStyle: 'short' | 'long' | 'round';
  mouthStyle: 'laugh' | 'smile' | 'peace';
  shirtStyle: 'hoody' | 'short' | 'polo';
  shirtColor: string;
}

/** A 32-bit hash of the seed for one option: FNV-1a, then a finaliser, so near seeds land far apart. */
export function hash(seed: string, key: string): number {
  let h = 0x811c9dc5;
  for (const ch of `${key}\u0000${seed}`) h = Math.imul(h ^ (ch.codePointAt(0) ?? 0), 0x01000193);
  h = Math.imul(h ^ (h >>> 16), 0x85ebca6b);
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
  return (h ^ (h >>> 16)) >>> 0;
}

const pick = <T>(seed: string, key: string, list: readonly T[]): T => list[hash(seed, key) % list.length];

/** The same pick; a colour to avoid gives way to the next one in the list, so only that pick moves. */
function pickNot(seed: string, key: string, list: string[], avoid: string[]): string {
  const i = hash(seed, key) % list.length;
  for (let n = 0; n < list.length; n++) {
    const c = list[(i + n) % list.length];
    if (!avoid.includes(c)) return c;
  }
  return list[i];
}

export function faceOptions(
  seed: string,
  set: { gender?: Gender | null; skin?: number | null } = {},
): FaceOptions {
  const gender = set.gender ?? pick(seed, 'gender', ['man', 'woman'] as const);
  const faceColor = set.skin ? SKIN[set.skin - 1] : pick(seed, 'skin', SKIN);
  const skip = NOT_ON[SKIN.indexOf(faceColor)];
  const hatStyle = pick(seed, 'hatStyle', HAT_STYLE);
  const hairColor = pickNot(seed, 'hairColor', HAIR, skip);
  const hatColor = pickNot(seed, 'hatColor', HAT, skip);
  const top = hatStyle === 'none' ? hairColor : hatColor;
  return {
    gender,
    skin: SKIN.indexOf(faceColor) + 1,
    faceColor,
    earSize: pick(seed, 'earSize', ['small', 'big'] as const),
    hairStyle: pick(seed, 'hairStyle', HAIR_STYLE[gender]),
    hairColor,
    hatStyle,
    hatColor,
    eyeBrowStyle: gender === 'man' ? 'up' : 'upWoman',
    eyeStyle: pick(seed, 'eyeStyle', ['circle', 'oval', 'smile'] as const),
    glassesStyle: pick(seed, 'glassesStyle', GLASSES),
    noseStyle: pick(seed, 'noseStyle', ['short', 'long', 'round'] as const),
    mouthStyle: pick(seed, 'mouthStyle', ['laugh', 'smile', 'peace'] as const),
    shirtStyle: pick(seed, 'shirtStyle', ['hoody', 'short', 'polo'] as const),
    shirtColor: pickNot(seed, 'shirtColor', SHIRT, [top]),
  };
}
