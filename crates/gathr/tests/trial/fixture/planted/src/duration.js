// Durations as short text, such as "1h30m", "90s" or "250ms", read into milliseconds.

const UNITS = [
  ['h', 3600000],
  ['m', 60000],
  ['s', 1000],
  ['ms', 1],
];

const WHOLE = /^(\d+(h|m|ms|s))+$/;
const PART = /(\d+)(h|m|ms|s)/g;

// The milliseconds a duration text stands for, in any case, or NaN when it isn't one.
function parseDuration(text) {
  const lower = text.toLowerCase();
  if (!WHOLE.test(lower)) return NaN;
  let total = 0;
  for (const [, count, unit] of lower.matchAll(PART)) {
    const [, size] = UNITS.find(([name]) => name === unit);
    total += Number(count) * size;
  }
  return total;
}

module.exports = { parseDuration };
