// Adapted from react-nice-avatar 1.5.0 (tag v1.5.0, commit 4cae947), src/eyebrow, src/eyes and
// src/glasses. MIT: see LICENSE here.
// Drawings: the Avatar Illustration System by Micah Lanier, CC BY 4.0. Where each part sits: Drawing.tsx.
import { Part } from './part';

const SMILE_1 = [
  'M5.28675 34.0729C5.40099 34.8857 6.43424 35.0669 7.00876 34.4806',
  'C9.47388 31.9648 13.2637 30.1163 17.663 29.5936C20.2577 29.2853 22.7544 29.4749 24.9787 30.0657',
  'C25.7326 30.2659 26.4737 29.6294 26.2105 28.8951C24.5451 24.2497 19.8447 21.1962 14.7356 21.8033',
  'C8.79442 22.5093 4.55046 27.8978 5.25642 33.839C5.26572 33.9172 5.27583 33.9952 5.28675 34.0729Z',
].join(' ');

const SMILE_2 = [
  'M69.3848 24.0725C69.4976 24.8856 70.5308 25.0671 71.1062 24.4816',
  'C73.5487 21.9959 77.2977 20.1702 81.6484 19.6532C84.2128 19.3485 86.6804 19.5348 88.88 20.1167',
  'C89.6341 20.3162 90.3751 19.6795 90.1108 18.9456C88.456 14.3522 83.8041 11.3346 78.7482 11.9354',
  'C72.8624 12.6348 68.6579 17.9732 69.3573 23.8591C69.3658 23.9305 69.3749 24.0016 69.3848 24.0725',
  'Z',
].join(' ');

const ROUND_1 = [
  'M98.5 35C98.5 32.8783 97.6571 30.8434 96.1569 29.3431C94.6566 27.8429 92.6217 27 90.5 27',
  'C88.3783 27 86.3434 27.8429 84.8431 29.3431C83.3429 30.8434 82.5 32.8783 82.5 35',
].join(' ');

const SQUARE_1 = [
  'M32.4878 25.9393C31.618 22.4804 33.9396 19.0308 37.4716 18.5345L76.1882 13.0932',
  'C79.4696 12.632 82.5036 14.9183 82.9648 18.1998L88.5317 57.8105',
  'C88.9929 61.0919 86.7066 64.126 83.4252 64.5871L48.9526 69.4319',
  'C45.9161 69.8587 43.0465 67.9273 42.2987 64.9536L32.4878 25.9393Z',
].join(' ');

const SQUARE_2 = [
  'M154.751 8.96388C154.634 5.39915 151.451 2.7232 147.919 3.21958L109.203 8.66085',
  'C105.921 9.12202 103.635 12.156 104.096 15.4375L109.663 55.0482',
  'C110.124 58.3297 113.158 60.616 116.44 60.1548L150.912 55.31',
  'C153.949 54.8832 156.175 52.2357 156.074 49.1711L154.751 8.96388Z',
].join(' ');

export function Brows({ style }: { style: 'up' | 'upWoman' }) {
  return style === 'up' ? (
    <Part x={13} y={37} width={80} height={11.7} viewBox="0 0 149 48" fill="none">
      <path
        d="M99 10.2143C104.667 7.5476 118 5.11427 126 16.7143"
        stroke="black"
        strokeWidth="4"
        strokeLinecap="round"
      />
      <path
        d="M23.5791 35.521C25.6497 29.6104 33.2612 18.3959 47.1418 20.8224"
        stroke="black"
        strokeWidth="4"
        strokeLinecap="round"
      />
    </Part>
  ) : (
    <Part x={13} y={37} width={80} height={11.7} viewBox="0 0 149 51" fill="none">
      <path
        d="M99 13.2143C104.667 10.5476 118 8.11427 126 19.7143"
        stroke="black"
        strokeWidth="4"
        strokeLinecap="round"
      />
      <path
        d="M23.5791 38.521C25.6497 32.6104 33.2612 21.3959 47.1418 23.8224"
        stroke="black"
        strokeWidth="4"
        strokeLinecap="round"
      />
      <path d="M26.0742 32.4578L19.9258 27.0312" stroke="black" strokeWidth="4" strokeLinecap="round" />
      <path d="M122.961 14.157L129.109 8.73047" stroke="black" strokeWidth="4" strokeLinecap="round" />
      <path d="M32.5233 26.8142L28.4766 19.6816" stroke="black" strokeWidth="4" strokeLinecap="round" />
      <path d="M115.512 10.5135L119.559 3.38086" stroke="black" strokeWidth="4" strokeLinecap="round" />
      <path d="M40.5994 23.2005L38.3984 15.3008" stroke="black" strokeWidth="4" strokeLinecap="round" />
      <path d="M106.436 9.89973L108.637 2" stroke="black" strokeWidth="4" strokeLinecap="round" />
    </Part>
  );
}

export function Eyes({ style }: { style: 'circle' | 'oval' | 'smile' }) {
  if (style === 'circle')
    return (
      <Part x={3} y={43.3} width={100} height={10.8} viewBox="0 0 96 48" fill="none">
        <ellipse
          cx="16.1171"
          cy="28.9268"
          rx="9"
          ry="10"
          transform="rotate(-6.77646 16.1171 28.9268)"
          fill="black"
        />
        <ellipse
          cx="80.1486"
          cy="18.9231"
          rx="9"
          ry="10"
          transform="rotate(-6.27568 80.1486 18.9231)"
          fill="black"
        />
      </Part>
    );
  return style === 'oval' ? (
    <Part x={3} y={42.4} width={100} height={10.8} viewBox="0 0 96 48" fill="none">
      <ellipse
        cx="16.5301"
        cy="29.4023"
        rx="9"
        ry="13.5"
        transform="rotate(-6.77646 16.5301 29.4023)"
        fill="black"
      />
      <ellipse
        cx="80.5312"
        cy="19.4021"
        rx="9"
        ry="13.5"
        transform="rotate(-6.27568 80.5312 19.4021)"
        fill="black"
      />
    </Part>
  ) : (
    <Part x={3} y={42.4} width={100} height={12.6} viewBox="0 0 96 48" fill="none">
      <path fillRule="evenodd" clipRule="evenodd" d={SMILE_1} fill="#171921" />
      <path fillRule="evenodd" clipRule="evenodd" d={SMILE_2} fill="#171921" />
    </Part>
  );
}

export function Glasses({ style }: { style: 'none' | 'round' | 'square' }) {
  if (style === 'none') return null;
  return style === 'round' ? (
    <Part x={0} y={38.35} width={100} height={18} viewBox="0 0 152 65" fill="none">
      <circle cx="123.5" cy="28" r="26" stroke="black" strokeWidth="4" />
      <circle cx="56.5" cy="37" r="26" stroke="black" strokeWidth="4" />
      <path d={ROUND_1} stroke="black" strokeWidth="4" />
      <path d="M31 39L1 44.5" stroke="black" strokeWidth="4" />
    </Part>
  ) : (
    <Part x={0} y={37.45} width={100} height={19.8} viewBox="0 0 160 74" fill="none">
      <path d="M36.5 42.5L2 49.125" stroke="black" strokeWidth="4" strokeLinecap="round" />
      <path d={SQUARE_1} stroke="black" strokeWidth="4" />
      <path d={SQUARE_2} stroke="black" strokeWidth="4" />
      <path d="M85.5 37.125L107.5 33.625" stroke="black" strokeWidth="4" />
    </Part>
  );
}
