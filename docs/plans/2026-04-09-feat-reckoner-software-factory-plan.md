---
title: "feat: Reckoner — Software Factory Wrapping PAS"
type: feat
status: active
date: 2026-04-09
deepened: 2026-04-09
---

## Enhancement Summary

**Deepened on:** 2026-04-09
**Research agents used:** 10 (containers/OrbStack, SQLite patterns, observability, git automation, linter framework, container security x2, macOS scheduling, architecture review, performance review)

### Key Improvements from Research
1. **Switch from sqlx to rusqlite** — sync API matches SQLite reality, faster CLI startup, no pool needed
2. **Start with JSON log files, not Loki** — zero infra, hl CLI for queries, promote to Loki/VictoriaLogs later
3. **Bare clone + git worktree per task** — solves concurrent same-repo, improves virtiofs perf, shared object store
4. **Shell out to git/gh** — not git2/gitoxide; auth is free, containers already have git
5. **Docker Compose secrets** — not env vars; prevents agent from reading API keys via env
6. **Network egress filtering** — Squid proxy allowlist for LLM APIs + GitHub only
7. **launchd LaunchAgents** — sleep-aware, concurrent-run-safe, plist crate for generation
8. **JSON-Lines linter output** — token-efficient for agents, with natural-language remediation field
9. **PAS needs --output-result** — structured result contract across container boundary
10. **outcome_json out of SQLite** — prevents DB bloat, keeps under 100MB target

### Critical Security Findings
- Use repo-scoped fine-grained PATs, not SSH keys (limits blast radius)
- Drop ALL container capabilities + no-new-privileges
- Read-only root filesystem + tmpfs scratch
- Separate .git/ from working tree bind mount
- Cap background agents with approval gates

---

# Reckoner — Software Factory Wrapping PAS

## Overview

Reckoner (reck) is a software factory that accepts git repository URLs, provisions isolated OrbStack containers per task, runs PAS pipelines inside them, and PRs the results back. It implements the harness engineering environment layer — container lifecycle, architectural linting, doc freshness enforcement, observability, and background entropy cleanup — that PAS intentionally does not own.

PAS is the pipeline execution engine. Reckoner is everything else.

**Name origin:** Pascal's calculator was called the Reckoner. Reckoner is the machine; PAS is the engine inside it.

## Problem Statement / Motivation

PAS can run pipelines, but it has no opinion about:
- Where code lives or how it gets there (git clone, branching, PRs)
- Isolation between concurrent tasks (containers, resource limits)
- Architectural enforcement on the code pipelines produce
- Observability of the running application being built
- Recurring maintenance (doc rot, entropy accumulation, quality drift)
- The full lifecycle: task intake, build, verify, PR, merge

OpenAI's harness engineering post demonstrates that the environment around an agent execution engine matters more than the engine itself for sustained quality. Reckoner fills this gap.

## Proposed Solution

A new Rust CLI application (reck) in a separate repository that:

1. **Registers repos** via git URL — bare clones with treeless filter for speed
2. **Provisions containers** per task via OrbStack — each task gets a fully isolated environment with its own git worktree, browser, and logs
3. **Runs PAS inside containers** — pas run pipeline.dot runs inside the container, not on the host
4. **PRs results back** — uses gh inside the container to open PRs against the source repo
5. **Preserves logs/observability** — structured JSON log files on persistent volumes, queryable via hl CLI, promotable to Loki/Grafana later
6. **Enforces architecture** — runs linters with agent-readable remediation messages on every task
7. **Fights entropy** — scheduled background PAS pipelines via macOS launchd for doc-gardening, quality scanning, and pattern drift detection

## Technical Approach

### Architecture

The system is split into two crates to start, with internal trait boundaries designed for future extraction:

- **reckoner-cli** — clap-based CLI binary (reck)
- **reckoner-core** — engine modules behind trait interfaces:
  - repo_manager: git clone/fetch/worktree/PR
  - container_mgr: OrbStack create/start/stop/rm (ContainerRuntime + ContainerExecutor traits)
  - task_runner: provision, pas run, collect
  - toolchain: per-repo code quality tools (ruff, ty, biome, etc.)
  - lint_runner: Reckoner's architectural linters (LintRule trait)
  - obs_collector: gather logs post-run
