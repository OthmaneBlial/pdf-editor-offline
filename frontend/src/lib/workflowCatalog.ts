import type { LucideIcon } from 'lucide-react';
import {
  Bookmark,
  FilePenLine,
  FileSearch,
  FileText,
  ImageIcon,
  Layers,
  PenTool,
  RefreshCw,
  ScanSearch,
  Scissors,
  Shield,
  ShieldCheck,
  Sparkles,
  Type,
  Zap,
} from 'lucide-react';

export type ViewMode =
  | 'editor'
  | 'redact'
  | 'fill-sign'
  | 'sanitize'
  | 'ocr'
  | 'manipulation'
  | 'conversion'
  | 'security'
  | 'advanced'
  | 'batch'
  | 'text'
  | 'navigation'
  | 'annotations'
  | 'images';

export type CommandGroup = 'workflow' | 'workspace' | 'specialist';

export interface WorkflowCommand {
  id: ViewMode;
  label: string;
  shortLabel: string;
  description: string;
  group: CommandGroup;
  keywords: string[];
  icon: LucideIcon;
  accent: string;
}

export const PRIMARY_WORKFLOWS: WorkflowCommand[] = [
  {
    id: 'redact',
    label: 'Redact & Prove',
    shortLabel: 'Redact',
    description: 'Remove sensitive content, sanitize the copy, and verify the result.',
    group: 'workflow',
    keywords: ['privacy', 'remove', 'verify', 'proof', 'sensitive'],
    icon: ShieldCheck,
    accent: 'from-rose-400 to-orange-300',
  },
  {
    id: 'fill-sign',
    label: 'Fill & Sign',
    shortLabel: 'Fill & Sign',
    description: 'Complete forms and create a clearly labelled visual or certificate signature.',
    group: 'workflow',
    keywords: ['form', 'signature', 'certificate', 'acroform', 'flatten'],
    icon: FilePenLine,
    accent: 'from-indigo-400 to-cyan-300',
  },
  {
    id: 'manipulation',
    label: 'Organize Pages',
    shortLabel: 'Organize',
    description: 'Reorder, merge, split, rotate, crop, extract, and assemble pages.',
    group: 'workflow',
    keywords: ['pages', 'merge', 'split', 'rotate', 'crop', 'bates'],
    icon: Scissors,
    accent: 'from-amber-300 to-lime-300',
  },
  {
    id: 'sanitize',
    label: 'Sanitize & Share',
    shortLabel: 'Sanitize',
    description: 'Inspect hidden structures and prepare a safer sharing copy.',
    group: 'workflow',
    keywords: ['metadata', 'attachments', 'comments', 'scripts', 'cleanup'],
    icon: Sparkles,
    accent: 'from-emerald-300 to-lime-200',
  },
  {
    id: 'ocr',
    label: 'OCR & Search',
    shortLabel: 'OCR',
    description: 'Make scans searchable locally, then inspect and correct the text layer.',
    group: 'workflow',
    keywords: ['scan', 'search', 'tesseract', 'language', 'confidence'],
    icon: ScanSearch,
    accent: 'from-cyan-300 to-violet-400',
  },
];

export const SECONDARY_COMMANDS: WorkflowCommand[] = [
  {
    id: 'editor',
    label: 'PDF Editor',
    shortLabel: 'Editor',
    description: 'Open the canvas for overlays, annotations, zoom, and page navigation.',
    group: 'workspace',
    keywords: ['canvas', 'document', 'open', 'overlay', 'home'],
    icon: FileText,
    accent: 'from-sky-400 to-cyan-300',
  },
  {
    id: 'conversion',
    label: 'Convert formats',
    shortLabel: 'Conversion',
    description: 'Use documented local PDF and office conversion tools.',
    group: 'workspace',
    keywords: ['word', 'excel', 'image', 'html', 'markdown', 'convert'],
    icon: RefreshCw,
    accent: 'from-sky-400 to-blue-400',
  },
  {
    id: 'security',
    label: 'Security tools',
    shortLabel: 'Security',
    description: 'Inspect encryption, permissions, signatures, and document safety.',
    group: 'workspace',
    keywords: ['password', 'encrypt', 'permission', 'signature', 'validate'],
    icon: Shield,
    accent: 'from-emerald-400 to-cyan-300',
  },
  {
    id: 'batch',
    label: 'Batch processing',
    shortLabel: 'Batch',
    description: 'Apply supported local operations to a selected file set.',
    group: 'workspace',
    keywords: ['multiple', 'files', 'bulk', 'queue'],
    icon: Layers,
    accent: 'from-violet-400 to-fuchsia-400',
  },
  {
    id: 'advanced',
    label: 'Advanced tools',
    shortLabel: 'Advanced',
    description: 'Open specialist inspection and transformation controls.',
    group: 'specialist',
    keywords: ['expert', 'inspect', 'objects', 'structure'],
    icon: Zap,
    accent: 'from-amber-300 to-orange-400',
  },
  {
    id: 'text',
    label: 'Text tools',
    shortLabel: 'Text',
    description: 'Add and inspect text overlays with explicit font behavior.',
    group: 'specialist',
    keywords: ['font', 'overlay', 'replace', 'extract'],
    icon: Type,
    accent: 'from-cyan-300 to-sky-400',
  },
  {
    id: 'navigation',
    label: 'Bookmarks & navigation',
    shortLabel: 'Navigation',
    description: 'Manage bookmarks, destinations, labels, and navigation structure.',
    group: 'specialist',
    keywords: ['bookmark', 'outline', 'destination', 'label'],
    icon: Bookmark,
    accent: 'from-lime-300 to-emerald-400',
  },
  {
    id: 'annotations',
    label: 'Annotations',
    shortLabel: 'Annotations',
    description: 'Create and inspect standard local PDF annotations.',
    group: 'specialist',
    keywords: ['comment', 'highlight', 'stamp', 'drawing'],
    icon: PenTool,
    accent: 'from-rose-400 to-violet-400',
  },
  {
    id: 'images',
    label: 'Image tools',
    shortLabel: 'Images',
    description: 'Inspect, place, extract, or transform document images.',
    group: 'specialist',
    keywords: ['picture', 'photo', 'extract', 'replace', 'compress'],
    icon: ImageIcon,
    accent: 'from-fuchsia-400 to-orange-300',
  },
];

export const ALL_COMMANDS = [...PRIMARY_WORKFLOWS, ...SECONDARY_COMMANDS];

export const VIEW_LABELS: Record<ViewMode, string> = Object.fromEntries(
  ALL_COMMANDS.map(command => [command.id, command.shortLabel]),
) as Record<ViewMode, string>;

export function commandSearchText(command: WorkflowCommand): string {
  return [command.label, command.shortLabel, command.description, ...command.keywords]
    .join(' ')
    .toLocaleLowerCase();
}

export function commandGroupLabel(group: CommandGroup): string {
  if (group === 'workflow') return 'Primary workflows';
  if (group === 'workspace') return 'Workspace tools';
  return 'Specialist tools';
}

export const COMMAND_PALETTE_HINT = '⌘ K';

export const COMMAND_PALETTE_ICON = FileSearch;
