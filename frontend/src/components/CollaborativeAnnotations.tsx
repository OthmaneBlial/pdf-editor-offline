import { useState } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { MessageSquare, User, Send, X, Plus, Smile } from 'lucide-react';

interface Comment {
    id: string;
    username: string;
    text: string;
    timestamp: Date;
    pageNumber: number;
    position: { x: number; y: number };
    color: string;
}

const COMMENT_COLORS = [
    '#EF4444', '#F59E0B', '#10B981', '#3B82F6', '#8B5CF6', '#EC4899'
];

const CollaborativeAnnotations: React.FC = () => {
    const { currentPage } = useEditor();
    const [comments, setComments] = useState<Comment[]>([]);
    const [isAddingComment, setIsAddingComment] = useState(false);
    const [newComment, setNewComment] = useState('');
    const [username, setUsername] = useState(() =>
        localStorage.getItem('pdf_editor_username') || ''
    );
    const [showUsernamePrompt, setShowUsernamePrompt] = useState(!username);
    const [selectedColor, setSelectedColor] = useState(COMMENT_COLORS[0]);

    const handleSetUsername = (name: string) => {
        localStorage.setItem('pdf_editor_username', name);
        setUsername(name);
        setShowUsernamePrompt(false);
    };

    const addComment = () => {
        if (!newComment.trim() || !username) return;

        const comment: Comment = {
            id: Date.now().toString(),
            username,
            text: newComment,
            timestamp: new Date(),
            pageNumber: currentPage,
            position: { x: 50, y: 50 + comments.length * 10 },
            color: selectedColor
        };

        setComments(prev => [...prev, comment]);
        setNewComment('');
        setIsAddingComment(false);
    };

    const deleteComment = (id: string) => {
        setComments(prev => prev.filter(c => c.id !== id));
    };

    const pageComments = comments.filter(c => c.pageNumber === currentPage);

    return (
        <div className="overflow-hidden rounded-xl border border-slate-700/50 bg-slate-900/30">
            {/* Beautiful gradient header */}
            <div className="relative">
                <div className="absolute inset-0 bg-gradient-to-br from-amber-500 via-orange-500 to-rose-500" />
                <div className="absolute inset-0 bg-[url('data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjAiIGhlaWdodD0iMjAiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PGNpcmNsZSBjeD0iMSIgY3k9IjEiIHI9IjEiIGZpbGw9InJnYmEoMjU1LDI1NSwyNTUsMC4xKSIvPjwvc3ZnPg==')] opacity-30" />

                <div className="relative p-4">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <div className="p-2 bg-white/20 backdrop-blur-sm rounded-xl">
                                <MessageSquare className="w-4 h-4 text-white" />
                            </div>
                            <div>
                                <span className="text-sm font-bold text-white">Comments</span>
                                <span className="ml-2 px-2 py-0.5 bg-white/20 text-white text-xs rounded-full">
                                    {pageComments.length}
                                </span>
                            </div>
                        </div>
                        <button
                            onClick={() => setIsAddingComment(true)}
                            className="p-2 bg-white text-orange-600 rounded-xl hover:bg-white/90 shadow-lg shadow-black/20 transition-all hover:scale-105 active:scale-95"
                            title="Add Comment"
                        >
                            <Plus className="w-4 h-4" />
                        </button>
                    </div>

                    {/* Username display */}
                    <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-white/90">
                        <User className="w-3.5 h-3.5" />
                        <span>Commenting as</span>
                        <button
                            onClick={() => setShowUsernamePrompt(true)}
                            className="max-w-[140px] sm:max-w-[170px] truncate whitespace-nowrap px-2 py-0.5 bg-white/20 hover:bg-white/30 rounded-lg font-semibold transition-all"
                        >
                            {username || 'Set username'}
                        </button>
                    </div>
                </div>
            </div>

            {/* Username Prompt */}
            {showUsernamePrompt && (
                <div className="p-4 bg-gradient-to-r from-amber-50 to-orange-50 dark:from-amber-900/30 dark:to-orange-900/30 border-b border-amber-200 dark:border-amber-800">
                    <div className="flex items-center gap-2 mb-2">
                        <Smile className="w-4 h-4 text-amber-600" />
                        <label className="text-xs font-semibold text-amber-700 dark:text-amber-300">
                            Enter your name for comments
                        </label>
                    </div>
                    <div className="flex gap-2">
                        <input
                            type="text"
                            value={username}
                            onChange={(e) => setUsername(e.target.value)}
                            placeholder="Your name"
                            className="flex-1 px-3 py-2 text-sm rounded-xl border border-amber-300 dark:border-amber-700 bg-white dark:bg-slate-800 text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-amber-500 focus:border-transparent transition-all"
                            onKeyDown={(e) => e.key === 'Enter' && handleSetUsername(username)}
                        />
                        <button
                            onClick={() => handleSetUsername(username)}
                            disabled={!username.trim()}
                            className="px-4 py-2 bg-gradient-to-r from-amber-500 to-orange-500 text-white text-sm font-semibold rounded-xl disabled:opacity-50 hover:from-amber-600 hover:to-orange-600 transition-all shadow-lg shadow-amber-500/30"
                        >
                            Save
                        </button>
                    </div>
                </div>
            )}

            {/* Add Comment Form */}
            {isAddingComment && (
                <div className="p-4 bg-gradient-to-b from-slate-50 to-white dark:from-slate-900 dark:to-slate-950 border-b border-slate-200 dark:border-slate-800">
                    <div className="flex gap-2 mb-3">
                        {COMMENT_COLORS.map((color) => (
                            <button
                                key={color}
                                onClick={() => setSelectedColor(color)}
                                className={`w-7 h-7 rounded-full transition-all duration-200 ${
                                    selectedColor === color
                                        ? 'scale-125 ring-2 ring-offset-2 ring-amber-400 shadow-lg'
                                        : 'hover:scale-110'
                                }`}
                                style={{ backgroundColor: color, boxShadow: selectedColor === color ? `0 0 15px ${color}80` : undefined }}
                            />
                        ))}
                    </div>
                    <textarea
                        value={newComment}
                        onChange={(e) => setNewComment(e.target.value)}
                        placeholder="Write your comment..."
                        className="w-full px-4 py-3 text-sm rounded-xl border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-900 dark:text-white resize-none focus:outline-none focus:ring-2 focus:ring-amber-500 focus:border-transparent transition-all shadow-sm"
                        rows={3}
                        autoFocus
                    />
                    <div className="flex justify-end gap-2 mt-3">
                        <button
                            onClick={() => setIsAddingComment(false)}
                            className="px-4 py-2 text-sm text-slate-600 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-800 rounded-xl transition-all font-medium"
                        >
                            Cancel
                        </button>
                        <button
                            onClick={addComment}
                            disabled={!newComment.trim()}
                            className="flex items-center gap-2 px-4 py-2 bg-gradient-to-r from-amber-500 to-orange-500 text-white text-sm font-semibold rounded-xl disabled:opacity-50 hover:from-amber-600 hover:to-orange-600 transition-all shadow-lg shadow-amber-500/30 hover:shadow-amber-500/50 hover:scale-105 active:scale-95"
                        >
                            <Send className="w-4 h-4" />
                            Add Comment
                        </button>
                    </div>
                </div>
            )}

            {/* Comments List */}
            <div className="max-h-[clamp(12rem,32vh,18rem)] overflow-y-auto custom-scrollbar bg-slate-50 dark:bg-slate-950">
                {pageComments.length === 0 ? (
                    <div className="p-8 text-center">
                        <div className="w-16 h-16 mx-auto mb-4 rounded-2xl bg-gradient-to-br from-amber-100 to-orange-100 dark:from-amber-900/30 dark:to-orange-900/30 flex items-center justify-center">
                            <MessageSquare className="w-8 h-8 text-amber-400" />
                        </div>
                        <p className="text-sm text-slate-600 dark:text-slate-400 font-medium">
                            No comments yet
                        </p>
                        <p className="text-xs text-slate-500 dark:text-slate-500 mt-1">
                            Click + to add the first comment!
                        </p>
                    </div>
                ) : (
                    pageComments.map((comment, idx) => (
                        <div
                            key={comment.id}
                            className={`p-4 border-b border-slate-200 dark:border-slate-800 group hover:bg-white dark:hover:bg-slate-900 transition-all ${
                                idx === 0 ? 'border-t-0' : ''
                            }`}
                        >
                            <div className="flex items-start gap-3">
                                <div
                                    className="w-10 h-10 rounded-full flex items-center justify-center text-white text-sm font-bold flex-shrink-0 shadow-lg"
                                    style={{
                                        backgroundColor: comment.color,
                                        boxShadow: `0 4px 15px ${comment.color}40`
                                    }}
                                >
                                    {comment.username.charAt(0).toUpperCase()}
                                </div>
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center justify-between gap-2">
                                        <span className="text-sm font-bold text-slate-800 dark:text-white">
                                            {comment.username}
                                        </span>
                                        <button
                                            onClick={() => deleteComment(comment.id)}
                                            className="opacity-0 group-hover:opacity-100 p-1.5 text-red-500 hover:bg-red-100 dark:hover:bg-red-900/30 rounded-lg transition-all"
                                        >
                                            <X className="w-4 h-4" />
                                        </button>
                                    </div>
                                    <p className="text-sm text-slate-600 dark:text-slate-300 mt-1 leading-relaxed">
                                        {comment.text}
                                    </p>
                                    <span className="text-xs text-slate-400 dark:text-slate-500 mt-2 inline-flex items-center gap-1">
                                        {comment.timestamp.toLocaleTimeString()}
                                    </span>
                                </div>
                            </div>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
};

export default CollaborativeAnnotations;
