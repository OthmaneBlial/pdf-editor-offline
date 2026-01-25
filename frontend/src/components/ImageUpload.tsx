import { useRef } from 'react';
import * as fabric from 'fabric';
import { useEditor } from '../contexts/EditorContext';
import { Image as ImageIcon } from 'lucide-react';

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
        const ImageClass = (fabric as any).FabricImage || (fabric as any).Image;

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
        className="w-full flex items-center justify-center gap-2 p-3.5 bg-gradient-to-r from-green-50 to-emerald-50 border border-green-200 rounded-xl hover:from-green-100 hover:to-emerald-100 hover:border-green-300 hover:shadow-lg hover:shadow-green-200/50 hover:-translate-y-0.5 transition-all duration-300 shadow-sm group focus:outline-none focus:ring-2 focus:ring-green-500 focus:ring-offset-2"
        aria-label="Upload image to add to canvas"
      >
        <div className="p-2 bg-white rounded-lg group-hover:scale-110 transition-transform shadow-sm" aria-hidden="true">
          <ImageIcon className="w-4 h-4 text-green-600" />
        </div>
        <span className="text-sm font-semibold text-green-700 group-hover:text-green-800">Add Image</span>
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
