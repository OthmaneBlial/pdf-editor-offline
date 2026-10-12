import { spawn } from 'node:child_process';
import { constants } from 'node:os';

export function launch(binary, argv, { stdio = 'inherit', host = process } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, argv, { stdio, shell: false, windowsHide: false });
    const signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
    const handlers = new Map(signals.map(signal => [signal, () => { try { child.kill(signal); } catch { /* child may already have exited */ } }]));
    for (const [signal, handler] of handlers) host.on(signal, handler);
    const cleanup = () => { for (const [signal, handler] of handlers) host.removeListener(signal, handler); };
    child.once('error', error => { cleanup(); reject(error); });
    child.once('exit', (code, signal) => { cleanup(); resolve({ code: code ?? 128 + (constants.signals[signal] ?? 1), signal }); });
  });
}
