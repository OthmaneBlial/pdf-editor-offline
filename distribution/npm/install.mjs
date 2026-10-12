import path from 'node:path';
import { ensureInstalled, readManifest, PACKAGE_DIRECTORY } from './lib/native.mjs';
try { await ensureInstalled(await readManifest(), path.join(PACKAGE_DIRECTORY, 'native')); }
catch (error) { console.error(`pdf-editor-offline install: ${error.message}`); process.exitCode = 1; }
