// Writes assets/vendor/milkdown.LICENSES.txt: the license text of every
// package that went into the bundle (read from esbuild's metafile), because
// minifying removes most of the notices from the bundle itself.
import { readFileSync, writeFileSync, readdirSync, existsSync } from 'node:fs';
import { join, sep } from 'node:path';

const meta = JSON.parse(readFileSync('meta.json', 'utf8'));
const dirs = new Set();
for (const input of Object.keys(meta.inputs)) {
  const parts = input.split(/[\\/]/);
  const i = parts.lastIndexOf('node_modules');
  if (i === -1) continue;
  const scoped = parts[i + 1].startsWith('@');
  dirs.add(parts.slice(0, i + (scoped ? 3 : 2)).join(sep));
}

const sections = [];
for (const dir of [...dirs].sort()) {
  const pkg = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8'));
  const file = readdirSync(dir).find((f) => /^(licen[cs]e|copying)(\.|$)/i.test(f));
  const text = file && existsSync(join(dir, file))
    ? readFileSync(join(dir, file), 'utf8').trim()
    : `License: ${pkg.license || 'unknown'} (no license file in the package)`;
  sections.push(`${pkg.name} ${pkg.version}\n${'-'.repeat(60)}\n${text}`);
}

writeFileSync('../../assets/vendor/milkdown.LICENSES.txt',
  `Third-party software in assets/vendor/milkdown.js (Cairn's visual editor)\n\n${sections.join('\n\n\n')}\n`);
console.log(`licenses for ${sections.length} packages written`);
