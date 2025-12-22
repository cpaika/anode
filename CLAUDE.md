# Claude Code Rules for Anode

## Project Overview

Anode is an S3-compatible distributed object storage system written in Rust, featuring Raft consensus, clustering, and comprehensive deployment options (Docker, Helm, k3d).

## Task Queue System

This project has a task queue system that allows Claude to run autonomously for extended periods.

### Commands

- **`/todo-all`** - Process ALL tasks sequentially without stopping. Uses subagents to keep context clean.
- **`/todo`** - Process ONE task, then stop.

### Task Queue Location

Tasks are stored in `.claude/todo.md` as a markdown checklist:

```markdown
## Tasks

- [ ] Incomplete task 1
- [ ] Incomplete task 2
- [x] Completed task
```

### How to Add Tasks

**Option 1: Edit the file directly**
```bash
# Open .claude/todo.md and add:
- [ ] Your new task description
```

**Option 2: Use the helper script**
```bash
.claude/scripts/todo-add.sh "Your new task description"
```

### Helper Scripts

| Script | Purpose |
|--------|---------|
| `.claude/scripts/todo-get.sh` | Get the next incomplete task |
| `.claude/scripts/todo-complete.sh "task"` | Mark a task as complete |
| `.claude/scripts/todo-count.sh` | Count remaining incomplete tasks |
| `.claude/scripts/todo-add.sh "task"` | Add a new task to the queue |

### How It Works

1. `/todo-all` reads the next incomplete task from `.claude/todo.md`
2. It launches a subagent (via Task tool) to implement the task
3. The subagent does the work, runs tests/builds, commits, and marks the task complete
4. Control returns to the main context with a brief summary
5. Repeat until no tasks remain

**Why subagents?** All verbose output (file edits, test output, build logs) stays in the subagent's context. This preserves the main conversation's token budget, allowing Claude to run for hours.

### Tips for Long Runs

- Keep tasks focused and specific (e.g., "Add test for X" not "Improve testing")
- You can add/reorder tasks in `.claude/todo.md` while Claude works
- Monitor progress - Claude reports after each task completion
- If a task fails, Claude will report why and continue to the next task

## Build Commands

```bash
cargo build                    # Build all crates
cargo test                     # Run all tests
cargo clippy                   # Run linter
cargo fmt                      # Format code
```

## Project Structure

- `crates/anode/` - Main server binary
- `crates/anode-s3/` - S3 API compatibility layer
- `crates/anode-storage/` - Storage engine
- `crates/anode-raft/` - Raft consensus implementation
- `crates/anode-protocol/` - Protocol definitions (protobuf)
- `crates/anode-client/` - Client library
- `crates/anode-parquet/` - Parquet file support
- `tests/` - Integration, chaos, and correctness tests
- `deploy/` - Docker, Helm, and k3d deployment configs