- **State**: SQLite via rusqlite (sync, no pool)
- **Logs**: Structured JSONL files on persistent volumes

The ContainerRuntime trait is split from ContainerExecutor to enable future remote execution (SSH-based executor + local runtime):

```rust
#[async_trait]
trait ContainerRuntime: Send + Sync {
    async fn create(&self, spec: ContainerSpec) -> Result<ContainerId>;
    async fn start(&self, id: &ContainerId) -> Result<()>;
    async fn stop(&self, id: &ContainerId) -> Result<()>;
    async fn remove(&self, id: &ContainerId) -> Result<()>;
}

#[async_trait]
trait ContainerExecutor: Send + Sync {
    async fn exec(&self, id: &ContainerId, cmd: &[&str]) -> Result<ExecResult>;
    async fn exec_stream(&self, id: &ContainerId, cmd: &[&str]) -> Result<ExecStream>;
}
```

**PAS integration contract:** PAS needs a --output-result path flag added to pas run that writes PipelineResult as JSON. Reckoner reads this from the volume mount. Without this, Reckoner must infer success/failure from exit codes, which is fragile.

### Data Model

```sql
CREATE TABLE repos (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    url            TEXT NOT NULL UNIQUE,
    name           TEXT NOT NULL,
    local_path     TEXT NOT NULL,
    default_branch TEXT NOT NULL DEFAULT 'main',
    last_synced    TEXT,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE tasks (
    id              TEXT PRIMARY KEY,
    repo_id         INTEGER NOT NULL REFERENCES repos(id),
    schedule_id     INTEGER REFERENCES schedules(id),
    prompt          TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','provisioning','running',
                          'linting','pr_open','done','failed')),
    container_id    TEXT,
    branch_name     TEXT,
    pr_url          TEXT,
    pipeline_path   TEXT,
    total_cost_usd  REAL DEFAULT 0.0,
    attempt_count   INTEGER DEFAULT 1,
    failed_stage    TEXT,
    error_message   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    started_at      TEXT,
    completed_at    TEXT
);

CREATE TABLE task_transitions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id     TEXT NOT NULL REFERENCES tasks(id),
    from_status TEXT NOT NULL,
    to_status   TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    detail      TEXT
);

CREATE TABLE runs (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id       TEXT NOT NULL REFERENCES tasks(id),
    pipeline_path TEXT NOT NULL,
    status        TEXT NOT NULL,
    cost_usd      REAL DEFAULT 0.0,
    duration_secs INTEGER,
    logs_path     TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE lint_results (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id       INTEGER NOT NULL REFERENCES repos(id),
    task_id       TEXT REFERENCES tasks(id),
    lint_run_id   TEXT NOT NULL,
    rule_name     TEXT NOT NULL,
    status        TEXT NOT NULL,
    message       TEXT,
    remediation   TEXT,
    file_path     TEXT,
    line_number   INTEGER,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE schedules (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id       INTEGER NOT NULL REFERENCES repos(id),
    name          TEXT NOT NULL,
    pipeline_path TEXT NOT NULL,
    cron_expr     TEXT NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 1,
    last_run      TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_repo_id ON tasks(repo_id);
CREATE INDEX idx_tasks_schedule_id ON tasks(schedule_id);
CREATE INDEX idx_runs_task_id ON runs(task_id);
CREATE INDEX idx_lint_results_repo_task ON lint_results(repo_id, task_id);
CREATE INDEX idx_schedules_enabled ON schedules(enabled);
```

**SQLite configuration (set once on connection open):**
- journal_mode=WAL, synchronous=NORMAL, busy_timeout=5000
- foreign_keys=ON, cache_size=-64000, mmap_size=256MB, temp_store=MEMORY
- Migrations via rusqlite_migration (PRAGMA user_version, no migration tables)
- Single Connection per invocation, no pool
- Long-running tasks: open/close connections around each state transition
- File permissions: 0600 on reckoner.db and backups

**outcome_json stored on DISK** at logs_path/outcome.json, NOT in SQLite. This is critical for keeping the DB under 100MB.

### Container Lifecycle

