import https from 'node:https';
import { createHash, randomBytes } from 'node:crypto';
import { constants } from 'node:fs';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import packageMetadata from '../package.json' with { type: 'json' };

export const PACKAGE_DIRECTORY = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
export const VERSION = packageMetadata.version;
export const MAX_BINARY_BYTES = 128 * 1024 * 1024;
export const PLATFORMS = Object.freeze({
  'darwin-arm64': 'pdf-editor-offline-darwin-arm64',
  'darwin-x64': 'pdf-editor-offline-darwin-x64',
  'win32-x64': 'pdf-editor-offline-windows-x64.exe',
  'linux-x64': 'pdf-editor-offline-linux-x64',
  'linux-arm64': 'pdf-editor-offline-linux-arm64',
});
const HOSTS = new Set(['github.com', 'release-assets.githubusercontent.com', 'objects.githubusercontent.com', 'github-releases.githubusercontent.com']);
const DOWNLOAD_MILLISECONDS = 120_000;

export function verifyManifest(manifest) {
  if (manifest?.version !== VERSION || !manifest.assets || Object.keys(manifest.assets).length !== 5) {
    throw new Error('Release checksum manifest is incomplete or has the wrong version; installation refused.');
  }
  for (const [key, asset] of Object.entries(PLATFORMS)) {
    const entry = manifest.assets[key];
    if (entry?.asset !== asset || !/^[a-f0-9]{64}$/.test(entry.sha256 ?? '') || /^0+$/.test(entry.sha256)
      || !Number.isSafeInteger(entry.bytes) || entry.bytes <= 0 || entry.bytes > MAX_BINARY_BYTES) {
      throw new Error(`Invalid pinned release asset: ${key}`);
    }
  }
  return manifest;
}

export async function readManifest() {
  return verifyManifest(JSON.parse(await fs.readFile(path.join(PACKAGE_DIRECTORY, 'release-manifest.json'), 'utf8')));
}

export function approvedUrl(value) {
  const url = new URL(value);
  if (url.protocol !== 'https:' || url.username || url.password || url.port || !HOSTS.has(url.hostname)) {
    throw new Error('Download URL is outside the approved HTTPS GitHub release hosts.');
  }
  return url;
}

async function openRegular(filename, flags = constants.O_RDONLY) {
  const stat = await fs.lstat(filename);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error('Native cache entry is not a regular file.');
  const handle = await fs.open(filename, flags | (constants.O_NOFOLLOW ?? 0));
  const actual = await handle.stat();
  if (!actual.isFile() || actual.dev !== stat.dev || actual.ino !== stat.ino) {
    await handle.close(); throw new Error('Native cache file changed during verification.');
  }
  return handle;
}

export async function verifyBinary(filename, entry) {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    if (await verifyBinaryOnce(filename, entry)) return;
  }
  throw new Error('Native binary changed during verification.');
}

async function verifyBinaryOnce(filename, entry) {
  const handle = await openRegular(filename);
  try {
    const stat = await handle.stat();
    if (stat.size !== entry.bytes) throw new Error('Native binary size does not match the pinned release.');
    const hash = createHash('sha256');
    let bytes = 0;
    for await (const chunk of handle.createReadStream({ autoClose: false })) {
      bytes += chunk.length;
      if (bytes > entry.bytes) throw new Error('Native binary grew during verification.');
      hash.update(chunk);
    }
    if (bytes !== entry.bytes || hash.digest('hex') !== entry.sha256) throw new Error('Native binary SHA-256 does not match the pinned release.');
    const after = await handle.stat();
    // Removing the publisher's temporary hard link can change ctime during a concurrent read.
    // Reopen and rehash the whole file; accept only a stable, pinned verification.
    return after.size === stat.size && after.mtimeMs === stat.mtimeMs && after.ctimeMs === stat.ctimeMs;
  } finally { await handle.close(); }
}

