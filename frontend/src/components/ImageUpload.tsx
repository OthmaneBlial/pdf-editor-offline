import { useRef } from 'react';
import * as fabric from 'fabric';
import { useEditor } from '../contexts/EditorContext';
import { Image as ImageIcon, Sparkles } from 'lucide-react';

const ImageUpload: React.FC = () => {
  const { canvas } = useEditor();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleImageUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (file && canvas) {
      const reader = new FileReader();
      reader.onload = (e) => {
        const imgSrc = e.target?.result as string;

        // Fabric.js v6 uses FabricImage instead of Image
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const ImageClass = (fabric as any).FabricImage || (fabric as any).Image;

        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        ImageClass.fromURL(imgSrc).then((img: any) => {
          img.set({
            left: 100,
            top: 100,
            scaleX: 0.5,
            scaleY: 0.5,
            selectable: true,
            hasControls: true,
          });
          canvas.add(img);
          canvas.setActiveObject(img);
          canvas.requestRenderAll();
        }).catch((err: Error) => {
          console.error('Error loading image:', err);
        });
      };
      reader.readAsDataURL(file);
    }
  };

  const triggerFileInput = () => {
    fileInputRef.current?.click();
  };

  return (
    <div className="w-full">
      <button
        onClick={triggerFileInput}
        className="group relative w-full flex items-center justify-center gap-3 p-4 bg-gradient-to-br from-emerald-500 via-teal-500 to-cyan-500 rounded-xl hover:from-emerald-600 hover:via-teal-600 hover:to-cyan-600 transition-all duration-300 shadow-lg shadow-emerald-500/30 hover:shadow-emerald-500/50 hover:-translate-y-0.5 hover:scale-[1.02] active:scale-[0.98] focus:outline-none focus:ring-2 focus:ring-emerald-400 focus:ring-offset-2 overflow-hidden"
        aria-label="Upload image to add to canvas"
      >
        {/* Animated background pattern */}
        <div className="absolute inset-0 bg-[url('data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjAiIGhlaWdodD0iMjAiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PGNpcmNsZSBjeD0iMSIgY3k9IjEiIHI9IjEiIGZpbGw9InJnYmEoMjU1LDI1NSwyNTUsMC4xKSIvPjwvc3ZnPg==')] opacity-20" />

        {/* Glow effect on hover */}
        <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/20 to-transparent -translate-x-full group-hover:animate-[shimmer_1.5s_infinite]" />

        <div className="relative flex items-center justify-center gap-3">
          {/* Icon container with animation */}
          <div className="p-2.5 bg-white/20 backdrop-blur-sm rounded-xl group-hover:scale-110 group-hover:rotate-3 transition-all duration-300 shadow-lg" aria-hidden="true">
            <ImageIcon className="w-5 h-5 text-white" />
          </div>

          {/* Text with glow effect */}
          <div className="flex flex-col items-start">
            <span className="text-sm font-bold text-white drop-shadow-md">Add Image</span>
            <span className="text-[10px] text-white/80 font-medium">Upload from device</span>
          </div>

          {/* Sparkle icon that appears on hover */}
          <div className="ml-auto opacity-0 group-hover:opacity-100 transition-opacity duration-300">
            <Sparkles className="w-4 h-4 text-white/80" />
          </div>
        </div>
      </button>
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        onChange={handleImageUpload}
        className="hidden"
        aria-label="Image file input"
      />
    </div>
  );
};

export default ImageUpload;
