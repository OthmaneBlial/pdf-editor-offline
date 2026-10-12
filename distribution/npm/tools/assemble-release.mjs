import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { PLATFORMS, VERSION, verifyManifest, MAX_BINARY_BYTES } from '../lib/native.mjs';
const TARGETS={'darwin-arm64':'aarch64-apple-darwin','darwin-x64':'x86_64-apple-darwin','win32-x64':'x86_64-pc-windows-msvc','linux-x64':'x86_64-unknown-linux-musl','linux-arm64':'aarch64-unknown-linux-musl'};
async function readBounded(file,maximum) {const stat=await fs.lstat(file);if(!stat.isFile() || stat.isSymbolicLink() || stat.size>maximum)throw new Error('Release evidence is not a bounded regular file.');const data=await fs.readFile(file);if(data.length>maximum)throw new Error('Release evidence grew past its limit.');return data;}
export async function assemble(directory,sourceCommit) {
if(!/^[a-f0-9]{40}$/.test(sourceCommit))throw new Error('Invalid source commit.');
const assets = {}; const records = []; let notices = '';
for (const [key, asset] of Object.entries(PLATFORMS)) {
  const record = JSON.parse(await readBounded(path.join(directory, `record-${key}.json`),1024*1024));
  const binary = await readBounded(path.join(directory, asset),MAX_BINARY_BYTES);
  const sha256 = createHash('sha256').update(binary).digest('hex');
  const [platform,architecture]=key.split('-');
  if (record.version !== VERSION || record.source_commit !== sourceCommit || record.source_dirty!==false || record.key !== key || record.asset !== asset || record.bytes !== binary.length || record.sha256 !== sha256
    || record.target!==TARGETS[key] || record.runner_platform!==platform || record.runner_architecture!==architecture
    || !record.smoke?.source_unchanged || !record.smoke?.help || record.smoke.version !== `pdf-editor-offline ${VERSION}` || record.smoke.info?.page_count !== 1
    || JSON.stringify(record.smoke.info.pages?.[0]?.media_box)!=='[0,0,612,792]' || record.smoke.info.pages[0].rotation!==0
    || (key.startsWith('linux-') && (!record.static_proof?.no_interpreter || !record.static_proof?.no_needed_libraries))) throw new Error(`Release provenance or smoke evidence mismatch: ${key}`);
  assets[key] = { asset, bytes: binary.length, sha256 }; records.push(record);
  const data=await readBounded(path.join(directory, `notices-${key}.txt`),32*1024*1024);const text=data.toString('utf8');
  if(createHash('sha256').update(data).digest('hex')!==record.notices_sha256 || !text.includes('PDFium Authors') || !text.includes('CC0 1.0 Universal') || !text.includes('moxcms')) throw new Error(`Incomplete or changed embedded font/ICC notices: ${key}`);
  notices += `\n########## ${key} ##########\n${text}\n`;
}
const manifest = verifyManifest({ version: VERSION, assets });
await fs.writeFile(path.join(directory, 'release-manifest.json'), JSON.stringify(manifest, null, 2) + '\n', { flag: 'wx' });
await fs.writeFile(path.join(directory, 'THIRD_PARTY_NOTICES.txt'), notices, { flag: 'wx' });
await fs.writeFile(path.join(directory, 'release-evidence.json'), JSON.stringify({ version: VERSION, source_commit: sourceCommit, platforms: records }, null, 2) + '\n', { flag: 'wx' });
return manifest;
}
if(process.argv[1] && import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href) {
const commit=spawnSync('git',['rev-parse','HEAD'],{encoding:'utf8',shell:false,timeout:30_000,maxBuffer:1024});
if(commit.status!==0)throw new Error('Cannot identify checked-out source commit.');
await assemble(process.argv[2],commit.stdout.trim());
console.log('All five actual-runner artifacts, pinned checksums and license notices verified. Draft creation may proceed.');
}
