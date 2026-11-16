// Durations as short text, such as "1h30m", "90s" or "250ms", read into milliseconds.

const UNITS = [
  ['h', 3600000],
  ['m', 60000],
  ['s', 1000],
  ['ms', 1],
];

const WHOLE = /^(\d+(ms|h|m|s))+$/;
const PART = /(\d+)(ms|h|m|s)/g;

// The milliseconds a duration text stands for, or NaN when it isn't one.
function parseDuration(text) {
  if (!WHOLE.test(text)) return NaN;
  let total = 0;
  for (const [, count, unit] of text.matchAll(PART)) {
    const [, size] = UNITS.find(([name]) => name === unit);
    total += Number(count) * size;
  }
  return total;
}

module.exports = { parseDuration };
