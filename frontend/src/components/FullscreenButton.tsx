import { useState, useEffect, useCallback } from 'react';
import { Maximize, Minimize } from 'lucide-react';

interface FullscreenButtonProps {
    targetRef?: React.RefObject<HTMLElement>;
}

const FullscreenButton: React.FC<FullscreenButtonProps> = ({ targetRef }) => {
    const [isFullscreen, setIsFullscreen] = useState(false);

    const checkFullscreen = useCallback(() => {
        setIsFullscreen(!!document.fullscreenElement);
    }, []);

    useEffect(() => {
        document.addEventListener('fullscreenchange', checkFullscreen);
        return () => document.removeEventListener('fullscreenchange', checkFullscreen);
    }, [checkFullscreen]);

    const toggleFullscreen = async () => {
        try {
            if (!document.fullscreenElement) {
                const element = targetRef?.current || document.documentElement;
                await element.requestFullscreen();
            } else {
                await document.exitFullscreen();
            }
        } catch (error) {
            console.error('Fullscreen error:', error);
        }
    };

    return (
        <button
            onClick={toggleFullscreen}
            className="p-2 text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] hover:text-[var(--color-primary-600)] rounded-lg transition-all"
            title={isFullscreen ? 'Exit Fullscreen (Esc)' : 'Enter Fullscreen (F11)'}
        >
            {isFullscreen ? (
                <Minimize className="w-4 h-4" />
            ) : (
                <Maximize className="w-4 h-4" />
            )}
        </button>
    );
};

export default FullscreenButton;
