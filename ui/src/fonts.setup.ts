// A test that imports theme.css gets its font by swap, whenever the file arrives: a swap mid-test moves
// the text the test measures. beforeAll runs after the test file's imports, so the font face is known.
import { beforeAll } from 'vitest';

beforeAll(async () => {
  await document.fonts.load('15px "Hanken Grotesk Variable"');
});
