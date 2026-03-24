---
name: git-workflow
description: Agent-Driven GitHub Flow for iteration branch management, PR submission, release workflow, and cross-session handoff. Use when creating branches, submitting PRs, releasing versions, switching branches, or starting a new session.
---

# Git Workflow

Agent-Driven GitHub Flow：main 为唯一开发主线，迭代在独立分支上进行，通过 PR 合并。上架发布时启用 Release 分支。

适用于单人 + 多 AI Agent 并行开发的场景。

## 分支策略

| 分支 | 用途 | 来源 | 合并目标 | 生命周期 |
|---|---|---|---|---|
| `main` | 开发主线，最新代码 | — | — | 永久 |
| `iter/NNN-xxx` | 迭代开发分支 | main | main（PR） | 合并后删除 |
| `release/X.Y` | 发布准备 + bugfix | main（里程碑节点） | 打 tag，不合并 | 发布周期内保留 |

## 迭代分支生命周期

### 1. 创建分支

**始终从 main 创建**，不从其他迭代分支创建：

```bash
git checkout main
git checkout -b iter/NNN-xxx
```

前提：上一迭代的 PR 已合并。如果未合并，先完成上一迭代的 PR 流程。

### 2. 开发过程

- 每完成一个子任务（T1、T2...），做一次提交
- Commit message 格式由 `git-commit-summary` skill 处理
- 不要把整个迭代积攒成一个大提交
- 定期同步 main（当其他迭代的 PR 合并后）：

```bash
git merge main
```

### 3. 提交 PR

迭代结束、测试门禁通过后，提交 PR：

```bash
git push -u origin iter/NNN-xxx
gh pr create --title "iter/NNN: 标题" --body "$(cat <<'EOF'
## 迭代目标
（一句话描述）

## 完成内容
- T1: ...
- T2: ...

## 测试结果
- `cargo test`: ✅ 全量通过
- `flutter test`: ✅ 全量通过

## Checklist
（链接到 docs/ai2ai/checklist.md 或内联）

EOF
)"
```

### 4. Review + 合并

- 人审核 PR diff + checklist 验收
- 通过后合并（Merge commit，保留迭代历史）
- 合并后删除远程分支

### 5. 其他进行中的分支同步

PR 合并后，其他正在开发的迭代分支需要同步 main：

```bash
git checkout iter/其他迭代
git merge main
```

冲突在自己分支上解决，不影响 main。

## 并行开发规则

多个 AI Agent 可以同时在不同迭代分支上工作：

```
你（人）
 ├── Session A (Agent) → iter/013-entity-graph
 ├── Session B (Agent) → iter/014-batch-tags
 └── Session C (Agent) → iter/015-search-optimize
```

关键约束：
- 每个 Agent 只在自己的分支上工作
- main 是唯一合并目标，不允许分支间直接合并
- PR 按完成顺序合并，后合并的分支负责解决与先合并分支的冲突
- 合并顺序由人决定（可以优先合并更重要或更稳定的迭代）

## Release 工作流（上架时启用）

当前阶段不需要，上架发布时启用。

### 创建 Release 分支

开发到里程碑，main 上功能就绪：

```bash
git checkout -b release/0.2 main
```

### Release 分支上只做

- Bug 修复
- 版本号更新
- 文档完善
- **不接受新功能**

### 发布

```bash
git tag v0.2.0
# 构建发布包
```

### 热修复

发布后发现严重 bug：

```bash
git checkout release/0.2
# 修复 bug
git tag v0.2.1
# cherry-pick 回 main
git checkout main
git cherry-pick <commit>
```

## 切换分支安全检查

切换分支前必须确保工作区干净：

```bash
git status
```

如果有未提交的改动：
- 属于当前迭代的改动 → 先提交
- 临时性的改动 → `git stash`
- 不需要的改动 → `git checkout -- <file>`

**禁止在有未提交改动时切换分支。**

## 跨 Session 衔接

新 session 开始时，必须：

1. `git branch -a` — 确认当前分支和所有分支
2. `git status` — 确认工作区状态
3. `git log --oneline -5` — 确认最近提交
4. 读 `docs/ai2ai/status.md` — 确认迭代状态

基于以上信息判断应该在哪个分支上工作，而不是假设应该从 main 开始。