Each task gets a container with a dedicated git worktree:

```
reck task my-app "add user auth"

1. PROVISION
   - git fetch origin in ~/.reckoner/repos/my-app.git (bare)
   - git worktree add ~/.reckoner/worktrees/reck-{id} -b reckoner/feat/reck-{id}
   - docker create from reckoner-base image:
     - read_only: true (root filesystem)
     - tmpfs: /tmp (512m), /home/agent/.cache (1g)
     - bind mount: worktree to /workspace (rw)
     - bind mount: logs volume (rw)
     - secrets: /run/secrets/anthropic_api_key, gh_token
     - cap_drop: ALL, security_opt: no-new-privileges
     - network: reckoner-net (egress filtered via Squid proxy)
     - resources: --memory 4g --cpus 4 --pids-limit 512
     - user: 1000:1000
   - docker start

2. EXECUTE
   - Generate pipeline (pas generate) OR use provided .dot
   - pas run pipeline.dot --workdir /workspace --output-result /workspace/.reckoner/result.json
   - Stream progress via docker logs
   - Read result.json from volume mount

3. LINT
   - Run architectural linters inside container
   - If failures: lint-fix loop (max 3 iterations)
     - Feed remediation to PAS fixup pipeline, re-lint
   - Record lint results in SQLite

4. PR
   - git add + commit inside container
   - git push origin reckoner/feat/reck-{id}
   - gh pr create with structured body template
   - Record PR URL in SQLite

5. COLLECT
   - Logs already on volume mount (structured JSONL)
   - Update SQLite (status=done, cost, PR URL)
   - docker stop + docker rm
   - git worktree remove

Done. Container gone, worktree gone, logs preserved.
```

### Git + PR Integration

**Shell out to git CLI** (not git2/gitoxide). Auth is free via credential helpers. Containers already have git. No C deps, no build complexity. Set GIT_TERMINAL_PROMPT=0 to prevent blocking.

**Shell out to gh CLI** for PR lifecycle. Handles auth, JSON output, auto-detection of repo/branch.

**Credentials inside containers:** GH_TOKEN via Docker secrets + gh auth setup-git in entrypoint. Single token works for both git push and gh pr create.

**Branch naming:** reckoner/type/id-slug (e.g., reckoner/feat/reck-42-add-user-auth). Namespaced, regex-enforceable. Auto-delete on PR merge.

**Merge conflicts:** Preemptive rebase before PR. If rebase fails, abandon-and-retry (re-checkout from current main, re-run pipeline).

**Bare clone + worktree pattern:**
- reck add does git clone --bare --filter=blob:none (treeless, fast)
- Each task gets git worktree add from the bare clone
- All worktrees share one object store (2GB repo stored once)
- Git prevents same branch in two worktrees (built-in safety)
- Worktree has native filesystem speed inside container

### Toolchain (Per-Repo Code Quality)

Reckoner distinguishes two kinds of code checking:

1. **Toolchain** — language-specific code quality tools the *repo* needs (linters, type checkers, formatters). These are the tools you'd run in CI. Configurable per-repo.
2. **Architectural linters** — Reckoner's own structural checks (file size, dependency direction, doc freshness). These enforce harness-level invariants.

Both run in the LINT phase. Toolchain runs first (fix code quality), then architectural linters (fix structural issues).

**Toolchain is pluggable.** Tools are configured per-repo in `.reckoner/toolchain.toml`, falling back to auto-detection, falling back to global defaults. The framework doesn't hardcode any specific tool — it invokes whatever command is configured and parses the exit code.

**Per-repo config** (`.reckoner/toolchain.toml` in the repo root):
```toml
[python]
lint = "ruff check --fix ."
format = "ruff format ."
typecheck = "ty check ."

[typescript]
lint = "biome check --fix ."
format = "biome format --fix ."
typecheck = "biome check ."

[rust]
lint = "cargo clippy --workspace -- -D warnings"
format = "cargo fmt --all"
typecheck = "cargo check --workspace"
```

