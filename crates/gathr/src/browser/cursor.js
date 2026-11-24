// The pointer a page shows at a place in it, in CSS pixels of the page: the `cursor` the element there
// computes to, looked for through same-origin frames and open shadow roots, where `elementFromPoint` on
// the document stops. Where it is `auto`, as a browser shows it: the text bar over a field or over text
// that can be selected, the hand over a link, else the arrow. Run by `pages.rs` as `(<this>)(x, y)`.
((x, y) => {
  let doc = document;
  let at = null;
  for (;;) {
    let el = doc.elementFromPoint(x, y);
    while (el && el.shadowRoot) {
      const inner = el.shadowRoot.elementFromPoint(x, y);
      if (!inner || inner === el) break;
      el = inner;
    }
    if (!el) break;
    at = el;
    let inner = null;
    try {
      inner = el.contentDocument;
    } catch {}
    if (!inner || !/^i?frame$/.test(el.localName)) break;
    const box = el.getBoundingClientRect();
    const style = getComputedStyle(el);
    x -= box.left + el.clientLeft + parseFloat(style.paddingLeft);
    y -= box.top + el.clientTop + parseFloat(style.paddingTop);
    doc = inner;
  }
  if (!at) return 'default';
  const style = (el) => el.ownerDocument.defaultView.getComputedStyle(el);
  const cursor = style(at).cursor;
  if (cursor !== 'auto') return cursor;
  const plain = /^(button|submit|reset|checkbox|radio|range|color|file|image|hidden)$/;
  const field = at.localName === 'textarea' || (at.localName === 'input' && !plain.test(at.type));
  if (field || at.isContentEditable) return 'text';
  if (at.closest('a[href]')) return 'pointer';
  const caret = doc.caretRangeFromPoint && doc.caretRangeFromPoint(x, y);
  const text = caret && caret.startContainer;
  if (!text || text.nodeType !== 3 || style(text.parentElement).userSelect === 'none') return 'default';
  const one = doc.createRange();
  for (const from of [caret.startOffset, caret.startOffset - 1]) {
    if (from < 0 || from >= text.length) continue;
    one.setStart(text, from);
    one.setEnd(text, from + 1);
    for (const r of one.getClientRects()) {
      if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return 'text';
    }
  }
  return 'default';
})
