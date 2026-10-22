// The first-run tour: a few steps that point at the real screen, one at a
// time. The card is a modal <dialog>, so the page behind it can't be reached
// and Escape closes it; Cairn also closes open dialogs when the address
// changes, which ends the tour if someone navigates away.

import { h, button } from './core.js';

const STEPS = [
  {
    title: 'Welcome to Cairn',
    text: 'Here are the five things worth knowing. It takes about a minute, and you can skip it at any time.',
  },
  {
    target: '#sidenav', drawer: true,
    title: 'Find what you need',
    text: 'Pages are kept in folders. Choose a folder to see what is inside it.',
  },
  {
    target: '.hero-search',
    title: 'Or search',
    text: 'Type any word and Cairn lists every page that has it.',
  },
  {
    target: '.btn-new', drawer: true,
    title: 'Add or change a page',
    text: 'Choose “New page”, or open a page and choose “Edit this page”. Nobody else sees your changes until you choose “Publish”. Every page keeps its earlier versions, and deleted pages wait in “Recently deleted”, so mistakes can be undone.',
  },
  {
    target: '.topbar-actions',
    title: 'Help is always here',
    text: '“Help” explains what you can do on the screen you are looking at. In “Settings” you can choose the name others see while you edit, and see this tour again.',
  },
];

const RING_PADDING = 4;
const CARD_GAP = 12;
const EDGE = 8;

/**
 * Starts the tour on the home screen. `openNav`/`closeNav` show and hide the
 * page list when it is a drawer (narrow windows); `isDrawer` says whether it
 * is one. `onEnd` runs once, however the tour ends.
 */
export function startTour({ openNav, closeNav, isDrawer, onEnd }) {
  // Steps whose part of the screen isn't there (no "New page" when the folder
  // is read only) are left out.
  const steps = STEPS.filter((s) => !s.target || document.querySelector(s.target));
  let index = 0;

  const ring = h('div', { class: 'tour-ring', hidden: true });
  const count = h('p', { class: 'tour-count' });
  const title = h('h2', { id: 'tour-title' });
  const text = h('p', { id: 'tour-text' });
  const skip = button('Skip tour', { kind: 'quiet', onClick: () => card.close('skip') });
  const back = button('Back', { icon: 'back', onClick: () => show(index - 1) });
  const next = button('Next', {
    kind: 'primary',
    onClick: () => (index === steps.length - 1 ? card.close('done') : show(index + 1)),
  });
  const nextLabel = next.querySelector('span');
  const card = h('dialog', { class: 'tour-card', 'aria-labelledby': 'tour-title', 'aria-describedby': 'tour-text' },
    // Read out again whenever the step changes.
    h('div', { 'aria-live': 'polite' }, count, title, text),
    h('div', { class: 'actions' }, skip, back, next));

  // Always looked up again: the page list is redrawn a moment after the
  // folder opens, so a remembered element can be a detached copy.
  const targetOf = (step) => (step.target ? document.querySelector(step.target) : null);

  function place() {
    const rect = targetOf(steps[index])?.getBoundingClientRect();
    ring.hidden = !rect;
    card.classList.toggle('no-target', !rect);
    if (rect) {
      Object.assign(ring.style, {
        top: `${rect.top - RING_PADDING}px`, left: `${rect.left - RING_PADDING}px`,
        width: `${rect.width + RING_PADDING * 2}px`, height: `${rect.height + RING_PADDING * 2}px`,
      });
    }
    placeCard(rect);
  }

  /** Below the highlight if it fits, else beside it, else above, else centred. */
  function placeCard(rect) {
    card.classList.toggle('is-docked', isDrawer());
    if (isDrawer()) {
      // A phone: the card sits along the bottom edge, whatever is lit.
      Object.assign(card.style, { top: '', left: '' });
      return;
    }
    const { offsetWidth: w, offsetHeight: ht } = card;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let left = (vw - w) / 2;
    let top = (vh - ht) / 2;
    if (rect) {
      if (rect.bottom + CARD_GAP + ht <= vh) { top = rect.bottom + CARD_GAP; left = rect.left; }
      else if (rect.right + CARD_GAP + w <= vw) { left = rect.right + CARD_GAP; top = rect.top; }
      else if (rect.top - CARD_GAP - ht >= 0) { top = rect.top - CARD_GAP - ht; left = rect.left; }
    }
    Object.assign(card.style, {
      left: `${Math.min(Math.max(left, EDGE), Math.max(EDGE, vw - w - EDGE))}px`,
      top: `${Math.min(Math.max(top, EDGE), Math.max(EDGE, vh - ht - EDGE))}px`,
    });
  }

  function show(n) {
    index = n;
    const step = steps[index];
    if (step.drawer && isDrawer()) openNav(); else closeNav();
    targetOf(step)?.scrollIntoView({ block: 'nearest' });
    count.textContent = `Step ${index + 1} of ${steps.length}`;
    title.textContent = step.title;
    text.textContent = step.text;
    back.hidden = index === 0;
    nextLabel.textContent = index === steps.length - 1 ? 'Done' : 'Next';
    place();
    // With reduced motion the drawer arrives without a transition to wait for.
    requestAnimationFrame(place);
    next.focus();
  }

  // The highlight follows the window being resized or scrolled, and the
  // drawer once it has finished sliding in.
  const sidenav = document.getElementById('sidenav');
  window.addEventListener('resize', place);
  window.addEventListener('scroll', place, true);
  sidenav?.addEventListener('transitionend', place);
  card.addEventListener('close', () => {
    window.removeEventListener('resize', place);
    window.removeEventListener('scroll', place, true);
    sidenav?.removeEventListener('transitionend', place);
    closeNav();
    ring.remove();
    card.remove();
    onEnd();
  });

  document.body.append(ring, card);
  card.showModal();
  show(0);
}