**Global defaults** in `~/.reckoner/config.toml`:
```toml
[toolchain.defaults.python]
lint = "ruff check --fix ."
format = "ruff format ."
typecheck = "ty check ."

[toolchain.defaults.typescript]
lint = "biome check --fix ."
format = "biome format --fix ."
typecheck = "biome check ."

[toolchain.defaults.rust]
lint = "cargo clippy --workspace -- -D warnings"
format = "cargo fmt --all"
typecheck = "cargo check --workspace"
```

**Auto-detection:** If no `.reckoner/toolchain.toml` exists, Reckoner detects languages from file extensions and applies global defaults. `reck add` reports which toolchain it will use and scaffolds a `.reckoner/toolchain.toml` for the user to customize.

**Execution in the LINT phase:**
1. Detect which languages are present in changed files
2. Run `format` commands (auto-fix style issues before anything else)
3. Run `lint` commands (catch code quality issues)
4. Run `typecheck` commands (catch type errors)
5. If any fail: feed errors into the lint-fix loop

**Base image implications:** The base container image includes common runtimes. Language-specific tools are installed per-repo via the container entrypoint or a setup script. For Python repos, the entrypoint runs `uv sync` to install dependencies including ruff/ty. For TypeScript repos, it runs `npm install` or equivalent.

### Architectural Linters (Reckoner's Own)

Separate from the per-repo toolchain, Reckoner runs its own structural linters that enforce harness-level invariants across all repos.

**JSON-Lines output** to stdout. One JSON object per line. Token-efficient for LLM consumption, streaming-friendly, trivial to author in any language.

```json
{
  "rule": "dependency-direction",
  "status": "fail",
  "level": "error",
  "file": "src/service/handler.rs",
  "line": 12,
  "message": "service layer imports from ui layer. Imports must flow downward.",
  "remediation": "Move the shared type to types:: or define a trait in service::. Dependency direction: types, config, repo, service, runtime, ui.",
  "context": {
    "source_layer": "service",
    "target_layer": "ui"
  }
}
```

**Plugin discovery:** Executable directory scan. repo/.reckoner/linters/ then ~/.reckoner/linters/ then built-in. Naming: reck-lint-name. Exit codes: 0=pass, 1=failures, 2=linter error.

**Built-in architectural linters:**

| Linter | What it checks |
|--------|----------------|
| file-size | No source file exceeds N lines (default 500) |
| dependency-direction | Layer imports flow in one direction only (configurable per-repo) |
| doc-freshness | Docs reference code that changed since last doc update |
| agents-md-toc | AGENTS.md exists, under N lines, points to docs/ |
| structured-logging | No raw println/console.log |

### Lint-Fix Loop

Both toolchain and architectural linters feed into the same fix loop (max 3 iterations):

1. Run toolchain (format → lint → typecheck), collect errors
2. Run architectural linters, collect JSON-Lines
3. If no failures: done
4. Construct fixup prompt from all errors + remediation fields
5. Run lightweight PAS fixup pipeline (edit tools only, no shell)
6. Re-run ALL checks (a fix can introduce new violations)
7. Track stuck violations (same rule+file+line across iterations) for human review

**Key principle:** Write error messages for the agent, not a human. The remediation field IS the fix instruction.

### Observability (Tiered)

**Tier 1 (start here): Structured JSON files + hl CLI**
- Containers write JSONL to stdout, Reckoner captures to ~/.reckoner/logs/
- Query with hl (Rust, 2 GiB/s): hl --filter 'task_id=reck-42' logs/
- Zero infrastructure, zero idle cost

**Tier 2 (web UI): SQLite log store + datasette**
- ~10MB idle, SQL querying, web UI

**Tier 3 (scale): VictoriaLogs + Grafana**
- 87% less memory than Loki, auto-indexes all fields, ~120-200MB idle

**Tier 4 (Loki ecosystem): Loki monolithic + Grafana**
- Promtail is EOL (March 2026). Use direct HTTP push or Grafana Alloy.
- 300-500MB idle. Only if you specifically want LogQL.

### Background Agents (macOS launchd)

**launchd LaunchAgents** at ~/Library/LaunchAgents/com.reckoner.name.plist:
- Sleep-aware: fires on wake if missed during sleep
- Concurrent-run-safe: will not start second instance
- ProcessType: Background + LowPriorityIO: true

