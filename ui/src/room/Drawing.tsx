import { useEffect, useState } from 'react';
import { Diagram } from '@/components/assistant-ui/elements/diagram';
import { api, type Block } from '../api';
import { PictureView, pictureUrl } from '../components/FileTile';
import { fileName } from '../components/files';

const isLight = () => document.documentElement.classList.contains('light');

/** Whether the window is light, following a change of theme. */
function useLight() {
  const [light, setLight] = useState(isLight);
  useEffect(() => {
    const seen = new MutationObserver(() => setLight(isLight()));
    seen.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
    return () => seen.disconnect();
  }, []);
  return light;
}

/** The window's colours as plain values, the diagram frame's own surface behind: a picture can't read the page's. */
function colours(light: boolean) {
  const style = getComputedStyle(document.documentElement);
  const token = (name: string) => style.getPropertyValue(name).trim();
  return {
    bg: token(light ? '--background' : '--popover'),
    fg: token('--foreground'),
    muted: token('--muted-foreground'),
    line: token('--muted-foreground'),
    border: token('--border'),
    accent: token('--attention'),
    transparent: true,
  };
}

/** An SVG as a picture's address. */
const picture = (svg: string) => URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));

/**
 * A diagram an agent showed: mermaid source drawn to SVG, or a picture file from the chat's folders, in a frame with
 * Gathr's zoom. The SVG is only ever a picture, an <img> of a blob: an SVG drawn that way runs no script, loads nothing
 * and can't reach the page, so nothing an agent writes in it can. Source that doesn't draw shows as plain text.
 */
export function Drawing({
  chat,
  diagram,
  title,
}: {
  chat: string;
  diagram: Block['diagram'];
  title?: string;
}) {
  const light = useLight();
  const { mermaid, file } = diagram ?? {};
  const svgFile = file !== undefined && /\.svg$/i.test(file);
  // Only mermaid is drawn in the window's colours.
  const shade = mermaid !== undefined && light;
  const [drawn, setDrawn] = useState<{ src?: string; failed?: boolean }>({});
  const [size, setSize] = useState<{ width: number; height: number }>();
  const [zoom, setZoom] = useState(1);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (file !== undefined && !svgFile) {
      setDrawn({ src: pictureUrl(chat, file) });
      return;
    }
    let live = true;
    let url: string | undefined;
    // The mermaid library loads only when a diagram is on screen.
    const svg =
      mermaid !== undefined
        ? import('beautiful-mermaid').then(({ renderMermaidSVG }) =>
            renderMermaidSVG(mermaid, colours(shade)),
          )
        : file !== undefined
          ? api.words(chat, file).then((got) => ('text' in got ? got.text : Promise.reject()))
          : Promise.reject();
    svg.then(
      (svg) => {
        if (!live) return;
        url = picture(svg);
        setDrawn({ src: url });
      },
      () => live && setDrawn({ failed: true }),
    );
    return () => {
      live = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [chat, mermaid, file, svgFile, shade]);
  const name = title || (file ? fileName(file) : 'Diagram');
  // The frame carries the title; without it, the title is the quiet line above.
  if (drawn.failed) {
    return (
      <>
        {title && <div className="shown-title">{title}</div>}
        <pre className="shown-source">{mermaid ?? file}</pre>
      </>
    );
  }
  const step = (by: number) => setZoom((z) => Math.min(3, Math.max(0.5, z + by)));
  return (
    <>
      <Diagram
        className="shown-diagram"
        title={name}
        zoom={zoom}
        onZoomIn={() => step(0.25)}
        onZoomOut={() => step(-0.25)}
        onReset={() => setZoom(1)}
        onExpand={drawn.src ? () => setOpen(true) : undefined}
      >
        {drawn.src && (
          <img
            src={drawn.src}
            alt={name}
            width={size && Math.round(size.width * zoom)}
            height={size && Math.round(size.height * zoom)}
            onLoad={(e) =>
              setSize({ width: e.currentTarget.naturalWidth, height: e.currentTarget.naturalHeight })
            }
            onError={() => setDrawn({ failed: true })}
          />
        )}
      </Diagram>
      {open && drawn.src && <PictureView name={name} src={drawn.src} onClose={() => setOpen(false)} />}
    </>
  );
}
