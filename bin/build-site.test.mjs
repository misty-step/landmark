import assert from 'node:assert/strict';
import { execFile, execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

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

test('literal CI promotion accepts forward progress and reserves regression for explicit recovery', async t => {
  const workflow = readFileSync(`${root}.github/workflows/pages.yml`, 'utf8');
  assert.match(workflow, /uses: actions\/checkout@v4\n        with:\n          fetch-depth: 0/);
  const block = workflow.split('      - name: Refuse backward source promotion\n')[1].split('      - uses:')[0];
  assert.ok(block.includes('ARTIFACT_RUN: ${{ inputs.artifact_run }}'));
  assert.ok(block.includes('ROLLBACK_VERSION: ${{ inputs.rollback_version }}'));
  const command = block.split('        run: |\n')[1].replace(/^          /gm, '');
  const fixture = mkdtempSync(`${tmpdir()}/landmark-promotion-`);
  t.after(() => rmSync(fixture, { recursive: true, force: true }));
  mkdirSync(`${fixture}/.landmark`);
  const git = (...args) => execFileSync('git', args, { cwd: fixture, encoding: 'utf8' }).trim();
  git('init', '-q', '-b', 'master');
  git('config', 'user.name', 'promotion-test');
  git('config', 'user.email', 'promotion-test@localhost');
  const commit = title => {
    git('commit', '-q', '--allow-empty', '-m', title);
    return git('rev-parse', 'HEAD');
  };
  const a = commit('live A');
  const b = commit('tested candidate B');
  const c = commit('newer source C');
  git('checkout', '-q', '--orphan', 'divergent');
  const divergent = commit('unrelated history');
  git('checkout', '-q', 'master');
  assert.equal(git('rev-parse', 'HEAD'), c);
  let live = a;
  let status = 200;
  const server = createServer((request, response) => {
    assert.equal(request.url, '/release.json');
    response.writeHead(status, { 'content-type': 'application/json' });
    response.end(JSON.stringify({ revision: live }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const url = `http://127.0.0.1:${server.address().port}`;
  const run = async (candidate, recovery = {}) => {
    try {
      await promisify(execFile)('bash', ['-euo', 'pipefail', '-c', command], {
        cwd: fixture,
        env: { ...process.env, SITE_URL: url, GITHUB_SHA: candidate,
          ARTIFACT_RUN: '', ROLLBACK_VERSION: '', ...recovery },
      });
      return 0;
    } catch (error) {
      return error.code;
    }
  };
  await t.test('A live, B candidate, C source: B still deploys', async () => assert.equal(await run(b), 0));
  live = b;
  await t.test('repeat B and next C deploy', async () => {
    assert.equal(await run(b), 0);
    assert.equal(await run(c), 0);
  });
  await t.test('older A and divergent history refuse', async () => {
    assert.equal(await run(a), 1);
    assert.equal(await run(divergent), 1);
  });
  live = 'unavailable-revision';
  await t.test('unknown live revision refuses', async () => assert.notEqual(await run(c), 0));
  status = 503;
  await t.test('unreadable live identity refuses', async () => assert.equal(await run(c), 22));
  await t.test('explicit artifact restore and native rollback bypass source age and live lookup', async () => {
    assert.equal(await run(a, { ARTIFACT_RUN: 'retained-run' }), 0);
    assert.equal(await run(divergent, { ROLLBACK_VERSION: 'previous-worker-version' }), 0);
  });
});
