import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@fontsource-variable/jetbrains-mono'
import '@fontsource-variable/syne'
import '@fontsource/instrument-serif/400.css'
import '@fontsource/instrument-serif/400-italic.css'
import './index.css'
import { initializeDesktopRuntime } from './lib/desktop'

const mount = async () => {
  await initializeDesktopRuntime();
  const [{ default: App }, { ThemeProvider }] = await Promise.all([
    import('./App.tsx'),
    import('./contexts/ThemeContext'),
  ]);

  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <ThemeProvider>
        <App />
      </ThemeProvider>
    </StrictMode>,
  );
};

mount();
