import assert from 'node:assert/strict';
import { before, after, test } from 'node:test';
import { execFileSync } from 'node:child_process';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { chromium } from 'playwright';

const root = fileURLToPath(new URL('../../', import.meta.url));
let directory, browser, context;
before(async () => {
  directory = await mkdtemp(join(tmpdir(), 'sldkit-viewer-browser-'));
  const python = process.env.SLDKIT_TEST_PYTHON || join(root, '.venv/bin/python');
  execFileSync(python, ['tests/viewer_fixtures.py', directory], { cwd: root });
  browser = await chromium.launch({
    executablePath: process.env.SLDKIT_TEST_CHROMIUM || '/usr/bin/google-chrome',
    headless: true,
    args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
  });
  context = await browser.newContext({ offline: true, viewport: { width: 1280, height: 900 } });
  console.log(`Viewer browser artifacts: ${directory}`);
});
after(async () => { await browser?.close(); });

async function open(name, setup) {
  const page = await context.newPage();
  const errors = [], remote = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('request', (request) => { if (/^https?:/.test(request.url())) remote.push(request.url()); });
  if (setup) await setup(page);
  await page.goto(pathToFileURL(join(directory, `${name}.html`)).href);
  await page.waitForFunction(() => document.documentElement.dataset.viewer === 'ready');
  return { page, errors, remote };
}

test('offline meshes render, orbit, zoom, fit and toggle without losing counts', async () => {
  const { page, errors, remote } = await open('meshes');
  assert.equal(await page.evaluate(() => document.documentElement.dataset.renderer), 'ready');
  assert.equal(await page.locator('#triangle-count').textContent(), '24');
  assert.equal(await page.locator('#visible-count').textContent(), '2 / 2 meshes visible');
  const canvas = page.locator('canvas');
  const shaded = await canvas.screenshot();
  await page.selectOption('#style', 'wireframe');
  assert.notDeepEqual(await canvas.screenshot(), shaded);
  await page.selectOption('#style', 'shaded');
  await page.selectOption('#view', 'front');
  const front = await canvas.screenshot();
  assert.notDeepEqual(front, shaded);
  const box = await canvas.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 80, box.y + box.height / 2 + 30, { steps: 6 });
  await page.mouse.up();
  assert.notDeepEqual(await canvas.screenshot(), front);
  await page.mouse.wheel(0, -200);
  await page.click('#fit');
  await page.locator('.mesh-row input').first().uncheck();
  assert.equal(await page.locator('#visible-count').textContent(), '1 / 2 meshes visible');
  await page.selectOption('#configuration', 'First');
  assert.equal(await page.locator('#visible-count').textContent(), '0 / 2 meshes visible');
  await page.click('#show-all');
  await page.selectOption('#configuration', 'First');
  assert.equal(await page.locator('#visible-count').textContent(), '1 / 2 meshes visible');
  await page.selectOption('#configuration', 'Empty');
  assert.equal(await page.locator('#empty-title').textContent(), 'No visible meshes');
  assert.equal(await page.locator('#configuration option[value="Unknown"]').isDisabled(), true);
  await page.click('#show-all');
  await page.selectOption('#view', 'iso');
  await page.screenshot({ path: join(directory, 'meshes.png') });
  assert.deepEqual(errors, []);
  assert.deepEqual(remote, []);
  await page.close();
});

test('unresolved ownership disables configuration filtering', async () => {
  const { page, errors } = await open('unresolved');
  assert.equal(await page.locator('#configuration').isDisabled(), true);
  assert.equal(await page.locator('#visible-count').textContent(), '2 / 2 meshes visible');
  assert.deepEqual(errors, []);
  await page.close();
});

test('saved PNG is displayed offline when geometry is empty', async () => {
  const { page, errors, remote } = await open('preview');
  assert.equal(await page.locator('#panel-preview').isVisible(), true);
  await page.waitForFunction(() => document.getElementById('preview-image').naturalWidth === 1);
  await page.click('#tab-3d');
  assert.equal(await page.locator('#empty-title').textContent(), 'No mesh available');
  await page.click('#empty-preview');
  assert.equal(await page.locator('#panel-preview').isVisible(), true);
  assert.deepEqual(errors, []);
  assert.deepEqual(remote, []);
  await page.close();
});

test('RLE DIB previews decode after BMP wrapping', async () => {
  const { page, errors, remote } = await open('rle');
  await page.waitForFunction(() => document.getElementById('preview-image').naturalWidth === 2);
  assert.equal(await page.locator('#panel-preview').isVisible(), true);
  assert.deepEqual(errors, []);
  assert.deepEqual(remote, []);
  await page.close();
});

test('drawing sheets and empty unsupported results remain usable', async () => {
  const { page, errors } = await open('drawing');
  assert.match(await page.locator('#document-info').textContent(), /Sheet One/);
  assert.match(await page.locator('#status').textContent(), /not requested/);
  assert.deepEqual(errors, []);
  await page.close();
  const empty = await open('empty');
  assert.equal(await empty.page.locator('#empty').isVisible(), true);
  assert.match(await empty.page.locator('#status').textContent(), /unsupported/);
  assert.deepEqual(empty.errors, []);
  await empty.page.close();
});

test('source strings cannot inject scripts, markup or network requests', async () => {
  const { page, errors, remote } = await open('escaped');
  assert.equal(await page.evaluate(() => globalThis.injected), undefined);
  assert.match(await page.locator('#file-title').textContent(), /@@SCRIPT@@/);
  assert.equal(await page.locator('img[src^="https:"]').count(), 0);
  assert.deepEqual(errors, []);
  assert.deepEqual(remote, []);
  await page.close();
});

test('missing WebGL keeps analysis and preview navigation accessible', async () => {
  const { page, errors } = await open('meshes', async (page) => {
    await page.addInitScript(() => {
      const original = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function (kind, ...args) {
        return kind.includes('webgl') ? null : original.call(this, kind, ...args);
      };
    });
  });
  assert.equal(await page.locator('#empty-title').textContent(), '3D display unavailable');
  await page.click('#tab-preview');
  assert.equal(await page.locator('#preview-empty').isVisible(), true);
  assert.deepEqual(errors, []);
  await page.close();
});

test('small screens keep the viewer within the viewport', async () => {
  const { page } = await open('meshes');
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  assert.equal(await page.locator('canvas').isVisible(), true);
  await page.screenshot({ path: join(directory, 'mobile.png'), fullPage: true });
  await page.close();
});
