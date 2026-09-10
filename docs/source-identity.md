# Source identity

A Typsastra source identity is the complete tuple `(part, identity, id)`. Consumers must never compare `id` without its package-relative OPC `part`. The same native ID in `word/document.xml` and `word/footnotes.xml` identifies two different editable nodes.

## Native paragraph IDs

For a physical WordprocessingML paragraph, producers prefer the `paraId` attribute in the `http://schemas.microsoft.com/office/word/2010/wordml` namespace. Namespace prefixes are irrelevant: the attribute is resolved by its namespace URI and local name.

A valid native value contains exactly eight hexadecimal digits. Producers canonicalize it to uppercase and emit `identity: "para_id"`; canonical IR requires that uppercase spelling so numerically identical IDs cannot become different string keys. Invalid or duplicate values within one OPC part are errors because silently selecting a fallback would make identity depend on parser tolerance. The same value may appear in different parts because the part qualifies it.

## Structural fallback

When a paragraph has no native ID, its identity is its physical XML location:

```text
path-v1:/Q{namespace-uri}local-name[index]/Q{namespace-uri}local-name[index]
```

Indexes are one-based among sibling elements with the same expanded name. Namespace URIs and local names—not source prefixes—make paths deterministic across XML namespace-prefix changes. `%` and `}` in namespace URIs are escaped as `%25` and `%7D`. Text nodes, comments, processing instructions, attributes, paragraph text, and formatting do not participate.

For example:

```text
part: word/document.xml
id: path-v1:/Q{http://schemas.openxmlformats.org/wordprocessingml/2006/main}document[1]/Q{http://schemas.openxmlformats.org/wordprocessingml/2006/main}body[1]/Q{http://schemas.openxmlformats.org/wordprocessingml/2006/main}p[2]
identity: generated_path
```

A generated path is a deterministic structural locator, not a persistent node UUID.

## Expected changes after editing

| Edit | Native `paraId` | Generated path |
|---|---|---|
| change paragraph text | unchanged | unchanged |
| change paragraph or run style | unchanged | unchanged |
| add or remove children inside the paragraph | unchanged | unchanged |
| change namespace prefixes or attribute order | unchanged | unchanged |
| insert a differently named sibling before it | unchanged | unchanged |
| insert or delete a same-name sibling before it | unchanged | changes |
| move or reorder the paragraph | unchanged if the editor retains the ID | changes |
| wrap or unwrap it in another element | unchanged | changes |
| move it to another OPC part | qualified identity changes | changes |
| delete it | identity disappears | its path may be reused by a following node |
| replace it at the same position | changes only if the replacement gets a new ID | path is reused and cannot reveal replacement |
| split it across pages | unchanged on every page segment | unchanged on every page segment |

Editors control native-ID behavior. If an editor preserves an old `paraId` while replacing a node, consumers cannot infer replacement from identity alone. Likewise, structural fallbacks intentionally cannot distinguish replacement at the same physical path.

No bookmarks, hidden text, or marker runs are injected to improve fallback identity because modifying the source can change the layout being measured.

## Format v2 nodes

The v2 contract applies `SourceRef` to stories, paragraphs, tables, rows, cells, images, shapes, and sections. The initial contract retains the v1 identity kinds; nodes without a suitable paragraph or relationship identifier use the same `path-v1` physical structural grammar. The path grammar version is independent of the IR format major. Producer extraction for non-paragraph nodes remains Milestone 4 work.

An image or image fill records two different facts: its structural `SourceRef` identifies the editable drawing node, while `RelationshipRef` identifies the source OPC part, relationship ID, and target media part. Consumers must not substitute one for the other.

V2 `RegionId` values identify laid-out instances and are not source identities. Repeated headers, footers, and split content may therefore share one `SourceRef` while using distinct region IDs.

## Processing and pagination

Identity is attached immediately after scanning the physical XML and carried as immutable provenance through future style resolution and pagination stages. `Sourced<T>` moves the same `SourceRef` through transformations without exposing mutable source state. The standalone producer introduced in Milestone 3 must use these APIs rather than reconstructing identity from text or layout output.

Pagination produces all fragments before `finish_paginated_paragraph` assigns indexes. Every fragment receives the same source and logical text, `segment` is zero-based in page order, and `segment_count` is the total number of fragments for that occurrence. Repeated story occurrences must be finalized separately so repeated headers do not become one multi-page paragraph.

Semantic validation accepts complete segment cycles such as `0/2, 1/2` and repeated independent cycles such as `0/1, 0/1`. It rejects missing, reordered, overlapping, text-inconsistent, or incomplete segments.
