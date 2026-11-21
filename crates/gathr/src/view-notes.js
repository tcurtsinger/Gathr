// Gathr's notes on a design's page (`view::NOTES`), added after the page as it is served in a chat's design tab. With
// Comment on, the part under the pointer is outlined, and a click goes to Gathr's window in place of the page; the
// window's pins are drawn on the parts they name. It defines no global name, and draws only in a layer of its own.
(() => {
  const KINDS = [
    ['button', 'button,[role=button],input[type=button],input[type=submit],input[type=reset]'],
    ['link', 'a[href]'],
    ['field', 'input,textarea,select'],
    ['heading', 'h1,h2,h3,h4,h5,h6'],
    ['picture', 'img,svg,picture,canvas,video'],
  ];
  const cut = (s, n) => String(s ?? '').replace(/\s+/g, ' ').trim().slice(0, n);
  // Text: an element that holds words of its own, not only in the parts inside it.
  const own = (e) => [...e.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
  const kindOf = (e) => KINDS.find((k) => e.matches(k[1]))?.[0] ?? (own(e) ? 'text' : 'box');
  const wordsOf = (e) =>
    cut(e.getAttribute('aria-label') || e.getAttribute('alt') || e.getAttribute('placeholder') || e.innerText, 80);
  // The part a click means: the button, link, field or heading it is in, a whole drawing, else what is under it.
  const partOf = (e) =>
    e instanceof Element ? (e.closest(KINDS.slice(0, 4).map((k) => k[1]).join(',')) ?? e.closest('svg') ?? e) : null;
  // A path from the body that finds it again: an id where there is a plain one, else each step's tag and place.
  const pathOf = (e) => {
    const steps = [];
    for (let at = e; at && at !== document.body && at !== document.documentElement; at = at.parentElement) {
      if (/^[A-Za-z][\w-]*$/.test(at.id) && document.querySelectorAll(`#${at.id}`).length === 1) {
        steps.unshift(`#${at.id}`);
        return steps.join(' > ');
      }
      const tag = at.localName;
      const same = [...(at.parentElement?.children ?? [])].filter((c) => c.localName === tag);
      steps.unshift(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(at) + 1})` : tag);
    }
    return ['body', ...steps].join(' > ');
  };
  const find = (pin) => {
    const like = (e) => e && kindOf(e) === pin.kind;
    const same = (e) => like(e) && wordsOf(e) === pin.words;
    let at = null;
    try {
      at = document.querySelector(pin.find);
    } catch {}
    if (same(at)) return at;
    // Only a part with words is looked for elsewhere: any empty box would do for one without.
    const elsewhere = pin.words ? [...document.body.querySelectorAll('*')].find(same) : null;
    return elsewhere ?? (like(at) ? at : null);
  };

  // The layer: a host of its own at the page's end, its looks sealed in its own shadow, taking no clicks.
  const host = document.createElement('gathr-notes');
  const layer = host.attachShadow({ mode: 'closed' });
  layer.innerHTML = `<style>
    :host { all: initial; position: fixed; inset: 0; z-index: 2147483647; pointer-events: none; }
    div { position: fixed; box-sizing: border-box; }
    .outline { border: 2px solid #4a8cff; border-radius: 4px; box-shadow: 0 0 0 3px #4a8cff33; display: none; }
    .pin { width: 20px; height: 20px; margin: -10px 0 0 -10px; border-radius: 50%; background: #4a8cff; color: #fff;
      font: 600 11px/20px system-ui, sans-serif; text-align: center; box-shadow: 0 0 0 2px #fff; }
    .pin.settled { background: #6b6f76; }
  </style><div class="outline"></div>`;
  const outline = layer.querySelector('.outline');
  document.documentElement.append(host);

  let on = false;
  // Each pin with the part it was found on, looked for once each time the window gives them.
  let found = [];
  const box = (div, e) => {
    const r = e.getBoundingClientRect();
    Object.assign(div.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` });
    return r;
  };
  const place = () => {
    for (const div of layer.querySelectorAll('.pin')) div.remove();
    for (const [pin, e] of found) {
      const r = e.getBoundingClientRect();
      if (!e.isConnected || (!r.width && !r.height)) continue;
      const div = document.createElement('div');
      div.className = pin.settled ? 'pin settled' : 'pin';
      // Escaped: a page served without a charset would read the check's bytes wrong.
      div.textContent = pin.settled ? '\u2713' : String(pin.n);
      Object.assign(div.style, { left: `${r.right}px`, top: `${r.top}px` });
      layer.append(div);
    }
  };
  addEventListener('scroll', place, { capture: true, passive: true });
  addEventListener('resize', place);

  addEventListener('message', (e) => {
    if (e.source !== parent || e.data?.gathr !== 'notes') return;
    on = e.data.on === true;
    const pins = Array.isArray(e.data.pins) ? e.data.pins : [];
    found = pins.map((pin) => [pin, find(pin)]).filter((f) => f[1]);
    // The window tells again as the pointer leaves the frame: the outline goes until the pointer is back.
    outline.style.display = 'none';
    place();
  });
  addEventListener(
    'pointermove',
    (e) => {
      const part = on && partOf(e.composedPath()[0]);
      outline.style.display = part ? 'block' : 'none';
      if (part) box(outline, part);
    },
    true,
  );
  // While Comment is on, a press on the page is the person's pointing, never the page's own.
  for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'dblclick', 'auxclick', 'click']) {
    addEventListener(
      type,
      (e) => {
        if (!on) return;
        e.preventDefault();
        e.stopImmediatePropagation();
        const part = type === 'click' && partOf(e.composedPath()[0]);
        if (!part) return;
        const r = box(outline, part);
        const rect = { x: r.left, y: r.top, w: r.width, h: r.height };
        parent.postMessage({ gathr: 'clicked', kind: kindOf(part), words: wordsOf(part), find: pathOf(part), rect }, '*');
      },
      true,
    );
  }
})();
