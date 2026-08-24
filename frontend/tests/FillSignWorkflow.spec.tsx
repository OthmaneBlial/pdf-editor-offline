import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import axios from 'axios';
import FillSignWorkflow from '../src/components/workflows/FillSignWorkflow';

const editor = vi.hoisted(() => ({
  sessionId: 'doc-forms' as string | null,
  pageCount: 2,
  currentPage: 0,
  documentMutationVersion: 0,
  reportToolResult: vi.fn(),
}));

const assetMocks = vi.hoisted(() => ({
  asset: {
    id: 'asset-1',
    kind: 'imported' as const,
    dataUrl: 'data:image/png;base64,c2ln',
    createdAt: '2026-08-24T00:00:00.000Z',
  },
  delete: vi.fn(() => []),
  save: vi.fn(),
  toFile: vi.fn(async () => new File(['signature'], 'visual-signature.png', { type: 'image/png' })),
}));

const saveBlob = vi.hoisted(() => vi.fn(async () => undefined));

vi.mock('../src/contexts/EditorContext', () => ({ useEditor: () => editor }));
vi.mock('../src/lib/apiClient', () => ({ API_BASE_URL: 'http://127.0.0.1:8000' }));
vi.mock('../src/lib/downloads', () => ({ saveBlob }));
vi.mock('../src/services/signatureAssets', () => ({
  loadSignatureAssets: () => [assetMocks.asset],
  saveSignatureAsset: assetMocks.save,
  deleteSignatureAsset: assetMocks.delete,
  signatureAssetToFile: assetMocks.toFile,
}));
vi.mock('axios', () => ({
  default: {
    get: vi.fn(),
    put: vi.fn(),
    post: vi.fn(),
    isAxiosError: vi.fn(() => false),
  },
}));

const mockedAxios = vi.mocked(axios, true);

const inventory = {
  fields: [
    { id: '0:1', page: 0, name: 'full_name', label: 'Full Name', value: 'Ada', type: 'Text', field_type: 'text', choices: [], button_values: [], read_only: false, required: true, tab_index: 1 },
    { id: '0:2', page: 0, name: 'approved', label: 'Approved', value: 'Yes', type: 'CheckBox', field_type: 'checkbox', choices: [], button_values: ['Yes'], read_only: false, required: false, tab_index: 2 },
    { id: '0:3', page: 0, name: 'priority', label: 'Priority', value: 'Normal', type: 'ComboBox', field_type: 'dropdown', choices: ['Low', 'Normal', 'High'], button_values: [], read_only: false, required: false, tab_index: 3 },
    { id: '0:4', page: 0, name: 'signed_date', label: 'Signed Date', value: '2026-08-24', type: 'Text', field_type: 'date', choices: [], button_values: [], read_only: false, required: false, tab_index: 4 },
  ],
  field_count: 4,
  has_xfa: false,
  javascript_actions: 1,
  calculation_actions: 1,
  signature_fields: 0,
  warnings: ['javascript_not_executed', 'calculations_not_executed'],
};

