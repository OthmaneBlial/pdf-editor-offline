import { useState } from 'react';
import { useEditor } from '../contexts/EditorContext';
import { MessageSquare, User, Send, X, Plus } from 'lucide-react';

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
        <div className="bg-[var(--card-bg)] border border-[var(--border-color)] rounded-xl shadow-lg overflow-hidden">
            {/* Header */}
            <div className="p-3 border-b border-[var(--border-color)] bg-[var(--bg-secondary)]/50">
                <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        <MessageSquare className="w-4 h-4 text-[var(--color-primary-600)]" />
                        <span className="text-sm font-semibold text-[var(--text-primary)]">
                            Comments ({pageComments.length})
                        </span>
                    </div>
                    <button
                        onClick={() => setIsAddingComment(true)}
                        className="p-1.5 bg-[var(--color-primary-600)] text-white rounded-lg hover:bg-[var(--color-primary-700)] transition-colors"
                        title="Add Comment"
                    >
                        <Plus className="w-3.5 h-3.5" />
                    </button>
                </div>

                {/* Username display */}
                <div className="mt-2 flex items-center gap-2 text-xs text-[var(--text-secondary)]">
                    <User className="w-3 h-3" />
                    <span>Commenting as: </span>
                    <button
                        onClick={() => setShowUsernamePrompt(true)}
                        className="font-medium text-[var(--color-primary-600)] hover:underline"
                    >
                        {username || 'Set username'}
                    </button>
                </div>
            </div>

            {/* Username Prompt */}
            {showUsernamePrompt && (
                <div className="p-3 border-b border-[var(--border-color)] bg-[var(--color-primary-50)]">
                    <label className="text-xs text-[var(--text-secondary)] block mb-1">
                        Enter your name for comments:
                    </label>
                    <div className="flex gap-2">
                        <input
                            type="text"
                            value={username}
                            onChange={(e) => setUsername(e.target.value)}
                            placeholder="Your name"
                            className="flex-1 px-2 py-1.5 text-xs rounded-lg border border-[var(--border-color)] bg-[var(--input-bg)] text-[var(--text-primary)]"
                            onKeyDown={(e) => e.key === 'Enter' && handleSetUsername(username)}
                        />
                        <button
                            onClick={() => handleSetUsername(username)}
                            disabled={!username.trim()}
                            className="px-3 py-1.5 bg-[var(--color-primary-600)] text-white text-xs rounded-lg disabled:opacity-50"
                        >
                            Save
                        </button>
                    </div>
                </div>
            )}

            {/* Add Comment Form */}
            {isAddingComment && (
                <div className="p-3 border-b border-[var(--border-color)] bg-[var(--bg-primary)]/50">
                    <div className="flex gap-2 mb-2">
                        {COMMENT_COLORS.map(color => (
                            <button
                                key={color}
                                onClick={() => setSelectedColor(color)}
                                className={`w-5 h-5 rounded-full transition-transform ${selectedColor === color ? 'scale-125 ring-2 ring-[var(--text-primary)]' : ''
                                    }`}
                                style={{ backgroundColor: color }}
                            />
                        ))}
                    </div>
                    <textarea
                        value={newComment}
                        onChange={(e) => setNewComment(e.target.value)}
                        placeholder="Write your comment..."
                        className="w-full px-3 py-2 text-xs rounded-lg border border-[var(--border-color)] bg-[var(--input-bg)] text-[var(--text-primary)] resize-none"
                        rows={3}
                        autoFocus
                    />
                    <div className="flex justify-end gap-2 mt-2">
                        <button
                            onClick={() => setIsAddingComment(false)}
                            className="px-3 py-1.5 text-xs text-[var(--text-secondary)] hover:bg-[var(--hover-bg)] rounded-lg transition-colors"
                        >
                            Cancel
                        </button>
                        <button
                            onClick={addComment}
                            disabled={!newComment.trim()}
                            className="flex items-center gap-1 px-3 py-1.5 bg-[var(--color-primary-600)] text-white text-xs rounded-lg disabled:opacity-50 transition-colors"
                        >
                            <Send className="w-3 h-3" />
                            Add
                        </button>
                    </div>
                </div>
            )}

            {/* Comments List */}
            <div className="max-h-64 overflow-y-auto custom-scrollbar">
                {pageComments.length === 0 ? (
                    <div className="p-4 text-center text-xs text-[var(--text-secondary)]">
                        No comments on this page yet.
                        <br />
                        Click + to add the first one!
                    </div>
                ) : (
                    pageComments.map(comment => (
                        <div
                            key={comment.id}
                            className="p-3 border-b border-[var(--border-color)]/50 last:border-0 group hover:bg-[var(--hover-bg)] transition-colors"
                        >
                            <div className="flex items-start gap-2">
                                <div
                                    className="w-6 h-6 rounded-full flex items-center justify-center text-white text-[10px] font-bold flex-shrink-0"
                                    style={{ backgroundColor: comment.color }}
                                >
                                    {comment.username.charAt(0).toUpperCase()}
                                </div>
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center justify-between gap-2">
                                        <span className="text-xs font-medium text-[var(--text-primary)]">
                                            {comment.username}
                                        </span>
                                        <button
                                            onClick={() => deleteComment(comment.id)}
                                            className="opacity-0 group-hover:opacity-100 p-1 text-red-500 hover:bg-red-50 rounded transition-all"
                                        >
                                            <X className="w-3 h-3" />
                                        </button>
                                    </div>
                                    <p className="text-xs text-[var(--text-secondary)] mt-0.5">
                                        {comment.text}
                                    </p>
                                    <span className="text-[10px] text-[var(--text-secondary)]/70 mt-1 block">
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
