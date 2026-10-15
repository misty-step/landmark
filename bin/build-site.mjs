import { execFileSync } from 'node:child_process';
import { cpSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = `${root}.landmark/site`;
const version = JSON.parse(readFileSync(`${root}package.json`, 'utf8')).version;
const revision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
const notes = readFileSync(`${root}CHANGELOG.md`, 'utf8');
if (!notes.startsWith(`# [${version}](`)) {
  throw new Error('CHANGELOG.md must start with the source package version');
}

const template = readFileSync(`${root}site/changelog.html`, 'utf8');
const marker = '<!-- LANDMARK_CHANGELOG -->';
if (template.split(marker).length !== 2) {
  throw new Error('Expected exactly one changelog template marker');
}
const fragment = `${root}.landmark/changelog-fragment.html`;
execFileSync('cargo', [
  'run', '--locked', '-p', 'landmark', '--', 'write-artifacts',
  '--notes-file', 'CHANGELOG.md', '--version', `v${version}`,
  '--output-html-file', fragment,
], { cwd: root, stdio: ['ignore', 'ignore', 'inherit'] });

rmSync(output, { recursive: true, force: true });
cpSync(`${root}site`, output, { recursive: true, dereference: true });
const html = template.replace(marker, () => readFileSync(fragment, 'utf8'));
writeFileSync(`${output}/changelog.html`, html);
writeFileSync(`${output}/CHANGELOG.md`, notes);
writeFileSync(`${output}/release.json`, `${JSON.stringify({ version, revision })}\n`);
rmSync(fragment);
console.log(`Built .landmark/site from ${revision} (v${version})`);
