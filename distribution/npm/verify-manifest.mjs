import { readManifest } from './lib/native.mjs';
import fs from 'node:fs/promises';
import path from 'node:path';
import { PACKAGE_DIRECTORY, PLATFORMS } from './lib/native.mjs';
await readManifest();
const notices=await fs.readFile(path.join(PACKAGE_DIRECTORY,'THIRD_PARTY_NOTICES.txt'),'utf8');
for(const key of Object.keys(PLATFORMS))if(!notices.includes(`########## ${key} ##########`))throw new Error(`Native license notices missing platform ${key}`);
if(!notices.includes('PDFium Authors') || !notices.includes('CC0 1.0 Universal') || !notices.includes('moxcms'))throw new Error('Native dependency/font/ICC license notices are incomplete.');
