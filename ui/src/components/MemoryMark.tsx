/**
 * Memory's mark, on the rail and on a noted line in the chat: "Book Bookmark" from the Solar icon set (Linear) by
 * 480 Design, CC BY 4.0. Its drawing is unchanged; it takes the colour of the words beside it and the line width each
 * place gives its icons, its file's own (1.5) where a place gives none. The notice is designs/notices/memory-mark.txt.
 */
export function MemoryMark() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <path d={COVER} />
      <path d={SPINE} />
      <path d={LINE_ONE} strokeLinecap="round" />
      <path d={LINE_TWO} strokeLinecap="round" />
      <path d={BOOKMARK} strokeLinecap="round" />
    </svg>
  );
}

// The file's five paths, the long ones cut into pieces only to keep these lines short.
const COVER = [
  'M4 8C4 5.17157 4 3.75736 4.87868 2.87868C5.75736 2 7.17157 2 10 2H14C16.8284 2 18.2426 2 19.1213 2.8',
  '7868C20 3.75736 20 5.17157 20 8V16C20 18.8284 20 20.2426 19.1213 21.1213C18.2426 22 16.8284 22 14 22',
  'H10C7.17157 22 5.75736 22 4.87868 21.1213C4 20.2426 4 18.8284 4 16V8Z',
].join('');
const SPINE =
  'M19.8978 16H7.89778C6.96781 16 6.50282 16 6.12132 16.1022C5.08604 16.3796 4.2774 17.1883 4 18.2235';
const LINE_ONE = 'M8 7H16';
const LINE_TWO = 'M8 10.5H13';
const BOOKMARK = [
  'M13 16V19.5309C13 19.8065 13 19.9443 12.9051 20C12.8103 20.0557 12.6806 19.9941 12.4211 19.8708L11.1',
  '789 19.2808C11.0911 19.2391 11.0472 19.2182 11 19.2182C10.9528 19.2182 10.9089 19.2391 10.8211 19.28',
  '08L9.57889 19.8708C9.31943 19.9941 9.18971 20.0557 9.09485 20C9 19.9443 9 19.8065 9 19.5309V16.45',
].join('');
