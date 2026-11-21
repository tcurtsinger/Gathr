import { expect, test } from 'vitest';
import { readTable } from './FileBody';

test('a CSV reads with quoted commas, doubled quotes and line breaks, from either line ending', () => {
  const rows = [
    ['Order', 'Customer', 'Note'],
    ['1042', 'Ward, Lena', 'said "soon"'],
    ['1043', 'Bright & Co', 'two\nlines'],
  ];
  const unix = 'Order,Customer,Note\n1042,"Ward, Lena","said ""soon"""\n1043,Bright & Co,"two\nlines"\n';
  expect(readTable(unix, ',')).toEqual(rows);
  const windows =
    'Order,Customer,Note\r\n1042,"Ward, Lena","said ""soon"""\r\n1043,Bright & Co,"two\nlines"\r\n';
  expect(readTable(windows, ',')).toEqual(rows);
  expect(readTable('a\tb\r\n1\t2', '\t')).toEqual([
    ['a', 'b'],
    ['1', '2'],
  ]);
});

test("a file that doesn't read as a table is none", () => {
  expect(readTable('a,b\n1,"open', ',')).toBeNull();
  expect(readTable('a,b\n1,2,3', ',')).toBeNull();
});
