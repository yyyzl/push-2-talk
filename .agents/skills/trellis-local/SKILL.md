---
name: trellis-local
description: |
  push-2-talk 项目的 Trellis 本地定制记录。
  用于记录当前 Trellis 版本、平台接入和本地运行约束。
---

# Trellis Local - push-2-talk

## 当前版本

- Trellis 版本：0.5.9
- 项目版本记录：`.trellis/.version`
- CLI 校验命令：`trellis --version`

## 平台接入

- 启用平台：Claude Code、Cursor、Codex、共享 `.agents/skills/`
- 共享技能：`.agents/skills/trellis-*/`
- Claude Code：
  - `.claude/skills/trellis-*/`
  - `.claude/agents/trellis-*.md`
  - `.claude/settings.json`
  - `.claude/hooks/`
- Codex：
  - `.codex/config.toml`
  - `.codex/hooks.json`
  - `.codex/agents/trellis-*.toml`
  - `.codex/hooks/inject-workflow-state.py`
  - `.codex/hooks/session-start.py`
  - `.codex/skills/`

## Codex 当前配置

- `.codex/hooks.json` 仅注册 `UserPromptSubmit`
- `UserPromptSubmit` 调用 `.codex/hooks/inject-workflow-state.py`
- Codex hooks 需要用户级 `~/.codex/config.toml` 开启 `[features].hooks = true`
- Codex 0.129+ 还需要在 Codex 内执行 `/hooks` 并批准项目 hook
- `.codex/config.toml` 保留项目级默认配置、multi-agent 设置和本地 MCP server 配置

## Hook 注册概览

- Claude Code：
  - `SessionStart`：`.claude/hooks/session-start.py`
  - `PreToolUse`：`.claude/hooks/inject-subagent-context.py`
  - `UserPromptSubmit`：`.claude/hooks/inject-workflow-state.py`
  - `PreCompact`：`jcodemunch-mcp hook-precompact`
  - `PostToolUse`：`jcodemunch-mcp hook-posttooluse`、GitNexus hook
- Codex：
  - `UserPromptSubmit`：`.codex/hooks/inject-workflow-state.py`
- Cursor：
  - `preToolUse`：`.cursor/hooks/inject-subagent-context.py`
  - `sessionStart`：`.cursor/hooks/session-start.py`
  - `beforeSubmitPrompt`：`.cursor/hooks/inject-workflow-state.py`
  - `beforeShellExecution`：`.cursor/hooks/inject-shell-session-context.py`

## Windows 运行约束

- Python hook 命令统一使用 `uv run --python 3.12 python -X utf8`
- `uv run --python 3.12` 用于固定 Python 运行环境
- `-X utf8` 用于保证 Windows 控制台中文输出稳定
- 重点保持一致的文件：
  - `.claude/settings.json`
  - `.codex/hooks.json`

## 维护约定

- Trellis 模板更新前先运行 `trellis update --dry-run`
- 合并更新时保留项目内 Windows hook 命令前缀
- 合并更新时保留 `.codex/config.toml` 中的项目级 Codex 与 MCP 配置
- `.codex/hooks.json` 保持当前 Codex hook 事件设计
