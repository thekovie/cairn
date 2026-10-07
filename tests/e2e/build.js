// Build the cairn program once before the tests (debug builds read assets/
// from disk, so JavaScript changes are picked up without rebuilding).
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export default function build() {
  execFileSync('cargo', ['build', '--quiet'], {
    cwd: fileURLToPath(new URL('../..', import.meta.url)),
    stdio: 'inherit',
  });
}
