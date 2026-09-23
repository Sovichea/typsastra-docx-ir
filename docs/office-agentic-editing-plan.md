# Typsastra Office — Agentic Editing Implementation Plan (Word)

Status: proposal · Scope: document editor (Word) first · Owner: TBD

## Objective

Turn the in-editor agent from **"edit and hope"** into
**"edit → observe → verify → repair"** by making layout and quality feedback a
first-class, deterministic part of the loop.

This plan adopts the *concept* of this repository — source-aware layout IR with
durable identities, resolved appearance, coverage declarations, deterministic
serialization — but produces that feedback **from the editor's own model**, so
it matches what the user actually sees and needs no PDF or file round-trip.

## Principles

1. **Feedback first.** The fastest quality win is letting the agent see the
   result, not adding more edit tools.
2. **One identity space.** Edits, feedback, and diffs reference the same durable
   node ids.
3. **Bounded and deterministic.** Snapshots are compact, versioned, canonical;
   findings carry severity and evidence.
4. **Honest coverage.** The snapshot declares what it knows; the agent never
   infers layout it did not measure.
5. **Stage it.** Ship a geometry-free feedback pass first (pure model reads),
   then add geometry from the editor engine.

## Architecture at a glance

```text
        USER TASK
            |
   +--------v--------+
   |  Agent planner   |  (system prompt + skills + doc outline)
   +--------+--------+
            | tool calls
   +--------v--------+        +--------------------------+
   |  Edit tools      |<------>|  Document model (engine) |
   |  (builder ops)   |        |  live, no file round-trip|
   +--------+--------+        +------------+-------------+
            |                              |
            |        +---------------------v---------------------+
            |        |  AGENT DOC SNAPSHOT (feedback IR)          |
            |        |  nodes - geometry - resolved appearance -  |
            |        |  findings - coverage   (durable ids)       |
            |        +---------------------+---------------------+
            |                              |
   +--------v------------------------------v--------+
   |  VERIFY: findings + diff(before, after)          |
   |   -> clean? finish : feed back -> repair         |
   +--------------------------------------------------+
```

## The feedback contract (`AgentDocSnapshot`)

Versioned JSON, aligned with this repository's contract shape but emitted by the
editor:

- `meta`: `{ schema: "tysastra.agent.doc/1.x", generator, docFingerprint, coverage{...} }`
- `pages[]`: `{ index, size, margins, headerPresent, footerPresent, contentBounds }`
- `nodes[]` keyed by `id = (part, identity, id)` (paragraph `w14:paraId` when
  available, otherwise a structural path):
  - `kind`: paragraph | table | image | shape | toc
  - `style`: resolved name plus **provenance** (style vs direct run overrides)
  - `runs[]`: resolved `{ text, font, size, color, bold, italic, source }`
  - `text`: trimmed, with UTF-8 line ranges
  - `geometry`: `{ page, x, y, w, h, baseline }` *(phase 1)*
  - `table`: `{ rows, cols, columnWidths, headerRepeat, cellShading[], cellText[] }` *(phase 1)*
  - `flags`: `overflow`, `fontSubstituted`, `styleOverridden`
- `findings[]`: `{ code, severity, nodeId, message, evidence }`
- `coverage`: `{ pagination, text_geometry, resolved_typography, table_geometry,
  drawing_appearance, header_footer }` each `unknown | partial | complete | absent`

**Diffs:** `diff(before, after)` returns
`{ addedNodes, removedNodes, changedNodes, newFindings, clearedFindings }`,
keyed by id.

## Findings catalog (the oracle)

Objective, source-linked findings turn measurements into agent guidance:

| Code | Catches |
| --- | --- |
| `PAGE_OVERFLOW` / `COLUMN_OVERFLOW` | content past page or column |
| `HEADER_OVERLAP` / `HEADER_CLIPPED` | running header clipped or overlapping body |
| `TABLE_WIDTH_OVERFLOW` / `CELL_TEXT_CLIPPED` | cramped table columns |
| `ORPHAN_HEADING` / `WIDOW_LINE` | heading at page bottom; widows |
| `DOUBLE_NUMBERING` | manual number plus automatic numbering |
| `EMPTY_SECTION` | heading with no content |
| `MISSING_CAPTION` | table or image without a caption |
| `DIRECT_FORMAT_OVERRIDE` | paste formatting beating the named styles |
| `STYLE_UNUSED` / `STYLE_MISSING` | style set the content does not use |
| `TOC_STALE` / `TOC_MISSING` | empty or out-of-date table of contents |
| `FONT_SUBSTITUTED` | missing font fell back to a substitute |
| `ACCENT_INCONSISTENT` | mixed heading colours |

Findings are advisory or blocking by severity, and every finding carries the
`nodeId` so the agent can jump straight to it.

## Agent loop redesign

Replace the current blind review with a findings-driven state machine:

1. **PLAN** — the model receives a bounded outline (styles, headings, counts),
   not raw HTML.
2. **ACT** — existing edit tools (surface unchanged initially).
3. **OBSERVE** — after each mutation batch, request a snapshot **delta**
   (cheap, bounded).
4. **VERIFY** — compute findings and diff. If new or blocking findings exist,
   build a repair prompt containing only the relevant nodes and findings.
5. **REPAIR** — loop back to ACT within a bounded round and token budget.
6. **FINISH** — when no blocking findings remain or the budget is exhausted
   (report what is unresolved).