function responseFor(url, signal, request) {
  return new Promise((resolve, reject) => {
    const outgoing = request(url, { signal, rejectUnauthorized: true, headers: { 'Accept-Encoding': 'identity', 'User-Agent': `pdf-editor-offline-npm/${VERSION}` } }, resolve);
    outgoing.on('error', reject);
    outgoing.setTimeout(30_000, () => outgoing.destroy(new Error('Native release download stalled.')));
  });
}

async function download(entry, destination, request) {
  let url = approvedUrl(`https://github.com/OthmaneBlial/pdf-editor-offline/releases/download/v${VERSION}/${entry.asset}`);
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(new Error('Native release download exceeded its deadline.')), DOWNLOAD_MILLISECONDS);
  let handle;
  try {
    handle = await fs.open(destination, 'wx', 0o700);
    let response;
    for (let redirects = 0; redirects <= 5; redirects += 1) {
      response = await responseFor(url, abort.signal, request);
      if ([301, 302, 303, 307, 308].includes(response.statusCode)) {
        const location = response.headers.location;
        response.destroy();
        if (!location || redirects === 5) throw new Error('Native release redirect limit exceeded or destination missing.');
        url = approvedUrl(new URL(location, url));
        continue;
      }
      break;
    }
    if (response.statusCode !== 200) { response.destroy(); throw new Error(`Native release download failed (HTTP ${response.statusCode}).`); }
    if (response.headers['content-encoding'] && response.headers['content-encoding'] !== 'identity') { response.destroy(); throw new Error('Encoded release response refused.'); }
    const length = response.headers['content-length'];
    if (length !== undefined && (!/^\d+$/.test(length) || Number(length) !== entry.bytes)) { response.destroy(); throw new Error('Release Content-Length does not match its pinned size.'); }
    const hash = createHash('sha256');
    let bytes = 0;
    for await (const chunk of response) {
      bytes += chunk.length;
      if (bytes > entry.bytes || bytes > MAX_BINARY_BYTES) { response.destroy(); throw new Error('Native release download exceeds its size limit.'); }
      hash.update(chunk);
      // FileHandle.write may perform a partial write.
      let offset = 0;
      while (offset < chunk.length) { const { bytesWritten } = await handle.write(chunk, offset, chunk.length - offset); if (!bytesWritten) throw new Error('Native cache write made no progress.'); offset += bytesWritten; }
    }
    if (bytes !== entry.bytes || hash.digest('hex') !== entry.sha256) throw new Error('Downloaded release SHA-256 or size does not match the pinned manifest.');
    await handle.chmod(0o755);
    await handle.sync();
  } finally { clearTimeout(timer); await handle?.close(); }
}

// request injection is only an in-process test seam. Production entry points
// supply no override and expose no environment/CLI bypass for URL or checksums.
export async function ensureInstalled(manifest, directory, { platform = process.platform, arch = process.arch, request = https.get } = {}) {
  verifyManifest(manifest);
  const key = `${platform}-${arch}`;
  const entry = manifest.assets[key];
  if (!PLATFORMS[key]) throw new Error(`Unsupported native platform: ${key}`);
  try { await fs.mkdir(directory, { mode: 0o700 }); } catch (error) { if (error.code !== 'EEXIST') throw error; }
  const stat = await fs.lstat(directory);
  if (!stat.isDirectory() || stat.isSymbolicLink()) throw new Error('Native cache directory must be a real directory.');
  const canonical = await fs.realpath(directory);
  const filename = path.join(canonical, platform === 'win32' ? 'pdf-editor-offline.exe' : 'pdf-editor-offline');
  try { await verifyBinary(filename, entry); return filename; } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const temporary = path.join(canonical, `.download-${randomBytes(16).toString('hex')}`);
  try {
    await download(entry, temporary, request);
    await verifyBinary(temporary, entry);
    try { await fs.link(temporary, filename); } catch (error) { if (error.code !== 'EEXIST') throw error; }
    await verifyBinary(filename, entry);
    return filename;
  } finally { await fs.unlink(temporary).catch(error => { if (error.code !== 'ENOENT') throw error; }); }
}
