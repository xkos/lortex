---
name: git-commit-summary
description: Analyze git working tree changes, group them by logical concern, generate concise commit messages, and commit. Use when the user asks to commit, generate commit messages, summarize and commit changes, or submit git changes. Optimized for small/fast models.
---

# Git Commit Summary

Analyze uncommitted changes, group by logical concern, generate commit messages, and commit.

## Constraints

- NEVER update git config
- NEVER run destructive git commands (push --force, hard reset, etc.)
- NEVER skip hooks (--no-verify)
- NEVER commit files that likely contain secrets (.env, credentials, keys)
- NEVER push unless the user explicitly asks
- Use HEREDOC for commit messages to preserve formatting
- NEVER add `--trailer` flags (e.g. `--trailer "Made-with: Cursor"`)

## Workflow

### Step 1: Gather State (run in parallel)

```bash
git status
git diff            # unstaged changes
git diff --cached   # staged changes
git log --oneline -5
```

### Step 2: Analyze and Group Changes

Read the diffs carefully. Group files into **logical commits** by concern:

- Files changed for the same feature/fix/refactor → one commit
- Unrelated changes → separate commits
- New untracked files → check if they belong to a group or are standalone

Decision rules:
- **1 concern** → 1 commit
- **N concerns** → N commits, ordered by dependency (infrastructure first, features second)
- If unclear whether to split, prefer splitting

### Step 3: Generate Commit Messages

Format — follow existing repo style from `git log`. If no clear style, use:

```
type(scope): short summary in imperative mood

Optional body explaining WHY, not WHAT. Keep to 1-2 sentences.
```

Common types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`, `perf`

Rules:
- Subject line max 72 chars
- Use imperative mood ("add" not "added", "fix" not "fixed")
- Focus on WHY/intent, not restating the diff
- Chinese commit messages are acceptable if the repo uses them

### Step 4: Stage and Commit

For each logical group, run sequentially:

```bash
git add <files-for-this-group>
git commit -m "$(cat <<'EOF'
type(scope): summary

Optional body.

EOF
)"
```

### Step 5: Verify

```bash
git status
git log --oneline -N  # N = number of new commits
```

Confirm working tree is clean (or only expected files remain).

## Examples

**Single concern — all files related:**

```bash
git add services/vectordb/client.go services/vectordb/types.go services/cron/sync.go
git commit -m "$(cat <<'EOF'
refactor: Milvus 同步改为基于 hash 的增量模式

通过 source_text_hash 对比仅同步变更和新增的角色向量，
避免每次全量 upsert 的性能开销。

EOF
)"
```

**Multiple concerns — split into separate commits:**

```bash
# Commit 1: infrastructure change
git add services/cron/stats_aggregate.go
git commit -m "$(cat <<'EOF'
refactor: 角色统计聚合改为分批处理，降低内存峰值

EOF
)"

# Commit 2: new feature depending on commit 1
git add controllers/manager/task.go router/http.go middlewares/auth.go
git commit -m "$(cat <<'EOF'
feat: 新增 manager 手动触发定时任务接口

EOF
)"
```

## Edge Cases

- **No changes**: Report "nothing to commit" — do not create empty commits
- **Only staged changes**: Commit what's staged, don't touch unstaged files
- **Mixed staged + unstaged**: Ask user whether to include unstaged changes or commit only staged
- **Secrets detected** (.env, credentials, tokens in code): Warn user, do not stage