Key changes:

- The review cap becomes *"review while findings exist"*, with a hard limit.
- The review instruction becomes *"call `get_document_feedback`; fix each
  blocking finding by `nodeId`."*
- Drop the current conform / recolor workarounds; the snapshot replaces them.
- Keep conversation history bounded; snapshots enter context as a summary plus
  top-N findings, with full detail on demand.

New read-side tools:

- `get_document_feedback({ scope, detail, includeGeometry })`
- `get_document_diff({ since })`
- `get_node({ id })`

## Where it plugs in

| Concern | Component | Change |
| --- | --- | --- |
| Feedback producer (no geometry) | agent plugin (TS) | new `getAgentSnapshot()` reading styles, runs, numbering, tables via the builder |
| Feedback producer (geometry) | editor engine plugin method | new `GetAgentDocumentSnapshot` serializer |
| Loop / review | agent orchestration | findings-driven verify and repair |
| Prompt and skills | agent provider | teach the verify workflow |
| UI | agent panel | "Document health" findings list, click to select the node |
| Writer quality (phase 3) | editor builder | merge-paste, clear-formatting, durable-id ops |

## Phased roadmap

### Phase 0 — Feedback MVP, no geometry (about 1 week)

- Define the `tysastra.agent.doc/1.0` contract and finding codes.
- Implement `get_document_feedback` from the live model: paragraph and run
  provenance, numbering, tables, TOC presence.
- Findings available now: `DOUBLE_NUMBERING`, `DIRECT_FORMAT_OVERRIDE`,
  `EMPTY_SECTION`, `MISSING_CAPTION`, `TOC_MISSING`, `STYLE_UNUSED`.
- Wire the loop to be findings-driven; remove the conform and recolor hacks.

Acceptance: on the quotation corpus the agent flags double numbering, direct
format overrides, empty sections and missing captions with correct `nodeId`s, and
reports zero blocking findings on the known-good build.

### Phase 1 — Geometry via the editor engine (about 2-3 weeks)

- The engine emits pagination, text geometry, resolved typography, table
  geometry, and header/footer presence — paragraphs first, then tables, headers
  and footers, images.
- Findings: `HEADER_OVERLAP` / `HEADER_CLIPPED`, `PAGE_OVERFLOW`,
  `TABLE_WIDTH_OVERFLOW`, `ORPHAN_HEADING`, `WIDOW_LINE`, `FONT_SUBSTITUTED`.

Acceptance: header-clipping and cramped-table cases are detected automatically;
snapshot cost under roughly 50 ms on a five-page document.

### Phase 2 — Diff, self-repair, budgets (about 1-2 weeks)

- `diff(before, after)` with id-keyed changes; repair prompts generated from
  findings plus diff.
- Budgets for rounds, tokens, and snapshot bytes; degrade gracefully.

Acceptance: the loop converges (median rounds down, no unbounded loops);
regressions are caught and repaired without human input.

### Phase 3 — Writer quality and durable-id ops (about 2-3 weeks)

- Editor builder additions: merge-paste, `ClearDirectFormatting`, full style
  authoring, table column widths, header layout.
- Durable-id patch model `Apply(ops[])` with a single undo transaction; ops bind
  to the same ids as feedback.

Acceptance: styles are authoritative end to end with no conform step; live
editing reaches OOXML-grade structure deterministically.

### Phase 4 — UX and hardening (about 2 weeks)

- "Document health" panel: findings, severity, jump-to-node, before/after summary.
- Corpus and CI: golden documents, regression findings, layout-diff stability;
  align schema majors with this repository.

Acceptance: pass rate and quality metrics are stable across releases; a user
visible health report appears on every build.

## Metrics

- **Quality:** blocking findings per generated document; task success rate on a
  quotation and report corpus.
- **Efficiency:** tool rounds, tokens, and wall-clock per task.
- **Reliability:** diff stability across identical runs (determinism);
  false-positive rate per finding code.
- **Experience:** time to a first good document; share of documents finished with
  zero blocking findings.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Engine layout serialization is non-trivial | Phase 0 delivers value with no engine changes; Phase 1 is additive |
| Verification engine differs from the rendering engine | Emit the snapshot from the same engine that renders |
| Snapshot cost on large documents | Incremental and delta snapshots, bounded coverage, top-N findings |
| False positives | Severity, evidence, and honest coverage; findings advisory unless blocking |
| Id instability across structural edits | Prefer `w14:paraId`, else content-hash paths; phase 3 op ids |
| Regression in the upstream editor | Everything additive: an extension module plus a new plugin method |

## Relationship to this repository

- Adopt this repository's **contract shape** (durable ids, coverage, canonical
  JSON, diff) as the north star for the agent feedback IR.
- Implement the live producer in the editor's own runtime; keep this Rust crate
  as the external and CI validator plus future engine-independence check.
- Feedback and this IR should agree on `(part, identity, id)` so tooling can
  cross-check, and schema versions can be aligned over time.

## First week (Phase 0, concrete)

1. Add `docs/agent-doc-snapshot.md` defining the contract and finding codes.
2. Implement `getAgentSnapshot()` (structure, provenance, findings) and expose
   `get_document_feedback`.
3. Make the review step findings-driven; delete the conform and recolor hacks;
   feed the verify workflow into the active instructions.
4. Add a corpus test that replays known documents and asserts findings.
5. Add a small "Document health" list in the panel for visibility.
