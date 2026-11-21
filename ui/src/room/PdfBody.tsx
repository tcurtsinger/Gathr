import type { PDFDocumentProxy, PDFPageProxy, RenderTask } from 'pdfjs-dist';
import { useEffect, useRef, useState } from 'react';
import { api, Failure, type Unshown } from '../api';
import './documents.css';

interface Props {
  chat: string;
  path: string;
  onUnshown: (why: Unshown) => void;
  onFailure: (failure: Failure) => void;
}

/** A PDF's pages one under another, each the tab's width. One PDF.js can't read, or locked with a password, isn't
 * shown. */
export function PdfBody({ chat, path, onUnshown, onFailure }: Props) {
  const [doc, setDoc] = useState<{ pdf: PDFDocumentProxy; ratio: number }>();
  const [width, setWidth] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let live = true;
    let destroy: (() => unknown) | undefined;
    (async () => {
      const got = await api.pdf(chat, path);
      if (!live) return;
      if ('unshown' in got) return onUnshown(got.unshown);
      const { getDocument } = await import('./pdf');
      const task = getDocument({ data: new Uint8Array(got.bytes) });
      destroy = () => task.destroy();
      const pdf = await task.promise;
      const first = (await pdf.getPage(1)).getViewport({ scale: 1 });
      if (live) setDoc({ pdf, ratio: first.height / first.width });
    })().catch((e) => {
      if (live) e instanceof Failure ? onFailure(e) : onUnshown('cant');
    });
    return () => {
      live = false;
      destroy?.();
    };
  }, [chat, path, onUnshown, onFailure]);
  useEffect(() => {
    const at = box.current;
    if (!at) return;
    const sized = new ResizeObserver(([e]) => setWidth(Math.floor(e.contentRect.width)));
    sized.observe(at);
    return () => sized.disconnect();
  }, []);
  return (
    <div ref={box} className="pdf">
      {doc &&
        width > 0 &&
        Array.from({ length: doc.pdf.numPages }, (_, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: a page's number is what it is
          <Page key={i + 1} pdf={doc.pdf} n={i + 1} width={width} ratio={doc.ratio} />
        ))}
    </div>
  );
}

/** Pages drawn ahead of the view, above and below it: a screen's height. */
const AHEAD = '100% 0px';

/**
 * One page, drawn only while it is in or near the view, sharp at the screen's density; out of it, its drawing is let
 * go, so a long PDF holds only a few pages' pixels. Until it is read, it has the first page's shape.
 */
function Page({ pdf, n, width, ratio }: { pdf: PDFDocumentProxy; n: number; width: number; ratio: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [near, setNear] = useState(false);
  const [own, setOwn] = useState<number>();
  useEffect(() => {
    const at = ref.current;
    if (!at) return;
    const seen = new IntersectionObserver(([e]) => setNear(e.isIntersecting), {
      root: at.parentElement,
      rootMargin: AHEAD,
    });
    seen.observe(at);
    return () => seen.disconnect();
  }, []);
  useEffect(() => {
    const drawn = canvas.current;
    if (!near || !drawn) return;
    let live = true;
    let page: PDFPageProxy | undefined;
    let task: RenderTask | undefined;
    pdf
      .getPage(n)
      .then((p) => {
        page = p;
        if (!live) return;
        const base = p.getViewport({ scale: 1 });
        setOwn(base.height / base.width);
        const view = p.getViewport({ scale: (width / base.width) * (window.devicePixelRatio || 1) });
        drawn.width = Math.floor(view.width);
        drawn.height = Math.floor(view.height);
        task = p.render({ canvas: drawn, viewport: view });
        return task.promise;
      })
      // Cancelled, or a page PDF.js can't draw: it stays blank.
      .catch(() => {});
    return () => {
      live = false;
      task?.cancel();
      page?.cleanup();
    };
  }, [near, pdf, n, width]);
  return (
    <div ref={ref} className="pdf-page" style={{ height: Math.round(width * (own ?? ratio)) }}>
      {near && <canvas ref={canvas} />}
    </div>
  );
}