**Plist generation:** plist crate + custom serde struct. Absolute paths required (no tilde expansion). Explicit PATH in EnvironmentVariables including /opt/homebrew/bin.

**CLI:**
```
reck schedule add --name entropy-gc --pipeline entropy-gc.dot --cron "0 3 * * *"
reck schedule list
reck schedule remove entropy-gc
reck schedule status entropy-gc
reck schedule run entropy-gc
```

**Security for background agents:**
- Read-only tools for scan/analysis phases
- Human approval gate before push/PR
- Lower budget caps than interactive tasks
- Dry-run mode by default

**Failure notifications:** terminal-notifier (Homebrew) with osascript fallback.

### Security Model

**Container hardening:**
- cap_drop: ALL + no-new-privileges
- read_only: true root filesystem + tmpfs scratch
- pids-limit: 512 (fork bomb protection)
- memory: 4g, memory-swap: 4g (no swap, OOM-kill)
- user: 1000:1000 (non-root)
- Never mount Docker socket
- OrbStack VM boundary = two-layer isolation

**Secrets:**
- Docker Compose secrets (file-mounted to /run/secrets/)
- Fine-grained PATs scoped per-repo (contents:write + pull_requests:write only)
- SSH agent socket forwarding if needed (never copy keys)
- Secrets die with container

**Network:**
- Squid forward proxy on host with domain allowlist
- Allow: api.anthropic.com, api.openai.com, github.com, api.github.com, package registries
- Block all other outbound traffic
- --network none for pipeline steps that do not need network

**Filesystem:**
- Worktree bind-mount (rw) — working files only, not .git/
- Log volume (rw) — named Docker volume
- Everything else: do not mount

**Base image:**
- Pinned by SHA256 digest
- Claude Code install script vendored and audited
- All apt packages version-pinned
- reck doctor verifies image digest

### CLI Commands

```
reck add <git-url>                     # Bare treeless clone, register
reck remove <repo-name>               # Unregister
reck list                              # List repos
reck sync <repo-name>                  # git fetch --prune

reck task <repo> "<prompt>"            # Full lifecycle
reck task <repo> --pipeline <.dot>     # Use specific pipeline
reck task <repo> --spec <spec.md>      # Generate pipeline from spec
reck task <repo> --resume <id>         # Resume failed task

reck status                            # All active tasks
reck status <task-id>                  # Detailed + transition history
reck logs <task-id>                    # Logs via hl
reck logs <task-id> --app              # App logs only
reck logs <task-id> --lint             # Lint results

reck lint <repo>                       # Run arch linters
reck lint <repo> --fix                 # Linters + auto-fix loop

reck schedule add/list/remove/status/run

reck infra up/down/status              # Observability stack (Tier 3+)
reck observe                           # Open dashboard

reck config                            # Show config
reck init                              # Interactive setup
reck doctor                            # Health checks
reck doctor --integrity                # SQLite vs filesystem consistency
```

### Configuration

```toml
# ~/.reckoner/config.toml

[general]
repos_dir = "~/.reckoner/repos"
worktrees_dir = "~/.reckoner/worktrees"
logs_dir = "~/.reckoner/logs"
db_path = "~/.reckoner/reckoner.db"

[container]
runtime = "orbstack"
base_image = "reckoner-base:latest"
network = "reckoner-net"
default_memory = "4g"
default_cpus = 4
pids_limit = 512

[observability]
tier = "files"

[git]
auto_pr = true
pr_prefix = "reckoner"
commit_author = "Reckoner <reckoner@local>"

[pas]
binary = "pas"
default_model = "sonnet"
default_max_budget_usd = 10.0
default_max_steps = 200
min_version = "0.5.1"

[toolchain.defaults.python]
lint = "ruff check --fix ."
format = "ruff format ."
typecheck = "ty check ."

[toolchain.defaults.typescript]
lint = "biome check --fix ."
format = "biome format --fix ."
typecheck = "biome check ."

[toolchain.defaults.rust]
lint = "cargo clippy --workspace -- -D warnings"
format = "cargo fmt --all"
typecheck = "cargo check --workspace"

[linters]
enabled = true
fail_on_warning = false
max_fix_iterations = 3

[linters.builtin]
file-size = { enabled = true, max_lines = 500 }
dependency-direction = { enabled = true }
doc-freshness = { enabled = true, max_age_days = 30 }
agents-md-toc = { enabled = true, max_lines = 150 }
structured-logging = { enabled = true }

[security]
egress_proxy = true
egress_allowlist = [
    "api.anthropic.com",
    "api.openai.com",
    "github.com",
    "api.github.com",
]
```

