# Workon

Workon is a CLI for creating agent-ready context workspaces.

Primary command: `wo`.

AI agents are replaceable. Context is not.

## Vision

Engineers should own the context that makes AI useful: goals, decisions, code intent, sources, workflows, and project memory.

Workon sits above agents and tools. It turns messy engineering intent into a durable workspace that Claude, Codex, Cursor, or future agents can use.

Workon prepares context. Agents use it. Engineers control it.

## Model

```text
intent -> work folder -> agent files -> agent session -> preserved context
```

Commands are shortcuts. Natural text starts work. The engineer selects the intent profile.

## Concepts

**Work**  
A started unit of engineering intent. Each Work gets a folder:

```text
.workon/work/<title>/
```

The folder holds agent files, notes, outputs, repos, evidence, and artifacts. Work can be switched, resumed, changed, and closed.

**Intent**  
A dynamic instruction profile for a kind of Work: investigate, review, address comments, design, brainstorm.

Intent adds weight to generated agent instructions. It tells the agent which skills, MCPs, sources, and output style to prefer. Weight means preference, not enforcement. Intent can change as the work changes.

**Skills and MCPs**  
Engineers configure skills and MCPs globally. Each Work can emphasize a subset through its current intent. The engineer can add, remove, or change that weight at any time.

**Agent Files**  
Workon writes instruction files into the Work folder.

`AGENTS.md` is the portable source. Agent-specific files like `CLAUDE.md` are projections.

They include the goal, current intent, preferred skills, preferred MCPs, attached repos, and orchestration instructions.

**Context**  
The current shape of the Work: intent, skills, MCPs, repos, notes, decisions, evidence, and outputs.

```sh
wo ctx
wo context
```

This is the dynamic surface for changing intent reinforcing via AGENTS.md/CLAUDE.md some skills, MCPs, also for changing repos.

Status: TBD. The exact behavior is still being shaped.

## First Scenario: Investigate

```sh
wo "As SE, I want to answer a technical question for my manager that needs investigation across one or more repositories."
```

Expected flow:

```text
intent picker opened
intent selected: investigate
work created: <ai-generated-title (using claude or codex cli to return structured output)>
folder created: .workon/work/<title>/
cwd changed: .workon/work/<title>/
created: AGENTS.md
created: CLAUDE.md
goal written: original input
instructions written: investigate, collect evidence, cite sources, keep caveats visible
workon finished
```

To attach repos or change context:

```sh
wo ctx
```

Expected flow:

```text
context editor opened: intent, skills, MCPs, repos
repos selected: repo-a, repo-b
folder created: repos/
cloned: repos/repo-a
cloned: repos/repo-b
updated: AGENTS.md
updated: CLAUDE.md
```

The engineer can now launch Claude, Codex, or Cursor from the Work folder with context already injected.

## CLI Reference

### `wo`

Open the Work list.

```sh
wo
```

Use this to switch into existing Work without remembering exact names.

### `wo "<intent text>"`

Create a Work from natural language.

```sh
wo "Answer a technical question for my manager across repos"
```

Workon opens the intent picker, creates a Work folder, writes agent files, and makes the context ready for Claude, Codex, Cursor, or another motor.

### `wo <work>`

Open the best matching Work.

```sh
wo billing
```

Use this as the fast path when the Work already exists.

### `wo ctx`

Open the context surface for the current Work.

```sh
wo ctx
wo context
```

Alias: `wo context`.

This is where intent, skills, MCPs, and repos can be added, removed, or changed.

Status: TBD. The exact interface is still being shaped.

## Principles

- Context-first
- Agent-agnostic
- Tool-agnostic
- Repo-native
- Dynamic by default
- Pluggable by default

## Non-Goals

- Not an AI IDE
- Not another chat app
- Not the agent

Workon prepares the context agents need to work well.
