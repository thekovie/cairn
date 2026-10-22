// The Help button: a few plain lines about what can be done on the screen
// that is open. Nothing here appears until someone asks for it.

import { h, openDialog } from './core.js';

// Keyed by route (see app.js). Wording matches the buttons on each screen.
const HELP = {
  '': {
    name: 'the home screen',
    points: [
      'Type a word in the search box to find every page that has it.',
      'Choose a folder under “Folders” to read what is inside it.',
      '“Recently changed” shows pages other people have updated lately.',
      'Pages you started but have not published are listed under “Your unsaved changes”.',
      'Use “New folder” to start a new group of pages.',
    ],
  },
  folder: {
    name: 'a folder',
    points: [
      'Choose a page to read it.',
      '“New page here” adds a page to this folder, and “New folder inside” adds a folder.',
      '“Download this folder” saves a copy of its pages to your computer.',
      'You can also rename, move or delete the folder.',
    ],
  },
  page: {
    name: 'a page',
    points: [
      '“Edit this page” lets you change it. Nobody else sees your changes until you publish.',
      '“Earlier versions” shows how the page looked before, and lets you bring one back.',
      '“Download” saves a copy of the page, for example as a PDF.',
      'Under “More actions” you can rename, move or delete the page.',
      'If the page is still right, “Mark as reviewed today” says so.',
    ],
  },
  edit: {
    name: 'the editor',
    points: [
      'Write as you would in a letter. The buttons above the text make headings, lists, links and pictures.',
      '“Publish changes” shares your page. Until then only you see it.',
      'Only one person can edit a page at a time. If you step away, Cairn asks whether you are still there, and later unlocks the page for others.',
      'Open “Formatting help” for a list of ways to format text.',
    ],
  },
  new: {
    name: 'the new page screen',
    points: [
      'Choose where the page goes and, if you like, a template to start from.',
      '“Create page and start writing” opens the editor. Nothing is shared until you publish.',
    ],
  },
  search: {
    name: 'search',
    points: [
      'Pages that have your words are listed with a short extract.',
      'Use the filters to narrow the list, and “Clear filters” to start again.',
      'Change the words in the search box to look again.',
    ],
  },
  history: {
    name: 'earlier versions',
    points: [
      'Each time a page is published, the version before it is kept.',
      '“View this version” shows how the page looked.',
      '“Restore this version” brings it back as the current page.',
    ],
  },
  templates: {
    name: 'templates',
    points: [
      'Templates are ready-made starting points for new pages.',
      '“Use for a new page” starts a page from one.',
      '“Make a team copy” lets you adjust a built-in template for your team.',
    ],
  },
  deleted: {
    name: 'recently deleted',
    points: [
      'Deleted pages and folders wait here.',
      '“Restore” puts one back.',
    ],
  },
  health: {
    name: 'documentation health',
    points: [
      'This lists pages that need attention, such as pages due for review or with broken links.',
      '“Check documentation” looks for broken links across all pages.',
      'Use the filters to look at one kind of problem at a time.',
      '“Mark as reviewed today” records that a page was checked.',
    ],
  },
  settings: {
    name: 'settings',
    points: [
      'Choose the name others see while you edit.',
      'Change colors and text size. Changes apply straight away.',
      'Choose when Cairn asks whether you are still editing.',
      'Back up your documentation, and check for new versions of Cairn.',
      'Choose “Show the tour again” to see the welcome tour once more.',
    ],
  },
};

const OTHER = {
  name: 'this screen',
  points: ['The search box at the top finds any page, and “Settings” changes how Cairn looks.'],
};

/** Opens Help for `route`; `onTour` runs if the person asks for the tour. */
export async function openHelp(route, onTour) {
  const { name, points } = HELP[route] || OTHER;
  const choice = await openDialog({
    title: 'What you can do here',
    iconName: 'help',
    body: [
      h('p', null, `You are on ${name}.`),
      h('ul', { class: 'help-points' }, points.map((p) => h('li', null, p))),
    ],
    actions: [
      { label: 'Take the tour', icon: 'eye', value: 'tour', left: true },
      { label: 'Close', kind: 'primary', value: 'close', autofocus: true },
    ],
  });
  if (choice === 'tour') onTour();
}
