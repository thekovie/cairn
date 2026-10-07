# Cairn: notes for Claude

## Graphify

`graphify-out/` holds a knowledge graph of this repository, made with `/graphify`. Use it as an architectural reference, not as source code.

- Read `graphify-out/GRAPH_REPORT.md` first for the high-level structure: the main communities, the most connected pieces (`AppState`, `Root`, `mountEditor()`), and surprising links.
- Use Graphify queries on `graphify-out/graph.json` for dependencies, relationships, paths, and impact analysis, for example `graphify query "<question>"`, `graphify path "A" "B"`, and `graphify explain "<name>"`.
- The graph shows where to look. Verify against the actual source before changing anything: it can be out of date, and some of its links are inferred (marked `INFERRED`), not read from the code.
- Leave `graphify-out/` out of normal code searches (grep, glob, find) so its contents don't show up as duplicate or noisy results.

Only `graphify-out/GRAPH_REPORT.md` and `graphify-out/graph.json` are committed (see `.gitignore`). `graph.html`, the cache, and other generated files stay local. Rebuild the graph now and then (`/graphify . --update`) rather than on every change: `graph.json` is large, so each rebuild makes a big diff.
