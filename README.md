# Workon

Workon is a CLI and compact TUI for creating agent-ready context workspaces.

Primary command: `wo`.

AI agents are replaceable. Context is not.

Workon switches folders through a small shell function.

By default, Workon stores Work in `~/.workon`. Set `WORKON_ROOT=/path/to/root` to use another root.

Common development commands:

```sh
just verify
just install-dev-shell
# restart the shell, or source the script printed by the command
wo --intent investigate "Trace a failing release check across repos"
just smoke
```

Homebrew install:

```sh
brew install philipesteiff/tap/workon && wo install-shell
# restart the shell, or source the script printed by the command
```

## Vision

Engineers should own the context that makes AI useful: goals, decisions, code intent, sources, workflows, and project memory.

Workon sits above agents and tools. It turns messy engineering intent into a durable workspace that Claude, Codex, Cursor, or future agents can use.

Workon prepares context. Agents use it. Engineers control it.

## Concepts

**Work**  
A started unit of engineering intent. Each Work gets a folder:

```text
~/.workon/work/<title>/
```

The folder holds agent files, notes, outputs, repos, evidence, and artifacts. 

Work can be switched, changed, and archived.

**Intent**  
A dynamic instruction profile for a kind of Work: investigate, review, address comments, design, brainstorm etc.

Intent adds weight to generated agent instructions. It tells the agent which skills, MCPs, sources, and output style to prefer. Weight means preference, not enforcement. Intent can change as the work changes.

**Skills and MCPs**  
Engineers configure skills and MCPs globally. Each Work can emphasize a subset through its current intent. The engineer can add, remove, or change that weight at any time.

**Agent Files**  
Workon writes instruction files into the Work folder.

`AGENTS.md` is the portable source. Agent-specific files like `CLAUDE.md` are projections.

They include the goal, current intent, preferred skills, preferred MCPs, attached repos, evidence hygiene, and next steps.

The engineer can now launch Claude, Codex, or Cursor from the Work folder with context already injected.

## CLI Reference

### `wo`

Open the compact Work list TUI.

```sh
wo
```

Use this to switch into existing Work without remembering exact names. 

Repository context from the TUI:

```text
/r       open repository context for the highlighted Work
/i       open intent context for the highlighted Work
tab      switch between repository sources, creation path, and selected repos
type     filter repositories
space    select or remove one or more repositories
enter    apply pending add/link/remove changes
esc      return to the Work queue
```

The left panel lists repositories from the active `gh` account and local repositories discovered in configured repo workspaces. Selecting a GitHub repo creates it in the selected repo workspace and links it. Selecting a local repo links that existing checkout directly. The right panel lists repositories selected for the highlighted Work, including pending additions and removals. Loading the view shows a loader while repositories are discovered. Applying changes shows per-repo progress and an operation log.

Intent context from the TUI:

```text
/i       open intent context for the highlighted Work
type     filter reusable intent profiles
up/down  move through matching intents
enter    switch the highlighted Work to the selected intent
esc      return to the Work queue
```

The intent view lists built-in and custom reusable intent profiles. Switching updates the Work metadata and rewrites `AGENTS.md` and `CLAUDE.md`.

### `wo list`

Print active Work directly in the terminal.

```sh
wo list
```

The list prints a compact table with each Work title, intent, slug, path, and goal.

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
wo "Trace a failing release check across repos"
wo --intent investigate "Trace a failing release check across repos"
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

### `wo intent`

Define and switch reusable Work intentions.

```sh
wo intent list
wo intent show investigate
wo intent switch billing review-pr
wo intent duplicate investigate debug-production --name "Debug Production"
wo intent new debug-production --name "Debug Production" --summary "Diagnose production behavior from evidence."
wo intent edit debug-production --instruction "Reproduce before changing code."
wo intent archive debug-production
```

Built-in intents are read-only. Custom intents are global reusable profiles. Switching a Work intent updates `workon.meta` and rewrites agent files.

### `wo repos`

Link repositories to a Work.

Prerequisites:

- `gh` installed and authenticated
- `git` installed
- at least one repo workspace configured for GitHub repo creation

List attached repositories:

```sh
wo repos list billing
```

Configure repo workspaces:

```sh
wo repos workspace add ~/Projects/worktrees ~/Code/client-worktrees
wo repos workspace list
wo repos workspace remove ~/Projects/worktrees
```

Repo workspaces are user-approved folders Workon may scan and create repos in. Workon does not create hidden real checkouts under `~/.workon`; it only creates inside configured repo workspaces.

Discover existing worktrees/checkouts in configured repo workspaces:

```sh
wo repos discover billing
```

Link an existing checkout or worktree:

```sh
wo repos link billing ~/Projects/worktrees/GalleryApp.git.feature-auth
```

Add one or more repositories:

```sh
wo repos add billing openai/workon
wo repos add billing openai/workon openai/another-repo
wo repos add --workspace ~/Projects/worktrees billing openai/workon
```

Remove one or more repositories:

```sh
wo repos remove billing openai/workon
```

Add behavior:

- caches the GitHub repo as a bare repo under Workon storage
- creates the real worktree under a configured repo workspace
- creates an initial branch named `workon/<work-slug>`
- exposes the repo at `<work>/repos/<repo>` with a symlink
- writes durable fallback/link data to `workon.repos.json`
- rewrites `AGENTS.md` and `CLAUDE.md` with the attached repo list

Remove behavior:

- removes the Workon symlink from `<work>/repos/`
- keeps the underlying checkout, worktree, branch, and repo cache
- updates `workon.repos.json`, `AGENTS.md`, and `CLAUDE.md`

`wo repos list` and the TUI reconstruct live repository state from `<work>/repos/`.
Branch names and worktree identity are read from local Git state, so user branch
changes are reflected without editing `workon.repos.json`.

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
- Not an terminal app
