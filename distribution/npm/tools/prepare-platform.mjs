import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { PLATFORMS, VERSION, MAX_BINARY_BYTES } from '../lib/native.mjs';

function run(binary, args, options = {}) {
  const result = spawnSync(binary, args, { encoding: 'utf8', timeout: 60_000, maxBuffer: 64 * 1024 * 1024, shell: false, ...options });
  if (result.error || result.status !== 0) throw new Error(`${binary} failed: ${result.error?.message ?? result.stderr}`);
  return result.stdout;
}
const [key, target, output] = process.argv.slice(2);
if (key !== `${process.platform}-${process.arch}` || !PLATFORMS[key]) throw new Error('Release runner architecture does not match selected platform.');
const filename = path.resolve('target', target, 'release', process.platform === 'win32' ? 'pdf-editor-offline.exe' : 'pdf-editor-offline');
const version = run(filename, ['--version'], { maxBuffer: 1024 }).trim();
if (version !== `pdf-editor-offline ${VERSION}`) throw new Error(`Wrong native release version: ${version}`);
const help = run(filename, ['--help'], { maxBuffer: 1024 * 1024 });
if (!help.includes('info') || !help.includes('render-page')) throw new Error('Native help did not expose expected real commands.');
const input = 'examples/sample_pdfs/demo-basic.pdf';
const before = await fs.readFile(input);
const info = JSON.parse(run(filename, ['--json', 'info', input], { maxBuffer: 1024 * 1024 }));
if (info.ok !== true || info.data.page_count !== 1 || info.data.pages[0].rotation !== 0
  || JSON.stringify(info.data.pages[0].media_box) !== '[0,0,612,792]') throw new Error('Synthetic PDF smoke geometry did not match.');
