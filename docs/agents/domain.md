# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root, or
- **`CONTEXT-MAP.md`** at the parent workspace root (`../../CONTEXT-MAP.md`) when operating from the meta-workspace: it points at one `CONTEXT.md` per context. Read each one relevant to the topic.
- **`docs/adr/`**: read ADRs that touch the area you're about to work in.

If any of these files don't exist, **proceed silently**. Don't flag their absence; don't suggest creating them upfront. The `/domain-modeling` skill (reached via `/grill-with-docs` and `/improve-codebase-architecture`) creates them lazily when terms or decisions actually get resolved.

## File structure

Single-context repo (most repos):

```
/
├── CONTEXT.md
├── docs/adr/
│   └── 0001-shared-host-seam-in-bar-core.md
└── desktop/
```

Multi-context meta-workspace (presence of `CONTEXT-MAP.md` at the parent workspace root):

```
/
├── CONTEXT-MAP.md
├── CONTEXT.md
├── docs/adr/                          ← workspace-wide decisions
└── repositories/
    ├── agent-skills/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← context-specific decisions
    └── t3code-bar/
        ├── CONTEXT.md
        └── docs/adr/                  ← context-specific decisions
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `CONTEXT.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal: either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/domain-modeling`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders), but worth reopening because…_
