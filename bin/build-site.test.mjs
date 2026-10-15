import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = `${root}.landmark/site`;
const build = () => execFileSync(process.execPath, [`${root}bin/build-site.mjs`], { stdio: 'inherit' });
const snapshot = () => Object.fromEntries(readdirSync(output, { recursive: true, withFileTypes: true })
  .filter(entry => entry.isFile())
  .map(entry => [`${entry.parentPath}/${entry.name}`.slice(output.length + 1),
    readFileSync(`${entry.parentPath}/${entry.name}`, 'base64')]));

test('site projects canonical notes and source identity, and rebuilds byte-identically', () => {
  build();
  const notes = readFileSync(`${root}CHANGELOG.md`);
  assert.deepEqual(readFileSync(`${output}/CHANGELOG.md`), notes);
  const { version } = JSON.parse(readFileSync(`${root}package.json`, 'utf8'));
  const revision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
  assert.deepEqual(JSON.parse(readFileSync(`${output}/release.json`, 'utf8')), { version, revision });
  const html = readFileSync(`${output}/changelog.html`, 'utf8');
  assert.ok(!html.includes('<!-- LANDMARK_CHANGELOG -->'));
  assert.ok(html.includes(`>${version}</a>`));
  assert.ok(html.includes('href="CHANGELOG.md"'));
  assert.ok(!html.includes('docs/releases/releases.json'));
  assert.deepEqual(readFileSync(`${output}/index.html`), readFileSync(`${root}site/index.html`));
  const before = snapshot();
  writeFileSync(`${output}/stale-file.txt`, 'must not survive a new build');
  build();
  assert.deepEqual(snapshot(), before);
});
