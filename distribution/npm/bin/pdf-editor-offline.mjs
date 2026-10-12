#!/usr/bin/env node
import path from 'node:path';
import { ensureInstalled, readManifest, PACKAGE_DIRECTORY } from '../lib/native.mjs';
import { launch } from '../lib/launch.mjs';
try {
  const binary = await ensureInstalled(await readManifest(), path.join(PACKAGE_DIRECTORY, 'native'));
  const result = await launch(binary, process.argv.slice(2));
  if (result.signal) { try { process.kill(process.pid, result.signal); } catch { process.exitCode = result.code; } }
  else process.exitCode = result.code;
} catch (error) { console.error(`pdf-editor-offline: ${error.message}`); process.exitCode = 1; }
