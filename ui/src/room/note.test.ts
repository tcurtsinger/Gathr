import { expect, test } from 'vitest';
import { type Note, noteText, readNote } from './note';

const page = 'C:\\Gathr\\Designs\\Shop\\export-empty-state\\a.html';

test("a note's first line reads back as it was made, the person's words under it", () => {
  const note: Note = {
    letter: 'A',
    page,
    kind: 'button',
    words: 'Import orders',
    find: 'body > main > button',
  };
  const text = noteText(note, 'Make it quieter:\nan outline button.');
  expect(text.split('\n')[0]).toBe(
    `Design note on option A, ${page}: the button "Import orders" at body > main > button`,
  );
  expect(readNote(text)).toEqual({ note, words: 'Make it quieter:\nan outline button.' });
  const mac: Note = { ...note, page: '/Users/sam/Gathr/Designs/Shop/x/b.html', letter: 'B', kind: 'box' };
  expect(readNote(noteText(mac, 'Wider'))?.note).toEqual(mac);
});

test("a page's words can't break the line: quotes, breaks and length are made plain", () => {
  const note: Note = { letter: 'C', page, kind: 'text', words: 'Say "hi"\nthen: the link "x" at', find: 'p' };
  const read = readNote(noteText(note, 'Shorter'));
  expect(read?.note.words).toBe(`Say 'hi' then: the link 'x' at`);
  expect(read?.note.find).toBe('p');
  expect(read?.words).toBe('Shorter');
  const long = readNote(noteText({ ...note, words: 'w'.repeat(500), find: 'f'.repeat(900) }, ''));
  expect(read && long?.note.words.length).toBe(80);
  expect(long?.note.find.length).toBe(300);
});

test('a message that only looks like a note is an ordinary message', () => {
  expect(readNote('Design note on option A, a.html: the thing "x" at p\nwords')).toBeUndefined();
  expect(readNote('hello\nDesign note on option A, a.html: the button "x" at p')).toBeUndefined();
});
