# Certificate-backed digital signatures

Certificate signing is a separate trust boundary inside **Fill & Sign**. It does not reuse, upgrade, or validate a typed, drawn, or imported Visual Signature.

## Signing contract

**Create separate signed copy** accepts one PKCS#12 identity (`.p12` or `.pfx`, maximum 4 MB), its passphrase, a field name, an optional reason/location, and a visible page rectangle. The loopback API:

1. stores the bounded certificate upload in the app-owned temporary directory;
2. opens the private key only for this request;
3. adds or fills a PDF signature field and signs a new incremental revision with SHA-256;
4. returns a separate PDF download without changing the open editable session;
5. deletes the certificate upload and signed temporary output, and never writes the passphrase.

There is no certificate library, remembered key, browser storage entry, log field, recovery copy, or application database record for the private key or passphrase. The UI clears both the file input and passphrase after success or failure.

The workflow does not contact a timestamp authority. The signer-reported clock value is therefore not a trusted timestamp. It also does not add online OCSP/CRL evidence or long-term validation data.

## Offline validation contract

**Validate offline** reports independent facts for every embedded signature:

- whether the signed byte range is intact and the CMS signature is cryptographically valid;
- whether later PDF revisions or modifications are present;
- the signature field, digest/signature mechanism, coverage, signer-reported time, and a bounded certificate summary;
- whether the certificate chains to a trust root supplied explicitly for that request.

The application never silently imports the operating-system or browser TLS trust store. With no explicit PEM/DER root, a cryptographically valid signature remains **not trusted**. A supplied root is bounded to 2 MB, used only for that validation request, and then deleted.

Network fetching is disabled in the validation context. OCSP/CRL revocation status is therefore reported as `not_checked_offline`, never inferred as good. This is intentionally different from identity, legal authority, signature policy, qualified-signature status, or a trusted timestamp.

## Reading a result

| Result | Meaning |
| --- | --- |
| Cryptography: Valid | The embedded signature matches its covered bytes and certificate key. |
| Later changes: None | The signature covers the entire current file and no later modification is classified. |
| Later changes: Detected | A later revision or modification exists; inspect it even if the original signed revision remains cryptographically intact. |
| Explicit trust: Trusted | The signer certificate validates against the root supplied for this request. |
| Explicit trust: Not established | No root was supplied or the chain could not be established. |
| Revocation: Not checked | No live or embedded OCSP/CRL conclusion was made. |

After downloading a signed copy, reopen it in PDF Editor Offline and run **Validate offline**. For high-assurance or regulated use, also validate in an independently maintained reader under the applicable organizational certificate policy.

## Failure and cleanup behavior

Invalid PKCS#12 data, a wrong passphrase, an already-filled signature field, invalid geometry, malformed trust roots, and unsafe file sizes fail closed. No signed output is offered after failure. Operation errors do not return local paths, key material, passphrases, certificate bytes, or document-derived contents.

Executable evidence lives in `tests/test_digital_signatures.py` and `frontend/tests/FillSignWorkflow.spec.tsx`.
