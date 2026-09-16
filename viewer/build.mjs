import { build } from 'esbuild';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('.', import.meta.url));
const license = await readFile(`${root}node_modules/three/LICENSE`, 'utf8');
const notice = await readFile(`${root}LICENSE.txt`, 'utf8');
if (notice.includes('*/')) throw new Error('Invalid license comment terminator');
await build({
  absWorkingDir: root,
  entryPoints: ['src/viewer.js'],
  outfile: '../python/sldkit/viewer/_assets/viewer.js',
  bundle: true,
  minify: true,
  format: 'iife',
  target: ['es2020'],
  legalComments: 'inline',
  banner: { js: `/*\n${notice}\n*/` },
});
await writeFile(`${root}../python/sldkit/viewer/_assets/three-LICENSE.txt`, license);
await writeFile(`${root}../LICENSES/three-MIT.txt`, license);
await writeFile(`${root}../python/sldkit/viewer/_assets/LICENSE.txt`, notice);
