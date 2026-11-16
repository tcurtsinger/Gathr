# durations

Durations as short text, such as `1h30m`, `90s` or `250ms`, read into milliseconds.

```js
const { parseDuration } = require('./src/duration');
parseDuration('1h30m'); // 5400000
```

From the command line:

```
node src/cli.js 1h30m
```

prints `5400000`. Units are `h`, `m`, `s` and `ms`, largest first, in lower case.

## Tests

```
node --test
```

No dependencies: Node's own test runner and nothing else. How changes are made is in
CONTRIBUTING.md.
