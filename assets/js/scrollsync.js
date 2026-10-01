// Scroll together: in "Show formatting codes", the text box and the preview
// follow each other. They are lined up by heading, the one place the text
// and the preview are sure to match, and in between by how far through the
// section each one is. So a tall picture or table in one section never
// throws the rest of the page off.

const HEADING = /^ {0,3}#{1,6}(\s|$)/;
const FENCE = /^ {0,3}(```|~~~)/;

/** Line numbers of the headings in Markdown text, skipping code blocks. */
export function headingLines(text) {
  const lines = text.split('\n');
  const out = [];
  let inCode = false;
  lines.forEach((line, i) => {
    if (FENCE.test(line)) inCode = !inCode;
    else if (!inCode && HEADING.test(line)) out.push(i);
  });
  return out;
}

/** Where `y` in one list of points falls in the other: straight lines
 *  between matching points. Both lists rise and have the same length. */
export function mapPosition(y, from, to) {
  for (let i = 0; i < from.length - 1; i++) {
    if (y <= from[i + 1]) {
      const span = from[i + 1] - from[i];
      const t = span > 0 ? (y - from[i]) / span : 0;
      return to[i] + t * (to[i + 1] - to[i]);
    }
  }
  return to[to.length - 1];
}

/** How far down the text box each of these lines starts, found with a
 *  hidden copy of the box that wraps its lines the same way. */
function lineTops(ta, lineNumbers) {
  const style = getComputedStyle(ta);
  const mirror = document.createElement('div');
  for (const prop of ['fontFamily', 'fontSize', 'fontWeight', 'lineHeight', 'letterSpacing',
    'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft', 'tabSize', 'wordSpacing']) {
    mirror.style[prop] = style[prop];
  }
  Object.assign(mirror.style, {
    position: 'absolute', visibility: 'hidden', top: '0', left: '-9999px',
    boxSizing: 'border-box', width: `${ta.clientWidth}px`,
    whiteSpace: 'pre-wrap', overflowWrap: 'break-word',
  });
  const lines = ta.value.split('\n');
  const marks = [];
  let from = 0;
  for (const n of lineNumbers) {
    mirror.append(document.createTextNode(lines.slice(from, n).map((l) => `${l}\n`).join('')));
    const mark = document.createElement('span');
    marks.push(mark);
    mirror.append(mark);
    from = n;
  }
  document.body.append(mirror);
  const tops = marks.map((m) => m.offsetTop);
  mirror.remove();
  return tops;
}

/**
 * Keep `ta` (the text box) and `preview` (the preview's scrolling box)
 * scrolled to the same part of the page while `isOn()` says so.
 * Returns { refresh(), destroy() }: call refresh when the text or the
 * preview changes.
 */
export function scrollTogether(ta, preview, isOn) {
  let points = null; // [text box positions, preview positions]
  let settingBy = null; // the box we just moved, whose next scroll event is ours
  const top = (el) => el.getBoundingClientRect().top - preview.getBoundingClientRect().top + preview.scrollTop;

  function measure() {
    const lines = headingLines(ta.value);
    const headings = [...preview.querySelectorAll(':scope > :is(h1, h2, h3, h4, h5, h6)')];
    const taEnd = ta.scrollHeight - ta.clientHeight;
    const pvEnd = preview.scrollHeight - preview.clientHeight;
    const src = [0];
    const dst = [0];
    // Only line up headings when both sides agree how many there are (a
    // heading typed inside a quote, say, shows differently); otherwise go
    // by how far down the page each is.
    if (lines.length === headings.length) {
      const tops = lineTops(ta, lines);
      headings.forEach((hd, i) => {
        const a = Math.min(tops[i], taEnd);
        const b = Math.min(top(hd), pvEnd);
        if (a > src.at(-1) && b > dst.at(-1)) { src.push(a); dst.push(b); }
      });
    }
    if (taEnd > src.at(-1) && pvEnd > dst.at(-1)) { src.push(taEnd); dst.push(pvEnd); }
    points = src.length > 1 ? [src, dst] : null;
  }

  const follow = (driver, follower, forward) => () => {
    if (settingBy === driver) { settingBy = null; return; }
    if (!isOn()) return;
    if (!points) measure();
    if (!points) return;
    const [src, dst] = forward ? points : [points[1], points[0]];
    const y = Math.round(mapPosition(driver.scrollTop, src, dst));
    if (Math.abs(follower.scrollTop - y) < 1) return;
    settingBy = follower;
    follower.scrollTop = y;
    // Its scroll event comes before the next frame; if none comes, forget.
    requestAnimationFrame(() => { if (settingBy === follower) settingBy = null; });
  };
  const fromText = follow(ta, preview, true);
  const fromPreview = follow(preview, ta, false);
  const remeasure = () => { points = null; };
  ta.addEventListener('scroll', fromText, { passive: true });
  preview.addEventListener('scroll', fromPreview, { passive: true });
  window.addEventListener('resize', remeasure);

  return {
    /** The text or the preview changed: measure again when next needed,
     *  and bring the preview to where the text box is now. */
    refresh() {
      points = null;
      if (isOn()) fromText();
    },
    destroy() { window.removeEventListener('resize', remeasure); },
  };
}
