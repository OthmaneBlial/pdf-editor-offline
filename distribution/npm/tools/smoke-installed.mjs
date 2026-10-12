import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { VERSION } from '../lib/native.mjs';
const prefix=process.argv[2];
const launcher=path.join(prefix,process.platform==='win32'?'node_modules':'lib/node_modules','pdf-editor-offline/bin/pdf-editor-offline.mjs');
function run(args) {const result=spawnSync(process.execPath,[launcher,...args],{encoding:'utf8',shell:false,timeout:30_000,maxBuffer:1024*1024});if(result.error || result.status!==0)throw new Error(result.error?.message ?? result.stderr);return result.stdout;}
if(run(['--version']).trim()!==`pdf-editor-offline ${VERSION}`)throw new Error('Installed npm launcher has wrong version.');
if(!run(['--help']).includes('render-page'))throw new Error('Installed command help is incomplete.');
const info=JSON.parse(run(['--json','info','examples/sample_pdfs/demo-basic.pdf']));
if(!info.ok || info.data.page_count!==1 || JSON.stringify(info.data.pages[0].media_box)!=='[0,0,612,792]')throw new Error('Installed command failed synthetic PDF smoke.');
console.log(`Installed launcher ${VERSION} passed real HTTPS download/cache and native execution on ${process.platform}-${process.arch}.`);
