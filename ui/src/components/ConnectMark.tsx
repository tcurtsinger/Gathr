/**
 * Connect's mark in Settings: "Connect" from Round Filled Action Icons by Shannon E. Thomas, CC BY, as SVG Repo has
 * it (https://www.svgrepo.com/svg/412184/connect). Its drawing is unchanged: a filled shape, so it keeps its own
 * weight whatever a place gives its icons, and it takes the colour of the words beside it. The notice is
 * designs/notices/connect-mark.txt.
 */
export function ConnectMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" fill="currentColor" stroke="none" aria-hidden="true" className={className}>
      <path d={NODES} />
    </svg>
  );
}

// The file's one path, cut into pieces only to keep these lines short.
const NODES = [
  'M24,17c-0.665,0-1.302,0.113-1.9,0.312l-2.068-3.878C21.238,12.337,22,10.759,22,9 ',
  'c0-3.314-2.686-6-6-6s-6,2.686-6,6c0,1.759,0.762,3.337,1.968,4.434L9.9,17.312C9.302,17.113,8.665,17,8,17',
  'c-3.314,0-6,2.686-6,6 s2.686,6,6,6c2.972,0,5.433-2.163,5.91-5h4.18c0.477,2.837,2.938,5,5.91,5',
  'c3.314,0,6-2.686,6-6S27.314,17,24,17z M13.654,14.523 C14.375,14.83,15.167,15,16,15',
  's1.625-0.17,2.346-0.477l1.991,3.732c-1.168,0.903-1.991,2.226-2.247,3.744h-4.18 ',
  'c-0.255-1.518-1.078-2.841-2.247-3.744L13.654,14.523z',
].join('');