### Dependency Stack

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
rusqlite = { version = "0.39", features = ["bundled"] }
rusqlite_migration = "1"
bollard = "0.20"
toml = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "process"] }
plist = "1"
croner = "2"
dirs = "6"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
anyhow = "1"
thiserror = "2"
regex = "1"
```

## Implementation Phases

### Phase 1: Foundation (CLI + Repo + Config)
- Rust workspace: reckoner-cli, reckoner-core
- CLI skeleton: add, list, remove, sync, config, init
- SQLite with rusqlite + rusqlite_migration
- Config parsing (toml + serde)
- Git: bare treeless clone, fetch, worktree add/remove

### Phase 2: Container Engine
- bollard for container lifecycle
- Base Dockerfile (pinned, hardened, non-root)
- Docker secrets, egress proxy (Squid), network setup
- reck doctor

### Phase 3: Task Runner
- reck task full lifecycle
- Task state machine with transitions and audit trail
- PAS execution via docker exec + --output-result
- Log collection to JSONL, reck status/logs

### Phase 4: Git + PR Integration
- Worktree per task, branch naming, commit/push/PR
- gh auth setup-git in entrypoint
- Structured PR body template
- Cleanup: worktree remove + branch delete

### Phase 5: Toolchain + Linter Framework
- Pluggable toolchain: per-repo `.reckoner/toolchain.toml` with language auto-detection
- Default presets: ruff+ty (Python), biome (TypeScript), clippy+fmt (Rust)
- Architectural linters: JSON-Lines runner with plugin discovery
- Built-in linters: file-size, dependency-direction, doc-freshness, agents-md-toc
- Lint-fix loop (max 3 iterations) covering both toolchain and architectural linters
- Per-rule config in config.toml, per-repo overrides

### Phase 6: Observability
- Tier 1: JSONL files + hl integration
- Tier 2+: optional SQLite/VictoriaLogs/Loki via reck infra

### Phase 7: Background Agents
- launchd plist generation, schedule CLI
- Restricted tools, approval gates, lower budgets
- Built-in pipelines: entropy-gc, doc-gardening, quality-scan

## Risk Analysis

| Risk | Mitigation |
|------|------------|
| OrbStack API changes | ContainerRuntime trait; bollard uses stable Docker API |
| Container resource exhaustion | Per-container limits, reck status shows usage |
| PAS version mismatch | reck doctor checks version; config pins minimum |
| Git auth inside containers | Scoped fine-grained PATs, Docker secrets |
| Agent reads API keys | Docker secrets (file mount), egress proxy blocks exfil |
| Agent exfiltrates code | Network egress allowlist via Squid proxy |
| Concurrent same-repo tasks | Git worktrees with built-in branch locking |
| SQLite/filesystem desync | reck doctor --integrity |
| Background agent prompt injection | Restricted tools, approval gate before push |
| Base image supply chain | Pin by SHA, vendor install scripts |

## Sources

- [OpenAI Harness Engineering](https://openai.com/index/harness-engineering/)
- [bollard crate](https://crates.io/crates/bollard)
- [rusqlite](https://crates.io/crates/rusqlite)
- [hl JSON log viewer](https://github.com/pamburus/hl)
- [OWASP Docker Security](https://cheatsheetseries.owasp.org/cheatsheets/Docker_Security_Cheat_Sheet.html)
- [BitsAI-Fix: LLM Lint Resolution](https://arxiv.org/html/2508.03487v1)
- [Apple: Creating Launch Agents](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)
- [VictoriaLogs](https://docs.victoriametrics.com/victorialogs/)
- [SkyPilot: SQLite Concurrency](https://blog.skypilot.co/abusing-sqlite-to-handle-concurrency/)
