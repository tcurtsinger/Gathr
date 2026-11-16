// Prints the milliseconds a duration text stands for: node src/cli.js 1h30m
const { parseDuration } = require('./duration');

function main(args) {
  const [text] = args;
  if (text === undefined) {
    console.error('Usage: node src/cli.js <duration>');
    return 2;
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
