// Every test gets the app's styles, as the app entry does: without them WebKit shows a registry tooltip
// unstyled, in the flow, where it can cover the menu item a test clicks next (archive.test.tsx did).
import { beforeAll } from 'vitest';
import './theme.css';

// The font arrives by swap, whenever the file does: a swap mid-test moves the text a test measures.
// beforeAll runs after the imports, so the font face is known.
beforeAll(async () => {
  await document.fonts.load('15px "Hanken Grotesk Variable"');
});
