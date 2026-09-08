import { build } from 'esbuild';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('.', import.meta.url));
const license = await readFile(`${root}node_modules/three/LICENSE`, 'utf8');
await build({
  absWorkingDir: root,
  entryPoints: ['src/viewer.js'],
  outfile: '../python/sldkit/viewer/_assets/viewer.js',
  bundle: true,
  minify: true,
  format: 'iife',
  target: ['es2020'],
  legalComments: 'inline',
  banner: { js: `/* Three.js 0.180.0\n${license}*/` },
});
await writeFile(`${root}../python/sldkit/viewer/_assets/three-LICENSE.txt`, license);
await writeFile(`${root}../LICENSES/three-MIT.txt`, license);
