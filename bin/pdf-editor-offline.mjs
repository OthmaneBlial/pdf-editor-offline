#!/usr/bin/env node

import { spawnSync } from "node:child_process";

const VERSION = "3.0.0";
const RELEASE_URL =
  "https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/desktop-preview-3.0.0";
const WEBSITE_URL = "https://othmaneblial.github.io/pdf-editor-offline/";

const HELP = `PDF Editor Offline ${VERSION}

Local launcher for the desktop preview and bridge to the Python CLI.

Usage:
  pdf-editor-offline desktop       Open the official desktop download page
  pdf-editor-offline website       Open the project website
  pdf-editor-offline doctor        Check the local Python CLI installation
  pdf-editor-offline cli <args>    Run the separately installed Python CLI
  pdf-editor-offline <args>        Forward other commands to the Python CLI
  pdf-editor-offline --version     Print this npm launcher version
  pdf-editor-offline --help        Show this help

The npm package does not bundle unsigned desktop installers or upload documents.
Install the automation CLI separately with:
  python3 -m pip install pdf-editor-offline
`;

function pythonCandidates() {
  if (process.platform === "win32") {
    return [
      { command: "py", prefix: ["-3"] },
      { command: "python", prefix: [] },
      { command: "python3", prefix: [] },
    ];
  }

  return [
    { command: "python3", prefix: [] },
    { command: "python", prefix: [] },
  ];
}

function findPythonCli() {
  const probe = [
    "-c",
    "import importlib.metadata as m; print(m.version('pdf-editor-offline'))",
  ];

  for (const candidate of pythonCandidates()) {
    const result = spawnSync(candidate.command, [...candidate.prefix, ...probe], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    if (result.status === 0) {
      return {
        ...candidate,
        version: result.stdout.trim(),
      };
    }
  }

  return null;
}

function openUrl(url) {
  process.stdout.write(`${url}\n`);

  if (
    process.env.PDF_EDITOR_OFFLINE_NO_OPEN === "1" ||
    process.env.CI === "true" ||
    !process.stdout.isTTY
  ) {
    return 0;
  }

  let command;
  let args;
  if (process.platform === "darwin") {
    command = "open";
    args = [url];
  } else if (process.platform === "win32") {
    command = "rundll32.exe";
    args = ["url.dll,FileProtocolHandler", url];
  } else {
    command = "xdg-open";
    args = [url];
  }

  const result = spawnSync(command, args, { stdio: "ignore" });
  if (result.error || result.status !== 0) {
    process.stderr.write("Could not open a browser; use the URL printed above.\n");
  }
  return 0;
}

function doctor() {
  const pythonCli = findPythonCli();
  process.stdout.write(`npm launcher: ${VERSION}\n`);
  process.stdout.write(`node: ${process.version}\n`);
  process.stdout.write(`platform: ${process.platform}/${process.arch}\n`);
  if (pythonCli) {
    process.stdout.write(`python CLI: ${pythonCli.version}\n`);
    return 0;
  }

  process.stdout.write("python CLI: not installed\n");
  process.stdout.write("install: python3 -m pip install pdf-editor-offline\n");
  return 0;
}

function runPythonCli(args) {
  const pythonCli = findPythonCli();
  if (!pythonCli) {
    process.stderr.write(
      "The Python CLI is not installed. Run `python3 -m pip install pdf-editor-offline`,\n" +
        "or run `pdf-editor-offline desktop` to get the visual desktop app.\n",
    );
    return 1;
  }

  const result = spawnSync(
    pythonCli.command,
    [...pythonCli.prefix, "-m", "pdf_editor_offline.cli.main", ...args],
    { stdio: "inherit" },
  );
  if (result.error) {
    process.stderr.write(`${result.error.message}\n`);
    return 1;
  }
  return result.status ?? 1;
}

function main(args) {
  const [command, ...rest] = args;
  if (!command || command === "help" || command === "--help" || command === "-h") {
    process.stdout.write(HELP);
    return 0;
  }
  if (command === "--version" || command === "-V" || command === "version") {
    process.stdout.write(`${VERSION}\n`);
    return 0;
  }
  if (command === "desktop" || command === "download") {
    return openUrl(RELEASE_URL);
  }
  if (command === "website") {
    return openUrl(WEBSITE_URL);
  }
  if (command === "doctor") {
    return doctor();
  }
  if (command === "cli") {
    return runPythonCli(rest);
  }
  return runPythonCli(args);
}

process.exitCode = main(process.argv.slice(2));
