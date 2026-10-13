import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import { createServer, get } from 'node:http';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { EventEmitter } from 'node:events';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { ensureInstalled, approvedUrl, verifyManifest, verifyBinary, PLATFORMS, VERSION } from '../lib/native.mjs';
import { launch } from '../lib/launch.mjs';

function manifest(bytes) {
  return { version: VERSION, assets: Object.fromEntries(Object.entries(PLATFORMS).map(([key, asset]) => [key, { asset, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') }])) };
}
async function fixture(handler, body) {
  const directory = await fs.mkdtemp(path.join(tmpdir(), 'pdf-native-npm-'));
  const server = createServer(handler);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const request = (_url, options, callback) => get(`http://127.0.0.1:${server.address().port}/asset`, options, callback);
  try { await body({ directory, request }); }
  finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); await fs.rm(directory, { recursive: true, force: true }); }
}
const bytes = Buffer.from('local deterministic native download test data');
const platform = { platform: 'darwin', arch: 'arm64' };

test('bounded local download publishes exact bytes and valid cache runs offline', async () => {
  let requests = 0;
  await fixture((_req, response) => { requests++; response.end(bytes); }, async ({ directory, request }) => {
    const native = path.join(directory, 'native');
    const filename = await ensureInstalled(manifest(bytes), native, { ...platform, request });
    assert.deepEqual(await fs.readFile(filename), bytes);
    await ensureInstalled(manifest(bytes), native, { ...platform, request: () => { throw new Error('offline'); } });
    assert.equal(requests, 1);
    assert.deepEqual(await fs.readdir(native), ['pdf-editor-offline']);
  });
});

test('checksum mismatch, truncated and oversized bodies leave no published binary', async () => {
  for (const body of [Buffer.from('bad'), Buffer.concat([bytes, bytes]), Buffer.alloc(bytes.length, 42)]) {
    await fixture((_req, response) => { response.writeHead(200); response.write(body); response.end(); }, async ({ directory, request }) => {
      const native = path.join(directory, 'native');
      await assert.rejects(ensureInstalled(manifest(bytes), native, { ...platform, request }));
      assert.deepEqual(await fs.readdir(native), []);
    });
  }
});

test('invalid cached file refuses replacement and preserves contents', async () => {
  await fixture((_req, response) => response.end(bytes), async ({ directory, request }) => {
    const native = path.join(directory, 'native'); await fs.mkdir(native);
    const filename = path.join(native, 'pdf-editor-offline'); await fs.writeFile(filename, 'sentinel');
    await assert.rejects(ensureInstalled(manifest(bytes), native, { ...platform, request }));
    assert.equal(await fs.readFile(filename, 'utf8'), 'sentinel');
  });
});

test('symlink cache directories/files and unsupported platforms are rejected', async () => {
  await fixture((_req, response) => response.end(bytes), async ({ directory, request }) => {
    const outside = path.join(directory, 'outside'); await fs.mkdir(outside);
    const link = path.join(directory, 'link'); await fs.symlink(outside, link, process.platform === 'win32' ? 'junction' : 'dir');
    await assert.rejects(ensureInstalled(manifest(bytes), link, { ...platform, request }));
    const target = path.join(outside, 'target'); await fs.writeFile(target, bytes);
    await fs.symlink(target, path.join(outside, 'pdf-editor-offline'));
    await assert.rejects(ensureInstalled(manifest(bytes), outside, { ...platform, request }));
    await assert.rejects(ensureInstalled(manifest(bytes), path.join(directory, 'unsupported'), { platform: 'win32', arch: 'arm64', request }));
  });
});

test('pinned manifest cannot be incomplete and HTTPS redirect allowlist is exact', async () => {
  assert.throws(() => verifyManifest({ version: VERSION, assets: {} }));
  for (const url of ['http://github.com/x', 'https://github.com.evil.example/x', 'https://evil.github.com/x', 'https://user:pass@github.com/x', 'https://github.com:444/x']) assert.throws(() => approvedUrl(url));
  assert.equal(approvedUrl('https://release-assets.githubusercontent.com/asset?signed=query').protocol, 'https:');
  await fixture((_req, response) => { response.writeHead(302, { location: 'http://127.0.0.1/evil' }); response.end(); }, async ({ directory, request }) => {
    await assert.rejects(ensureInstalled(manifest(bytes), path.join(directory, 'native'), { ...platform, request }), /approved HTTPS/);
  });
});

test('concurrent installation is no-clobber and every result is verified', async () => {
  await fixture((_req, response) => response.end(bytes), async ({ directory, request }) => {
    const native = path.join(directory, 'native');
    const files = await Promise.all(Array.from({ length: 12 }, () => ensureInstalled(manifest(bytes), native, { ...platform, request })));
    assert.equal(new Set(files).size, 1); await verifyBinary(files[0], manifest(bytes).assets['darwin-arm64']);
    assert.deepEqual(await fs.readdir(native), ['pdf-editor-offline']);
  });
});

test('metadata races require a fresh full verification and persistent changes are refused', async () => {
  const directory = await fs.mkdtemp(path.join(tmpdir(), 'pdf-native-stat-'));
  const filename = path.join(directory, 'binary');
  await fs.writeFile(filename, bytes);
  const originalOpen = fs.open;
  let opens = 0;
  let persistent = false;
  // Inject the platform-dependent hard-link ctime change after the real hash read.
  fs.open = async (...args) => {
    const handle = await originalOpen(...args);
    if (args[0] === filename) {
      const attempt = ++opens;
      const stat = handle.stat.bind(handle);
      let calls = 0;
      handle.stat = async (...statArgs) => {
        const result = await stat(...statArgs);
        return ++calls === 3 && (persistent || attempt === 1)
          ? Object.assign(Object.create(Object.getPrototypeOf(result)), result, { ctimeMs: result.ctimeMs + 1 })
          : result;
      };
    }
    return handle;
  };
  try {
    const entry = manifest(bytes).assets['darwin-arm64'];
    await verifyBinary(filename, entry);
    assert.equal(opens, 2);
    opens = 0; persistent = true;
    await assert.rejects(verifyBinary(filename, entry), /changed during verification/);
    assert.equal(opens, 3);
    opens = 0;
    await fs.writeFile(filename, Buffer.alloc(bytes.length, 42));
    await assert.rejects(verifyBinary(filename, entry), /SHA-256/);
    assert.equal(opens, 1);
  } finally {
    fs.open = originalOpen;
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('launcher preserves literal argv, exit status and does not invoke a shell', async () => {
  const directory = await fs.mkdtemp(path.join(tmpdir(), 'pdf-native-launch-'));
  try {
    const script = path.join(directory, 'echo.mjs'); const output = path.join(directory, 'argv.json'); const marker = path.join(directory, 'never-created');
    await fs.writeFile(script, 'import fs from "node:fs";fs.writeFileSync(process.argv[2],JSON.stringify(process.argv.slice(3)));process.exitCode=7;');
    const args = ['spaces and café', `;touch ${marker}`, '$(echo nope)', '*'];
    const result = await launch(process.execPath, [script, output, ...args], { stdio: 'ignore' });
    assert.equal(result.code, 7); assert.deepEqual(JSON.parse(await fs.readFile(output, 'utf8')), args);
    await assert.rejects(fs.stat(marker), { code: 'ENOENT' });
  } finally { await fs.rm(directory, { recursive: true, force: true }); }
});

test('POSIX signals are forwarded and handlers are removed', { skip: process.platform === 'win32' }, async () => {
  const host = new EventEmitter();
  const running = launch(process.execPath, ['-e', 'setInterval(()=>{},1000)'], { stdio: 'ignore', host });
  setTimeout(() => host.emit('SIGTERM'), 100);
  const result = await running; assert.equal(result.signal, 'SIGTERM'); assert.equal(result.code, 143);
  assert.equal(host.listenerCount('SIGTERM'), 0);
});

test('actual native CLI executes through launcher with preserved JSON/exit behavior', async () => {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
  const binary = process.env.PDF_NATIVE_TEST_BINARY ?? path.join(root, 'target/release', process.platform === 'win32' ? 'pdf-editor-offline.exe' : 'pdf-editor-offline');
  await fs.access(binary); // Explicit failure if native proof artifact is absent.
  const direct = spawnSync(binary, ['--version'], { encoding: 'utf8' }); assert.equal(direct.status, 0);
  assert.equal(direct.stdout.trim(), `pdf-editor-offline ${VERSION}`);
  assert.equal((await launch(binary, ['--version'], { stdio: 'ignore' })).code, 0);
  const directory=await fs.mkdtemp(path.join(tmpdir(),'pdf-native-json-'));
  const filename=path.join(directory,'reply.json');
  const output=await fs.open(filename,'wx');
  try {
    assert.equal((await launch(binary, ['--json', 'info', 'missing file;literal $(nothing).pdf'], { stdio: ['ignore',output.fd,output.fd] })).code, 3);
    await output.sync();
    const envelope=JSON.parse(await fs.readFile(filename,'utf8'));
    assert.equal(envelope.ok,false);assert.equal(typeof envelope.error,'object');
  }finally{await output.close();await fs.rm(directory,{recursive:true,force:true});}
});
