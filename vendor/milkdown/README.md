# Visual editor engine (vendored)

`assets/vendor/milkdown.js` is a single pre-built file containing
[Milkdown](https://milkdown.dev) and the libraries it is built on
([ProseMirror](https://prosemirror.net), [remark](https://github.com/remarkjs/remark),
and their dependencies). All 89 bundled packages are MIT-licensed. Their license
texts are in `assets/vendor/milkdown.LICENSES.txt`, which the build writes and
which ships inside `cairn.exe` next to the bundle.

It is built from `entry.js`, which lists only the parts Cairn uses. Both files
are committed, so building Cairn itself needs no Node.js.

## Updating it

Only needed to upgrade Milkdown or to use more of it:

```powershell
cd vendor\milkdown
npm install            # versions are pinned in package.json
npm run build          # writes ..\..\assets\vendor\milkdown.js and the license file
cd ..\..
cargo test --test ui_quality
```

Then check visual editing in the browser (headings, lists, links, tables,
pictures, publishing), since the bundle is only exercised there.
