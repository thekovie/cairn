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
  wrapInBlockquoteCommand,
  insertHrCommand,
  insertImageCommand,
  toggleLinkCommand,
  linkSchema,
} from '@milkdown/kit/preset/commonmark';
export {
  gfm, insertTableCommand, toggleStrikethroughCommand,
  addRowBeforeCommand, addRowAfterCommand, addColBeforeCommand, addColAfterCommand,
  setAlignCommand,
} from '@milkdown/kit/preset/gfm';
export {
  deleteRow, deleteColumn, deleteTable, isInTable, selectedRect, moveTableRow, moveTableColumn,
} from '@milkdown/kit/prose/tables';
export { history, undoCommand, redoCommand } from '@milkdown/kit/plugin/history';
export { listener, listenerCtx } from '@milkdown/kit/plugin/listener';
export { clipboard } from '@milkdown/kit/plugin/clipboard';
export { callCommand, replaceAll, getMarkdown, insert, $prose } from '@milkdown/kit/utils';
export { TextSelection, Plugin } from '@milkdown/kit/prose/state';
export { Decoration, DecorationSet } from '@milkdown/kit/prose/view';
