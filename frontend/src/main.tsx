import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
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
