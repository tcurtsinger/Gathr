/** Two completed tasks and one pending, in the title bar's stroke style. */
export function BoardMark() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="m3 5 2 2 3-3M11 6h10M3 12l2 2 3-3M11 13h10M11 20h10" />
      <circle cx="5" cy="20" r="1" fill="currentColor" stroke="none" />
    </svg>
  );
}
