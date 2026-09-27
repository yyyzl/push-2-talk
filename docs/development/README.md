# 开发文档

这里记录项目已有的技术契约和实现经验，按当前修改内容查阅。

| 文档 | 内容 |
| --- | --- |
| [平台集成审查](platform-integration.md) | Windows/Mac 分支合并决策、验证证据与合入主线前的验收门槛 |
| [ASR 热词编译](asr-hotword-compilation.md) | 各 ASR 提供方的热词载荷与兼容约定 |
| [千问 ASR 模型](qwen-asr-profiles.md) | Audio 3.1、Audio 3.0、Qwen3 的选择、配置迁移与协议配对 |
| [TNL 文本规范化](tnl-normalization.md) | 词典、读音匹配、候选选择与确定性处理规则 |
| [事件契约](event-contracts.md) | Rust 后端与前端之间的事件和载荷 |
| [数据库约定](database-guidelines.md) | 个性化词库的 SQLite 存储、查询与迁移 |
| [资源与错误处理](error-handling.md) | 异步资源释放、并发守卫与完成事件 |
| [副窗口组件](secondary-window-components.md) | 事件监听、隐藏窗口刷新、拖动与状态清理 |
| [Tauri IPC](tauri-ipc.md) | 命令注册、窗口权限与待处理状态生命周期 |

构建、测试命令和项目结构见 [项目约定](../../AGENTS.md)。
