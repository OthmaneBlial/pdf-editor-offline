# Privacy and open-source community draft

## Suggested title

Offline PDF redaction with a content-free verification report

## Post

PDF Editor Offline is a local-first, MIT-licensed PDF workspace built around a
narrow privacy claim: document processing and telemetry egress are both zero in
the supported solo workflow.

The part I would like reviewed is Redact & Prove. It creates a separate copy,
reopens it, and checks extraction, renders, local OCR when available, annotations,
metadata, attachments, forms, scripts, raw objects, thumbnails, and earlier
revisions. Missing required evidence produces “incomplete,” not success. The JSON
report contains fixed check identifiers, counts, warnings, engine versions, and
an output hash; it excludes document content and local identifiers.

The repository includes the threat model, no-egress browser/backend tests,
network-inspection recipe, synthetic hostile/compatibility fixtures, and public
cross-engine results. Optional certificate signing uses request-only P12/PFX
material and explicit roots; it does not fetch trust data or infer legal status.

I am looking for critique of the privacy boundary and failure modes, not private
documents. If you found a PDF-specific bug, please recreate the minimum structure
with synthetic content under the fixture policy.

- Repository: https://github.com/OthmaneBlial/pdf-editor-offline
- Threat model: https://github.com/OthmaneBlial/pdf-editor-offline/blob/main/docs/THREAT_MODEL.md
- No-egress recipe: https://github.com/OthmaneBlial/pdf-editor-offline/blob/main/docs/NETWORK_INSPECTION.md
- Trust Lab: https://othmaneblial.github.io/pdf-editor-offline/trust-lab.html

## Before posting

Read and follow each community's self-promotion rules. Rewrite the introduction
for the actual discussion rather than cross-posting this verbatim. Do not use a
community logo, claim an audit/endorsement, or post before the signed release and
activation cohort are real.
