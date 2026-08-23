# PDF Editor Offline frontend

This React application is the task-oriented UI for the local FastAPI engine. It is used in source mode and embedded unchanged in the Tauri desktop shell.

## Local development

From the repository root, use `./start.sh`. It chooses free loopback ports, creates a per-launch API token, starts both processes, and stops only its own child processes.

Manual setup:

```bash
npm ci
VITE_API_BASE_URL=http://127.0.0.1:8000 VITE_API_TOKEN=dev-token npm run dev
```

Run the API with the same `PDF_EDITOR_OFFLINE_API_TOKEN` value.

## Validation

```bash
npm test
npm run lint
npm run build
```

The production build type-checks first. Frontend tests use Vitest and Testing Library under a pinned Node version in CI.

## Structure

- `src/components/` — editor shell, runtime trust console, and shared UI.
- `src/components/tools/` — task implementations and expert tools.
- `src/contexts/` — document, canvas, history, and tool-result state.
- `src/lib/apiClient.ts` — loopback API location and token propagation.
- `src/lib/desktop.ts` — Tauri-native open/save and runtime bootstrap.
- `tests/` — component, contract, lifecycle, and desktop-runtime tests.

Read the repository [architecture](../docs/ARCHITECTURE.md), [capability matrix](../docs/CAPABILITIES.md), and [privacy contract](../docs/PRIVACY.md) before changing network, storage, or document behavior.
