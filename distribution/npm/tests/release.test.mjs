import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import { assemble } from '../tools/assemble-release.mjs';
import { PLATFORMS, VERSION } from '../lib/native.mjs';

const commit='a'.repeat(40);
const targets={'darwin-arm64':'aarch64-apple-darwin','darwin-x64':'x86_64-apple-darwin','win32-x64':'x86_64-pc-windows-msvc','linux-x64':'x86_64-unknown-linux-musl','linux-arm64':'aarch64-unknown-linux-musl'};
const hash=data=>createHash('sha256').update(data).digest('hex');
async function fixture(callback) {
  const directory=await fs.mkdtemp(path.join(tmpdir(),'pdf-release-unit-'));
  try {
    for(const [key,asset] of Object.entries(PLATFORMS)) {
      const bytes=Buffer.from(`synthetic assembler fixture ${key}`);
      const notices='PDFium Authors\nCC0 1.0 Universal\nmoxcms\nSynthetic unit-test notice only.\n';
      const [platform,architecture]=key.split('-');
      const record={key,asset,target:targets[key],version:VERSION,source_commit:commit,source_dirty:false,bytes:bytes.length,sha256:hash(bytes),notices_sha256:hash(notices),runner_platform:platform,runner_architecture:architecture,smoke:{version:`pdf-editor-offline ${VERSION}`,help:true,source_unchanged:true,info:{page_count:1,pages:[{media_box:[0,0,612,792],rotation:0}]}},static_proof:key.startsWith('linux-')?{no_interpreter:true,no_needed_libraries:true}:null};
      await fs.writeFile(path.join(directory,asset),bytes);
      await fs.writeFile(path.join(directory,`record-${key}.json`),JSON.stringify(record));
      await fs.writeFile(path.join(directory,`notices-${key}.txt`),notices);
    }
    await callback(directory);
  }finally{await fs.rm(directory,{recursive:true,force:true});}
}
async function change(directory,callback,key='linux-arm64') {const file=path.join(directory,`record-${key}.json`);const record=JSON.parse(await fs.readFile(file,'utf8'));callback(record);await fs.writeFile(file,JSON.stringify(record));}

test('assembler verifies exactly five synthetic records and emits pinned manifest',async()=>fixture(async directory=>{
  const manifest=await assemble(directory,commit);assert.equal(Object.keys(manifest.assets).length,5);
  assert.equal(JSON.parse(await fs.readFile(path.join(directory,'release-evidence.json'),'utf8')).source_commit,commit);
  assert.match(await fs.readFile(path.join(directory,'THIRD_PARTY_NOTICES.txt'),'utf8'),/linux-arm64/);
}));

test('assembler refuses hash, size, source, dirty, architecture, geometry and static-proof mismatch before any output',async()=>{
  for(const mutation of [record=>record.sha256='b'.repeat(64),record=>record.bytes++,record=>record.source_commit='b'.repeat(40),record=>record.source_dirty=true,record=>record.runner_architecture='x64',record=>record.smoke.info.pages[0].media_box[2]=1,record=>record.static_proof.no_interpreter=false]) {
    await fixture(async directory=>{await change(directory,mutation);await assert.rejects(assemble(directory,commit),/mismatch/);await assert.rejects(fs.stat(path.join(directory,'release-manifest.json')),{code:'ENOENT'});});
  }
});

test('assembler rejects missing fifth asset and tampered/incomplete license notices',async()=>{
  await fixture(async directory=>{await fs.unlink(path.join(directory,PLATFORMS['win32-x64']));await assert.rejects(assemble(directory,commit));});
  await fixture(async directory=>{await fs.writeFile(path.join(directory,'notices-linux-arm64.txt'),'incomplete');await change(directory,record=>record.notices_sha256=hash('incomplete'));await assert.rejects(assemble(directory,commit),/notices/);});
  await fixture(async directory=>{await fs.appendFile(path.join(directory,'notices-linux-arm64.txt'),'tampered');await assert.rejects(assemble(directory,commit),/notices/);});
});
