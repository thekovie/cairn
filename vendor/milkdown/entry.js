// The parts of Milkdown (MIT, https://milkdown.dev) that Cairn's visual
// editing mode uses. Bundled into assets/vendor/milkdown.js by `npm run build`.

export {
  Editor, rootCtx, defaultValueCtx, editorViewOptionsCtx, editorViewCtx, remarkPluginsCtx,
  remarkStringifyOptionsCtx,
} from '@milkdown/kit/core';
export {
  commonmark,
  toggleStrongCommand,
  toggleEmphasisCommand,
  toggleInlineCodeCommand,
  wrapInHeadingCommand,
  turnIntoTextCommand,
  wrapInBulletListCommand,
  wrapInOrderedListCommand,
  insertImageCommand,
  toggleLinkCommand,
  linkSchema,
} from '@milkdown/kit/preset/commonmark';
export { gfm, insertTableCommand } from '@milkdown/kit/preset/gfm';
export { history } from '@milkdown/kit/plugin/history';
export { listener, listenerCtx } from '@milkdown/kit/plugin/listener';
export { clipboard } from '@milkdown/kit/plugin/clipboard';
export { callCommand, replaceAll, getMarkdown, insert } from '@milkdown/kit/utils';
export { TextSelection } from '@milkdown/kit/prose/state';
