// Prints the milliseconds a duration text stands for: node src/cli.js 1h30m
// With --format, the other way: node src/cli.js --format 5400000 prints 1h30m.
const { parseDuration, formatDuration } = require('./duration');

function main(args) {
  const [text] = args;
  if (text === undefined) {
    console.error('Usage: node src/cli.js <duration> | --format <milliseconds>');
    return 2;
  }
  if (text === '--format') {
    console.log(formatDuration(Number(args[1])));
    return 0;
  }
  const ms = parseDuration(text);
  if (Number.isNaN(ms)) {
    console.error(`Not a duration: ${text}`);
    return 1;
  }
  console.log(ms);
  return 0;
}

process.exitCode = main(process.argv.slice(2));
