import type { SVGProps } from 'react';

/** One part of a face: an <svg> at its box on the face, drawn in its own viewBox; the face carries any name. */
export const Part = (props: SVGProps<SVGSVGElement>) => <svg aria-hidden="true" {...props} />;
