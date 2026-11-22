/**
 * The Designs mark: "pencil-line" from Phosphor Icons (https://phosphoricons.com), Regular, MIT. Its drawing is
 * unchanged: a filled outline, so its lines keep their own width (1.5 on the icon set's grid) whatever a place gives
 * its icons, and it takes the colour of the words beside it. The notice is designs/notices/designs-mark.txt.
 */
export function DesignsMark() {
  return (
    <svg viewBox="0 0 256 256" fill="currentColor" stroke="none" aria-hidden="true">
      <path d={PENCIL} />
    </svg>
  );
}

// The file's one path, cut into pieces only to keep these lines short.
const PENCIL = [
  'M227.32,73.37,182.63,28.69a16,16,0,0,0-22.63,0L36.69,152A15.86,15.86,0,0,0,32,163.31V208a16,16,0,0,0,16,16H216',
  'a8,8,0,0,0,0-16H115.32l112-112A16,16,0,0,0,227.32,73.37ZM136,75.31,152.69,92,68,176.69,51.31,160ZM48,208V179.31',
  'L76.69,208Zm48-3.31L79.32,188,164,103.31,180.69,120Zm96-96L147.32,64l24-24L216,84.69Z',
].join('');
