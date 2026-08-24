import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';

import SanitizeShareWorkflow from '../src/components/workflows/SanitizeShareWorkflow';

vi.mock('axios');

const useEditorMock = vi.fn();
vi.mock('../src/contexts/EditorContext', () => ({
  useEditor: () => useEditorMock(),
}));

const mockedAxios = axios as unknown as {
  get: ReturnType<typeof vi.fn>;
  post: ReturnType<typeof vi.fn>;
};

const profilesResponse = {
  data: {
    data: {
      profiles: [
        {
          id: 'minimal_metadata',
          label: 'Minimal metadata',
          description: 'Remove metadata only.',
          rasterizes_pages: false,
          destructive_effects: ['metadata_provenance_removed'],
        },
        {
          id: 'collaboration_cleanup',
          label: 'Collaboration cleanup',
          description: 'Remove review residue.',
          rasterizes_pages: false,
          destructive_effects: ['comments_removed', 'attachments_removed'],
        },
        {
          id: 'maximum_sanitization',
          label: 'Maximum sanitization',
          description: 'Flatten pages.',
          rasterizes_pages: true,
          destructive_effects: ['searchable_text_removed', 'accessibility_tags_removed'],
        },
      ],
    },
  },
};

const inventory = {
  pages: 2,
  file_bytes: 8192,
  metadata_fields: 3,
  xml_metadata: 1,
  attachments: 2,
  annotations: 4,
  links: 1,
  form_fields: 2,
  populated_form_fields: 1,
  signature_fields: 0,
  javascript_actions: 1,
  thumbnails: 2,
  layers: 1,
  previous_revisions: 0,
};

const previewResponse = {
  data: {
    data: {
      profile: 'collaboration_cleanup',
      profile_label: 'Collaboration cleanup',
      description: 'Remove review residue.',
      before: inventory,
      planned_removals: {
        metadata_fields: 3,
        xml_metadata: 1,
        attachments: 2,
        annotations: 4,
        populated_form_fields: 1,
        javascript_actions: 1,
        thumbnails: 2,
      },
      destructive_effects: ['comments_removed', 'attachments_removed'],
      warnings: ['comments_removed', 'attachments_removed'],
      source_will_be_preserved: true,
      preview_token: 'c'.repeat(64),
    },
  },
};

const afterInventory = {
  ...inventory,
  file_bytes: 4096,
  metadata_fields: 0,
  xml_metadata: 0,
  attachments: 0,
  annotations: 0,
  populated_form_fields: 0,
  javascript_actions: 0,
  thumbnails: 0,
};

const applyResponse = {
  data: {
    data: {
      status: 'completed',
      source_preserved: true,
      copy: {
        id: 'copy-2',
        filename: 'sharing-collaboration-cleanup.pdf',
        download_url: '/api/documents/copy-2/download',
      },
      report: {
        status: 'completed',
        profile: 'collaboration_cleanup',
        profile_label: 'Collaboration cleanup',
        app_version: '3.0.0',
        output_sha256: 'd'.repeat(64),
        before: inventory,
        after: afterInventory,
        removed: {
          file_bytes: 4096,
          metadata_fields: 3,
          xml_metadata: 1,
          attachments: 2,
          annotations: 4,
          populated_form_fields: 1,
          javascript_actions: 1,
          thumbnails: 2,
        },
        destructive_effects: ['comments_removed', 'attachments_removed'],
        warnings: [],
      },
      reports: {
        json: '/api/documents/copy-2/sanitize-report/json',
        markdown: '/api/documents/copy-2/sanitize-report/markdown',
      },
    },
  },
};

describe('SanitizeShareWorkflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEditorMock.mockReturnValue({ sessionId: 'doc-1' });
    mockedAxios.get.mockResolvedValue(profilesResponse);
    mockedAxios.post.mockImplementation((url: string) => {
      if (url.endsWith('/sanitize/preview')) return Promise.resolve(previewResponse);
      if (url.endsWith('/sanitize/apply')) return Promise.resolve(applyResponse);
      return Promise.reject(new Error('Unexpected endpoint'));
    });
  });

  it('requires a loaded document', () => {
    useEditorMock.mockReturnValue({ sessionId: '' });
    render(<SanitizeShareWorkflow />);
    expect(screen.getByText(/Upload a PDF to preview/i)).toBeInTheDocument();
  });

  it('compares the three profiles and previews exact removal counts', async () => {
    render(<SanitizeShareWorkflow />);

    expect(await screen.findByRole('radio', { name: /Minimal metadata/i })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /Collaboration cleanup/i })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('radio', { name: /Maximum sanitization/i })).toHaveTextContent('Rasterizes');

    fireEvent.click(screen.getByRole('button', { name: 'Preview this profile' }));

    expect(await screen.findByText('7 removal categories planned')).toBeInTheDocument();
    expect(screen.getByText('Comments & annotations')).toBeInTheDocument();
    expect(screen.getByText('4')).toBeInTheDocument();
    expect(screen.getByText(/Comments and annotations will be permanently removed/i)).toBeInTheDocument();
    expect(mockedAxios.post).toHaveBeenCalledWith(
      expect.stringContaining('/sanitize/preview'),
      { profile: 'collaboration_cleanup' },
    );
  });

  it('requires damage acknowledgement and renders a before-after audit diff', async () => {
    render(<SanitizeShareWorkflow />);
    await screen.findByRole('radio', { name: /Collaboration cleanup/i });
    fireEvent.click(screen.getByRole('button', { name: 'Preview this profile' }));
    const applyButton = await screen.findByRole('button', { name: 'Create sanitized copy' });

    expect(applyButton).toBeDisabled();
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(applyButton);

    expect(await screen.findByText('03 · Sharing copy ready')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'PDF copy' })).toBeInTheDocument();
    expect(screen.getByText('d'.repeat(64))).toBeInTheDocument();
    expect(mockedAxios.post).toHaveBeenCalledWith(
      expect.stringContaining('/sanitize/apply'),
      {
        profile: 'collaboration_cleanup',
        preview_token: 'c'.repeat(64),
        review_acknowledged: true,
      },
    );
  });
});
