import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import test from "node:test";

const testDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(testDirectory, "..");
const launcher = path.join(repositoryRoot, "bin", "pdf-editor-offline.mjs");

function run(...args) {
  return spawnSync(process.execPath, [launcher, ...args], {
    cwd: repositoryRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      PDF_EDITOR_OFFLINE_NO_OPEN: "1",
    },
  });
}

test("prints the launcher version", () => {
  const result = run("--version");
  assert.equal(result.status, 0);
  assert.equal(result.stdout, "3.0.0\n");
  assert.equal(result.stderr, "");
});

test("documents the desktop and Python CLI paths", () => {
  const result = run("--help");
  assert.equal(result.status, 0);
  assert.match(result.stdout, /pdf-editor-offline desktop/);
  assert.match(result.stdout, /python3 -m pip install pdf-editor-offline/);
});

test("prints the immutable desktop preview URL without opening a browser", () => {
  const result = run("desktop");
  assert.equal(result.status, 0);
  assert.equal(
    result.stdout,
    "https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/desktop-preview-3.0.0\n",
  );
});

test("doctor reports the launcher and local CLI state", () => {
  const result = run("doctor");
  assert.equal(result.status, 0);
  assert.match(result.stdout, /npm launcher: 3\.0\.0/);
  assert.match(result.stdout, /python CLI: (not installed|\d+\.\d+\.\d+)/);
});