describe('FillSignWorkflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    editor.sessionId = 'doc-forms';
    mockedAxios.get.mockResolvedValue({ data: { data: inventory } });
    mockedAxios.put.mockResolvedValue({ data: { data: { warnings: [], can_undo: true, can_redo: false } } });
    mockedAxios.post.mockImplementation(async url => {
      if (String(url).includes('/digital-signatures/sign-copy')) {
        return { data: new Blob(['signed-pdf'], { type: 'application/pdf' }), headers: { 'x-source-preserved': 'true' } };
      }
      if (String(url).includes('/digital-signatures/validate')) {
        return { data: { data: {
          status: 'signed',
          signature_count: 1,
          all_intact: true,
          all_cryptographically_valid: true,
          all_documents_unchanged_since_signature: true,
          all_trusted: true,
          trust_roots_supplied: true,
          trust_model: 'explicit_roots_only',
          network_fetching: false,
          revocation_status: 'not_checked_offline',
          signatures: [{
            index: 0,
            field_name: 'OfflineSignature',
            intact: true,
            cryptographically_valid: true,
            trusted: true,
            trust_status: 'trusted_explicit_root',
            coverage: 'entire_file',
            modification_level: 'none',
            document_unchanged_since_signature: true,
            certificate: { subject_common_name: 'Synthetic Offline Signer', issuer_common_name: 'Synthetic Offline Signer', sha256_fingerprint: 'abcdef', valid_from: '2026-08-23', valid_until: '2026-09-23' },
          }],
        } } };
      }
      if (String(url).includes('/flatten-copy')) {
        return { data: new Blob(['pdf'], { type: 'application/pdf' }), headers: { 'x-fields-flattened': '4', 'x-pdf-editor-warnings': 'editable_original_preserved' } };
      }
      return { data: { data: { warnings: ['visual_signature_is_not_digital_signature'], can_undo: true, can_redo: false } } };
    });
  });

  it('shows a focused upload prompt without a document', () => {
    editor.sessionId = null;
    render(<FillSignWorkflow />);
    expect(screen.getByRole('heading', { name: 'Fill & Sign' })).toBeInTheDocument();
    expect(screen.getByText(/Upload a PDF to fill standard AcroForms/)).toBeInTheDocument();
  });

  it('renders typed fields in tab order and saves standard values', async () => {
    render(<FillSignWorkflow />);
    expect(await screen.findByLabelText(/1\. Full Name/)).toHaveValue('Ada');
    expect(screen.getByLabelText(/2\. Approved/)).toBeChecked();
    expect(screen.getByLabelText(/3\. Priority/)).toHaveValue('Normal');
    expect(screen.getByLabelText(/4\. Signed Date/)).toHaveAttribute('type', 'date');
    expect(screen.getByText(/Embedded JavaScript is not executed/)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText(/1\. Full Name/), { target: { value: 'Grace Hopper' } });
    fireEvent.click(screen.getByLabelText(/2\. Approved/));
    fireEvent.change(screen.getByLabelText(/3\. Priority/), { target: { value: 'High' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save standard form values' }));

    await waitFor(() => expect(mockedAxios.put).toHaveBeenCalledWith(
      expect.stringContaining('/api/documents/doc-forms/forms'),
      { fields: expect.arrayContaining([
        { name: 'full_name', value: 'Grace Hopper' },
        { name: 'approved', value: 'false' },
        { name: 'priority', value: 'High' },
      ]) },
    ));
    expect(editor.reportToolResult).toHaveBeenCalledWith('success', 'Form fields saved', true);
  });

  it('downloads a flattened copy and leaves the editable session open', async () => {
    render(<FillSignWorkflow />);
    fireEvent.click(await screen.findByRole('button', { name: 'Flatten sharing copy' }));

    await waitFor(() => expect(saveBlob).toHaveBeenCalledWith(expect.any(Blob), 'flattened-sharing-copy.pdf'));
    expect(screen.getByRole('status')).toHaveTextContent('editable original is still open');
  });

  it('places and explicitly deletes a locally stored visual signature', async () => {
    render(<FillSignWorkflow />);
    fireEvent.click(await screen.findByRole('button', { name: 'Select imported visual signature' }));
    fireEvent.click(screen.getByRole('button', { name: 'Place visual signature' }));

    await waitFor(() => expect(assetMocks.toFile).toHaveBeenCalled());
    const signatureCall = mockedAxios.post.mock.calls.find(call => String(call[0]).includes('/visual-signatures'));
    expect(signatureCall).toBeDefined();
    expect(signatureCall?.[1]).toBeInstanceOf(FormData);
    expect((signatureCall?.[1] as FormData).get('page_num')).toBe('0');
    expect(await screen.findByText(/visual mark, not a certificate-backed digital signature/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Delete imported visual signature' }));
    expect(assetMocks.delete).toHaveBeenCalledWith('asset-1');
  });

  it('keeps certificate signing separate, clears the passphrase, and validates with explicit trust', async () => {
    render(<FillSignWorkflow />);
    await screen.findByRole('heading', { name: 'Digital signing & offline validation' });

    const certificate = new File(['p12'], 'synthetic.p12', { type: 'application/x-pkcs12' });
    fireEvent.change(screen.getByLabelText('Import P12 or PFX certificate'), { target: { files: [certificate] } });
    const passphrase = screen.getByLabelText(/Passphrase · never stored/);
    fireEvent.change(passphrase, { target: { value: 'one-request-only' } });
    fireEvent.change(screen.getByLabelText(/Reason · optional/), { target: { value: 'Local approval' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create separate signed copy' }));

    await waitFor(() => expect(saveBlob).toHaveBeenCalledWith(expect.any(Blob), 'certificate-signed-copy.pdf'));
    const signingCall = mockedAxios.post.mock.calls.find(call => String(call[0]).includes('/digital-signatures/sign-copy'));
    const signingForm = signingCall?.[1] as FormData;
    expect(signingForm.get('certificate')).toBe(certificate);
    expect(signingForm.get('passphrase')).toBe('one-request-only');
    expect(signingForm.get('reason')).toBe('Local approval');
    expect(passphrase).toHaveValue('');
    expect(assetMocks.save).not.toHaveBeenCalled();

    const root = new File(['pem'], 'root.pem', { type: 'application/x-pem-file' });
    fireEvent.change(screen.getByLabelText('Import explicit trust root'), { target: { files: [root] } });
    fireEvent.click(screen.getByRole('button', { name: 'Validate offline' }));

    expect(await screen.findByText('Synthetic Offline Signer')).toBeInTheDocument();
    expect(screen.getByLabelText('Digital signature validation result')).toHaveTextContent('Valid');
    expect(screen.getByLabelText('Digital signature validation result')).toHaveTextContent('Trusted');
    expect(screen.getByLabelText('Digital signature validation result')).toHaveTextContent('Not checked');
    const validationCall = mockedAxios.post.mock.calls.find(call => String(call[0]).includes('/digital-signatures/validate'));
    expect((validationCall?.[1] as FormData).get('trust_roots')).toBe(root);
  });
});
