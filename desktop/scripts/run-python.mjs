import { spawnSync } from 'node:child_process';

const scriptArgs = process.argv.slice(2);
if (scriptArgs.length === 0) {
  console.error('Usage: node scripts/run-python.mjs <script.py> [arguments]');
  process.exit(2);
}

const configured = process.env.PYTHON?.trim();
const candidates = [
  ...(configured ? [{ command: configured, prefix: [] }] : []),
  { command: 'python3', prefix: [] },
  { command: 'python', prefix: [] },
  { command: 'py', prefix: ['-3'] },
];

let selected;
for (const candidate of candidates) {
  const probe = spawnSync(candidate.command, [...candidate.prefix, '--version'], {
    encoding: 'utf8',
    shell: false,
  });
  if (probe.status === 0) {
    selected = candidate;
    break;
  }
}

if (!selected) {
  console.error('Python 3.10 or newer is required to build the desktop sidecar.');
  process.exit(127);
}

const result = spawnSync(
  selected.command,
  [...selected.prefix, ...scriptArgs],
  { stdio: 'inherit', shell: false },
);

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
