# Workon

Workon is a CLI and compact TUI for creating agent-ready context workspaces.

Primary command: `wo`.

AI agents are replaceable. Context is not.

Workon switches folders through a small shell function. No subshell.

By default, Workon stores Work in `~/.workon`. Set `WORKON_ROOT=/path/to/root` to use another root.

Common development commands:

```sh
just verify
just install-dev-shell
# restart the shell, or source the script printed by the command
just wo --intent investigate "Answer a technical question for my manager across repos"
just smoke
```

Production shell setup:

```sh
wo install-shell
# restart the shell, or source the script printed by the command
```

## Vision

Engineers should own the context that makes AI useful: goals, decisions, code intent, sources, workflows, and project memory.

Workon sits above agents and tools. It turns messy engineering intent into a durable workspace that Claude, Codex, Cursor, or future agents can use.

Workon prepares context. Agents use it. Engineers control it.

## Model

```text
intent -> work folder -> agent files -> agent session -> preserved context
```

Commands are shortcuts. Natural text starts Work. The engineer selects the intent profile.

## Concepts

**Work**  
A started unit of engineering intent. Each Work gets a folder:

```text
~/.workon/work/<title>/
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

They include the goal, current intent, preferred skills, preferred MCPs, attached repos, evidence hygiene, and next steps.

**Context**  
The current shape of the Work: intent, skills, MCPs, repos, notes, decisions, evidence, and outputs.

```sh
wo ctx
wo context
```

This is intended to become the context surface for changing intent, skills, MCPs, and repos.

Status: placeholder. The command exists, but the editor behavior is not implemented yet.

## First Scenario: Investigate

```sh
wo --intent investigate "As SE, I want to answer a technical question for my manager that needs investigation across one or more repositories."
```

Current flow:

```text
work created: <generated-title>
intent: investigate
slug: <generated-slug>
folder created: ~/.workon/work/<title>/
files: AGENTS.md, CLAUDE.md
next: work from the folder
```

Planned context flow:

```sh
wo ctx
```

Future shape:

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

Open the compact Work list TUI.

```sh
wo
```

Use this to switch into existing Work without remembering exact names. The TUI is an inline command panel, not a full-screen app.

### `wo list`

Print active Work directly in the terminal.

```sh
wo list
```

The list includes each Work title, intent, slug, and folder.

### `wo install-shell`

Install the shell function that lets `wo` change the current directory.

```sh
wo install-shell
```

Local development equivalent:

```sh
just install-dev-shell
```

### `wo "<intent text>"`

Create a Work from natural language.

```sh
wo "Answer a technical question for my manager across repos"
wo --intent investigate "Answer a technical question for my manager across repos"
```

If the text does not match existing Work, Workon starts creation. In a terminal it asks for intent; in scripts use `--intent`.

Workon creates a Work folder, writes agent files, and switches the current shell there.

### `wo <work>`

Open the best matching Work.

```sh
wo billing
```

Use this as the fast path when the Work already exists.

### `wo archive <work>`

Archive the best matching active Work.

```sh
wo archive billing
```

Workon moves the Work folder from `~/.workon/work/` to `~/.workon/archive/`.
Archived Works no longer appear in `wo` and cannot be opened by normal Work queries.

### `wo ctx`

Show the current status of the context surface.

```sh
wo ctx
wo context
```

Alias: `wo context`.

This is where intent, skills, MCPs, and repos are expected to be added, removed, or changed later.

Status: placeholder. The command is wired through the command layer, but the editor is not implemented yet.

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
