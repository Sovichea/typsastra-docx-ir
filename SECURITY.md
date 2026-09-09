# Security

DOCX and IR files are untrusted ZIP, XML, and JSON input. The core `ProcessingLimits` defaults bound:

- compressed source documents to 100 MiB
- ZIP archives to 10,000 entries, 256 MiB per expanded entry, and 1 GiB total expansion
- XML to 10,000,000 elements and 128 levels of nesting
- IR JSON to 256 MiB and 64 levels of nesting
- generated layouts to 10,000 pages, 1,000,000 regions, 5,000,000 fitted lines, 128 MiB of paragraph text, 192 MiB of aggregate strings, and 6,000,000 aggregate elements
- generated canonical IR JSON to 256 MiB

The standalone producer must apply `PackageBudget` to actual streamed bytes and XML events; ZIP directory metadata alone is not authoritative. Parsers must also disable external entities, reject unsafe OPC paths, and independently bound decoded image dimensions. `read_document` bounds raw JSON before deserialization and applies typed limits afterward; the raw byte cap, not the post-deserialization element counters, is the allocation boundary during Serde parsing. Canonical serialization checks typed limits before allocating its normalized clone and output value.

These defaults can reject legitimate unusually large documents. Library callers may supply explicit `ProcessingLimits` after evaluating their trusted workload and available resources.

Please report vulnerabilities privately through GitHub Security Advisories for this repository rather than opening a public issue.