if (!before.equals(await fs.readFile(input))) throw new Error('Smoke command changed its input.');
let staticProof = null;
if (process.platform === 'linux') {
  const headers = run('readelf', ['-l', filename]);
  const dynamic = run('readelf', ['-d', filename]);
  if (headers.includes('INTERP') || dynamic.includes('(NEEDED)')) throw new Error('Linux release is not a standalone static musl binary.');
  staticProof = { no_interpreter: true, no_needed_libraries: true };
}
const bytes = await fs.readFile(filename);
if (!bytes.length || bytes.length > MAX_BINARY_BYTES) throw new Error('Release binary exceeds installer size budget.');
const hash = data => createHash('sha256').update(data).digest('hex');
const metadata = JSON.parse(run('cargo', ['metadata', '--locked', '--format-version', '1', '--filter-platform', target]));
const packages = new Map(metadata.packages.map(pkg => [pkg.id, pkg]));
const nodes = new Map(metadata.resolve.nodes.map(node => [node.id, node]));
const root = metadata.packages.find(pkg => pkg.name === 'pdf-editor-offline-cli');
const pending = [root.id]; const selected = new Set();
while (pending.length) {
  const id = pending.pop(); if (selected.has(id)) continue; selected.add(id);
  for (const dependency of nodes.get(id).deps) if (dependency.dep_kinds.some(kind => kind.kind !== 'dev')) pending.push(dependency.pkg);
}
let notices = `Native CLI ${VERSION}\nTarget ${target}\nSource ${run('git', ['rev-parse', 'HEAD']).trim()}\n\n`;
const inventory = [];
let noticeBytes=0;
async function licenseText(file, allowedDirectory) {
  const parent=await fs.realpath(allowedDirectory);const actual=await fs.realpath(file);const stat=await fs.lstat(file);
  if(!stat.isFile() || stat.isSymbolicLink() || !actual.startsWith(parent+path.sep) || stat.size>1024*1024)throw new Error(`Unsafe/oversized license source: ${path.basename(file)}`);
  noticeBytes+=stat.size;if(noticeBytes>16*1024*1024)throw new Error('License inventory exceeds16MiB.');
  return fs.readFile(file,'utf8');
}
for (const pkg of [...selected].map(id => packages.get(id)).sort((a, b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`))) {
  const directory = path.dirname(pkg.manifest_path);
  let files = (await fs.readdir(directory, { withFileTypes: true })).filter(entry => entry.isFile() && /^(licen[cs]e(?:[-_.]|$)|copying(?:[-_.]|$)|notice(?:[-_.]|$))/i.test(entry.name)).map(entry => path.join(directory, entry.name));
  if (pkg.license_file) files.push(path.resolve(directory, pkg.license_file));
  if (!pkg.source) files.push(path.resolve('LICENSE'));
  files = [...new Set(files)];
  let sharedDirectory=null;
  // alloc-stdlib0.2.4 is published from the alloc-stdlib subdirectory without
  // its repository-root license. The locked companion crate ships that exact
  // shared BSD license: upstream ae42d22078b98549e987d2f03d12df7b984fde47/LICENSE.
  if(!files.length && pkg.name==='alloc-stdlib' && pkg.version==='0.2.4' && pkg.license==='BSD-3-Clause') {
    const companion=metadata.packages.find(other=>other.name==='alloc-no-stdlib' && other.version==='2.0.4' && other.repository===pkg.repository && other.license===pkg.license);
    if(companion){sharedDirectory=path.dirname(companion.manifest_path);files.push(path.join(sharedDirectory,'LICENSE'));}
  }
  if (!files.length) throw new Error(`Missing license text for linked/build dependency ${pkg.name}@${pkg.version}`);
  notices += `\n========== ${pkg.name} ${pkg.version} ==========\nSPDX: ${pkg.license ?? 'license-file'}\nRepository: ${pkg.repository ?? 'not declared'}\nAuthors: ${(pkg.authors ?? []).join(', ')}\n`;
  if(files.length>32)throw new Error(`Too many license files for ${pkg.name}`);
  if(sharedDirectory)notices+='Shared upstream license: https://github.com/dropbox/rust-alloc-no-stdlib/blob/ae42d22078b98549e987d2f03d12df7b984fde47/LICENSE\n';
  for (const file of files) notices += `\n--- ${path.basename(file)} ---\n${await licenseText(file,!pkg.source && file===path.resolve('LICENSE') ? process.cwd() : sharedDirectory ?? directory)}\n`;
  inventory.push({ name: pkg.name, version: pkg.version, license: pkg.license });
}
for (const file of ['LICENSE_FOXIT', 'CGATS_LICENSE.txt', 'README.md']) notices += `\n========== Embedded font/ICC asset ${file} ==========\n${await licenseText(path.join('vendor/hayro-interpret/assets', file),'vendor/hayro-interpret/assets')}\n`;
const dirty=run('git',['status','--porcelain','--untracked-files=all']).trim();
await fs.mkdir(output, { recursive: true });
await fs.writeFile(path.join(output, PLATFORMS[key]), bytes, { flag: 'wx', mode: 0o755 });
await fs.writeFile(path.join(output, `notices-${key}.txt`), notices, { flag: 'wx' });
const record = { key, target, version: VERSION, source_commit: run('git', ['rev-parse', 'HEAD']).trim(), source_dirty:Boolean(dirty), asset: PLATFORMS[key], bytes: bytes.length, sha256: hash(bytes), notices_sha256:hash(Buffer.from(notices)), compiler: run('rustc', ['--version']).trim(), node: process.version, runner_platform: process.platform, runner_architecture: process.arch, smoke: { version, help: true, info: info.data, fixture_sha256: hash(before), source_unchanged: true }, static_proof: staticProof, dependency_inventory: inventory };
await fs.writeFile(path.join(output, `record-${key}.json`), JSON.stringify(record, null, 2) + '\n', { flag: 'wx' });
console.log(`Verified ${key}: ${record.bytes} bytes, SHA256 ${record.sha256}`);
