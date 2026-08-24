import { X } from 'lucide-react';
import { useEditor } from '../contexts/EditorContext';

const ToolToast: React.FC = () => {
  const { toolToast, clearToolToast } = useEditor();

  if (!toolToast) {
    return null;
  }

  return (
    <div className="fixed top-4 right-4 z-[60] max-w-sm w-[calc(100vw-2rem)] sm:w-auto">
      <div
        role={toolToast.type === 'error' ? 'alert' : 'status'}
        aria-live={toolToast.type === 'error' ? 'assertive' : 'polite'}
        aria-atomic="true"
        className={`rounded-xl border shadow-xl px-4 py-3 flex items-start gap-3 ${
          toolToast.type === 'success'
            ? 'bg-emerald-50 border-emerald-200 text-emerald-800'
            : 'bg-red-50 border-red-200 text-red-800'
        }`}
      >
        <div className="text-sm font-medium flex-1">{toolToast.text}</div>
        <button
          onClick={clearToolToast}
          className="touch-target inline-flex h-11 w-11 items-center justify-center rounded-lg hover:bg-black/5 transition-colors"
          aria-label="Dismiss notification"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
};

export default ToolToast;
