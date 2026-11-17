// Adapted from react-nice-avatar 1.5.0 (tag v1.5.0, commit 4cae947), src/nose, src/mouth. MIT: see LICENSE.
// Drawings: the Avatar Illustration System by Micah Lanier, CC BY 4.0. Where each part sits: Drawing.tsx.
import { Part } from './part';

const NOSE_ROUND_1 = [
  'M12.307 12.3397C17.753 11.0993 26.6843 12.9603 24.7238 22.8833',
  'C22.9813 31.7023 13.6141 32.1857 11 29.7048',
].join(' ');

const LAUGH_1 = [
  'M69.9204 13.3305C69.7501 11.263 67.6363 10.1117 65.8448 10.8176',
  'C61.6161 12.4839 47.6267 17.7834 37.7627 19.1403C26.9401 20.6291 10.5503 18.821 5.80622 18.2319',
  'C3.87694 17.9923 2.15721 19.6504 2.49021 21.6654C3.14178 25.6081 4.48399 29.4088 6.45857 32.8914',
  'C8.6608 36.7754 11.6065 40.1877 15.1274 42.9333C18.6484 45.679 22.6757 47.7042 26.9793 48.8935',
  'C31.283 50.0827 35.7787 50.4126 40.2099 49.8644C44.6411 49.3162 48.9208 47.9005 52.8049 45.6983',
  'C56.689 43.4961 60.1012 40.5504 62.8469 37.0294C65.5925 33.5085 67.6178 29.4812 68.807 25.1775',
  'C69.8726 21.3212 70.2483 17.3106 69.9204 13.3305Z',
].join(' ');

const LAUGH_2 = [
  'M67.7931 12.1925C68.3091 16.363 67.9985 20.5943 66.8793 24.6448',
  'C65.76 28.6953 63.8539 32.4857 61.2697 35.7996C58.6856 39.1134 55.474 41.8858 51.8185 43.9585',
  'C48.1629 46.0312 44.1348 47.3636 39.9643 47.8795C35.7938 48.3955 31.5625 48.085 27.512 46.9657',
  'C23.4615 45.8464 19.6712 43.9403 16.3573 41.3562C13.0435 38.772 10.2711 35.5605 8.19838 31.9049',
  'C6.12569 28.2493 4.79332 24.2213 4.27734 20.0508L67.7931 12.1925Z',
].join(' ');

const SMILE_1 = [
  'M10.0007 24.1649C14.941 30.6115 22.4277 37.7537 33.9767 36.125',
  'C45.5257 34.4963 50.6642 26.5297 49.1492 20.0779',
].join(' ');

export function Nose({ style }: { style: 'short' | 'long' | 'round' }) {
  if (style === 'short')
    return (
      <Part x={49} y={50.5} width={10} height={9} viewBox="0 0 32 40" fill="none">
        <path
          d="M16.5 7C16.1667 10.8333 16.5 19.2 20.5 22C25.5 25.5 20 34 10 32"
          stroke="#171921"
          strokeWidth="4"
        />
      </Part>
    );
  return style === 'long' ? (
    <Part x={49} y={50.5} width={10} height={9} viewBox="0 0 32 40" fill="none">
      <path d="M16.5 3C16.5 17 23.5 28 23.5 28C23.5 28 20 34 10 32" stroke="#171921" strokeWidth="4" />
    </Part>
  ) : (
    <Part x={49} y={50.5} width={10} height={9} viewBox="0 0 32 40" fill="none">
      <path d={NOSE_ROUND_1} stroke="black" strokeWidth="4" />
    </Part>
  );
}

/** `id` makes the laugh's mask id unique on the page. */
export function Mouth({ style, id }: { style: 'laugh' | 'smile' | 'peace'; id: string }) {
  if (style === 'laugh')
    return (
      <Part x={30} y={57.7} width={50} height={17.1} viewBox="0 0 73 64" fill="none">
        <path d={LAUGH_1} fill="#171921" stroke="#171921" strokeWidth="4" />
        <mask
          id={`${id}-mouth`}
          style={{ maskType: 'alpha' }}
          maskUnits="userSpaceOnUse"
          x="4"
          y="12"
          width="65"
          height="37"
        >
          <path d={LAUGH_2} fill="#171921" />
        </mask>
        <g mask={`url(#${id}-mouth)`}>
          <circle
            cx="40.5221"
            cy="52.3146"
            r="21.5"
            transform="rotate(-7.05286 40.5221 52.3146)"
            fill="#FC909F"
          />
        </g>
      </Part>
    );
  return style === 'smile' ? (
    <Part x={30} y={57.7} width={50} height={15.3} viewBox="0 0 64 64" fill="none">
      <path d={SMILE_1} stroke="black" strokeWidth="4" />
    </Part>
  ) : (
    <Part x={30} y={57.7} width={50} height={17.1} viewBox="0 0 67 64" fill="none">
      <path d="M2.5 17.5C5 34.5 33.5 42.5 59.5 23" stroke="black" strokeWidth="4" />
    </Part>
  );
}
