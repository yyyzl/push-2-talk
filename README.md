# PushToTalk - 语音输入助手

<div align="center">

**按住快捷键说话，松开自动转录并插入文本 | AI 智能助手，语音控制一切**

[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-blue.svg)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)

</div>

---

PushToTalk 是一个高性能的桌面语音输入工具，集成了大语言模型（LLM）能力。支持**两种工作模式**：

1. **听写模式**：按住 `Ctrl+Win` 说话，松开后自动转录并插入文本，支持 LLM 智能润色
2. **AI 助手模式**：选中文本后按 `Alt+Space` 说话，用语音命令处理选中的文本；或直接提问获得答案

[使用教程](https://ncn18msloi7t.feishu.cn/wiki/NFM3wAcWNi0IGTkUqkVckxWWntb)

### ✨ 核心特性

#### 双模式工作
- 🎤 **听写模式** - 传统的语音转文字功能
  - **按住模式**：按住快捷键录音，松开停止（传统方式）
  - **松手模式**：按一次 F2 开始录音，再按一次结束（防止误停）
- 🤖 **AI 助手模式** - 语音控制文本处理
  - **无选中文本**：Q&A 模式，提问获得答案
  - **选中文本**：语音命令处理文本（翻译、润色、总结、扩写等）

#### 核心功能
- ⚡ **实时流式转录** - WebSocket 边录边传，极低延迟（< 500ms），松手即出字
- 🧠 **LLM 智能后处理** - 内置"文本润色"、"邮件整理"、"中译英"等预设，支持自定义 Prompt
- ⌨️ **自定义快捷键** - 支持 73 种按键绑定（修饰键、字母、数字、功能键、方向键等）
- 🔄 **多 ASR 引擎** - 支持阿里云 Qwen、豆包 Doubao、SiliconFlow SenseVoice
- 🛡️ **智能兜底** - 主引擎失败时自动切换到备用引擎，并行竞速
- 🎨 **可视化反馈** - 录音状态悬浮窗，实时波形显示，三种视觉状态
- 🔊 **音频反馈** - 录音开始/结束的清脆提示音，盲操也放心
- 📜 **历史记录** - 自动保存转录历史，支持搜索、复制、清空
- 🚀 **系统托盘** - 支持最小化到托盘、开机自启动
- 🔄 **自动更新** - 内置 6 个镜像源，自动检查并安装更新
- 💾 **多配置管理** - 支持保存多套 LLM 预设，通过界面快速切换不同场景
- 📖 **个人词库** - 自定义热词列表，提升专业术语识别准确率
- 🔇 **录音时静音** - 录音时自动静音其他应用，避免干扰
- 🎚️ **VAD 静音检测** - 智能检测语音活动，自动过滤静音片段
- 📈 **AGC 自动增益** - 自动调节音量，优化细微声音场景下的转换效果
- 🖥️ **多屏幕支持** - 悬浮窗自动适配多显示器环境

---

## 🎬 快速开始

### 安装

1. 从 [Releases](https://github.com/your-repo/releases) 下载最新版本的安装包

2. 运行 NSIS 安装程序完成安装

3. 右键点击应用图标，选择"以管理员身份运行"

> ⚠️ **重要**：必须以管理员身份运行才能使用全局快捷键功能

### 配置

### 快捷链接
[API Key申请教学文档](https://ncn18msloi7t.feishu.cn/wiki/ZnBZwSNjpisUdYkKks1cbes8nGb)


#### 1. ASR 配置（至少配置一个）

##### 阿里云 Qwen（推荐）
- 超大量的免费额度，2025 年 3 月前基本用不完
- 支持实时流式和 HTTP 两种模式
- [获取 DashScope API Key](https://bailian.console.aliyun.com/?tab=model#/api-key)

##### 豆包 Doubao（可选）
- 支持实时流式和 HTTP 两种模式
- [录音文件识别大模型-极速版开通](https://console.volcengine.com/ark/region:ark+cn-beijing/tts/recordingRecognition)
- [流式语音识别大模型-小时版开通](https://console.volcengine.com/ark/region:ark+cn-beijing/tts/speechRecognition)
- 注意：App ID 和 Access Token 在网页下方

##### 硅基流动 SenseVoice（可选，免费）
- 免费使用的备用引擎
- 可作为主引擎的智能兜底
- [获取 SiliconFlow API Key](https://cloud.siliconflow.cn/me/account/ak)

#### 2. 快捷键配置（可自定义）

**听写模式**：
- 默认快捷键：`Ctrl + Win`
- 松手模式快捷键：`F2`
- 可自定义为任意组合键（支持 73 种按键）

**AI 助手模式**：
- 默认快捷键：`Alt + Space`
- 可自定义为任意组合键

#### 3. LLM 配置（可选）

##### 听写模式 LLM（文本润色）
- 用于对转录结果进行润色、翻译等后处理
- 推荐使用免费的智谱 GLM-4-Flash
- [获取智谱 API Key](https://docs.bigmodel.cn/cn/guide/models/free/glm-4-flash-250414)
- 可添加多个自定义预设（文本润色、中译英、邮件整理等）

##### AI 助手模式 LLM（必需）
- 用于 AI 助手模式的文本处理和问答
- 支持 OpenAI 兼容接口
- 配置两个系统提示词：
  - **Q&A 提示词**：用于回答问题
  - **文本处理提示词**：用于处理选中的文本

#### 4. 系统设置（可选）

- **关闭时最小化到托盘** - 关闭窗口时保持后台运行
- **开机自启动** - 系统启动时自动运行（需要管理员权限）
- **录音时静音其他应用** - 录音时自动静音其他应用，避免干扰

#### 5. 个人词库（可选）

- **自定义热词** - 添加专业术语、人名、地名等，提升识别准确率
- **支持多个 ASR 引擎** - Qwen（HTTP/实时）和 Doubao（HTTP/实时）均支持词库功能
- **格式要求** - 每行一个词，支持中英文混合

#### 6. 保存并启动

点击"保存配置"并"启动助手"。

---

## 📖 使用指南

### 听写模式

#### 按住模式（传统方式）
1. 将光标定位在任何输入框（微信、Word、VS Code）
2. 按住 `Ctrl` + `Win` 键，听到"滴"声后开始说话
3. 说完松开按键，听到结束提示音
4. 等待处理（悬浮窗显示处理状态），文本将自动打字上屏

#### 松手模式（防误停）
1. 将光标定位在输入框
2. 按一次 `F2` 键（可自定义），听到"滴"声后开始说话
3. 说话时手可以松开，防止长时间说话时误停
4. 说完后再按一次 `F2` 键，听到结束提示音
5. 等待处理，文本将自动打字上屏

**松手模式悬浮窗**：
- 蓝色药丸状态，中间显示迷你波形
- 左边 ❌ 按钮：取消录音
- 右边 ✓ 按钮：结束录音并转录
- 60 秒超时自动取消

### AI 助手模式

#### Q&A 模式（无选中文本）
1. 将光标定位在输入框
2. 按住 `Alt` + `Space` 键（可自定义），说出你的问题
3. 例如："What is the capital of France?"
4. 松开按键，LLM 将自动回答并插入答案

#### 文本处理模式（选中文本）
1. 在任何应用中选中一段文本
2. 按住 `Alt` + `Space` 键，说出你的命令
3. 常用命令示例：
   - "翻译成英文" - 将选中的中文翻译成英文
   - "润色一下" - 优化选中的文本
   - "总结一下" - 生成摘要
   - "扩写成三段" - 扩展内容
   - "添加注释" - 为代码添加注释
4. 松开按键，LLM 将处理选中的文本并替换

### 历史记录

在主界面的"历史记录"标签页可查看所有转录记录：
- 显示转录文本、时间、模式
- 支持搜索功能
- 点击复制按钮快速复制
- 一键清空所有历史

---

## 🛠️ 技术栈

### 前端
- **React 18** - UI 框架
- **TypeScript** - 类型安全
- **Tailwind CSS** - 样式框架
- **Vite** - 构建工具

### 后端 (Rust)
- **Tauri 2.0** - 跨平台桌面框架
- **rdev** - 全局键盘监听（支持 73 种按键）
- **cpal** - 实时音频录制
- **hound** - WAV 音频处理
- **tokio-tungstenite** - WebSocket 异步客户端
- **reqwest** - HTTP 客户端
- **arboard** - 剪贴板操作
- **Win32 SendInput** - 键盘模拟（Windows 原生 API）
- **rodio** - 音频播放（提示音）
- **tauri-plugin-updater** - 自动更新功能

### AI 服务
- **Alibaba Qwen ASR** - 阿里云语音识别（实时/HTTP）
- **Doubao ASR** - 豆包语音识别（实时/HTTP）
- **SiliconFlow SenseVoice** - 硅基流动语音识别（HTTP）
- **OpenAI-Compatible LLM** - 大语言模型后处理

---

## ⚙️ 高级配置

### ASR 引擎选择

应用支持多种 ASR 引擎，可在设置界面选择主引擎和备用引擎：

**主引擎选项**：
- **Qwen Realtime（推荐）**: WebSocket 实时流式，延迟最低（< 500ms）
- **Qwen HTTP**: 传统 HTTP 模式，稳定性更好
- **Doubao Realtime**: 豆包实时流式
- **Doubao HTTP**: 豆包 HTTP 模式

**备用引擎**：
- **SenseVoice**: 硅基流动 HTTP 模式
- 启用智能兜底后，主引擎失败时自动切换到备用引擎
- 并行竞速策略：主引擎重试 2 次（每次 500ms 间隔），备用引擎并行运行

### 快捷键自定义

支持 73 种按键的任意组合：
- **修饰键**：Ctrl（左/右）、Shift（左/右）、Alt（左/右）、Win（左/右）
- **字母键**：A-Z（26 个）
- **数字键**：0-9（10 个）
- **功能键**：F1-F12（12 个）
- **导航键**：方向键、Home、End、PageUp、PageDown
- **特殊键**：Space、Tab、Escape、Enter、Backspace、Delete、Insert、CapsLock

**配置示例**：
- 听写模式主键：`Ctrl + Win`（可改为 `Ctrl + Shift + A` 等）
- 松手模式键：`F2`（可改为 `F8`、`Space` 等）
- AI 助手键：`Alt + Space`（可改为 `Ctrl + Q` 等）

### LLM 预设管理

#### 听写模式预设
可以定义不同的预设来处理识别后的文本：
- **文本润色**：去除口语词（嗯、啊），修正标点，使语句通顺
- **中译英**：直接将中文语音翻译成地道的英文输出
- **邮件整理**：将口语化的指令转换为正式的邮件格式
- **自定义**：在设置界面添加、删除或修改预设的 System Prompt

#### AI 助手预设
配置两个系统提示词：
- **Q&A 系统提示词**：用于回答用户的问题
  - 例如："You are a helpful AI assistant. Provide clear and concise answers."
- **文本处理系统提示词**：用于处理选中的文本
  - 例如："You are an expert text editor. Follow user commands to modify the selected text."

### 系统托盘

- **最小化到托盘**：关闭窗口时应用不会退出，而是隐藏到系统托盘
- **开机自启动**：Windows 注册表方式实现（需要管理员权限）
- **托盘菜单**：右键托盘图标可显示/隐藏窗口或退出应用

### 自动更新

- **自动检查**：应用启动时自动检查更新
- **手动检查**：在设置界面点击"检查更新"按钮
- **6 个镜像源**：确保更新下载的可靠性
  - gh-proxy.org
  - hk.gh-proxy.org
  - cdn.jsdelivr.net
  - github.com（直连）
  - cdn.gh-proxy.org
  - edgeone.gh-proxy.org
- **安全验证**：Ed25519 签名验证
- **静默安装**：下载完成后自动安装，无需用户干预

---

## 🚀 开发指南

### 环境要求

- **Node.js** >= 18.0.0
- **Rust** >= 1.70.0
- **Windows** 10/11 (64-bit)

### 开发环境搭建

```bash
# 1. 克隆项目
git clone <repository-url>
cd push-2-talk

# 2. 安装前端依赖
npm install

# 3. 运行开发服务器（需要管理员权限）
npm run tauri dev
```

### 构建生产版本

```bash
npm run tauri build
```

生成的安装包位于：`src-tauri/target/release/bundle/nsis/`

**注意**：不再支持 MSI 安装包，仅提供 NSIS 安装包，以避免自动更新时创建多实例。

### 测试 API

使用独立的测试工具验证 Qwen ASR API：

```bash
cd src-tauri
cargo run --bin test_api
```

详细说明请参考 [测试工具使用说明.md](./测试工具使用说明.md)

### Rust 后端开发

```bash
cd src-tauri
cargo build    # 构建
cargo check    # 快速检查
cargo test     # 运行测试
```

---

## 📁 项目结构

```
├── src                          # 前端源码
│   ├── App.tsx                  # 主窗口（配置界面、历史记录）
│   ├── OverlayWindow.tsx        # 悬浮窗（录音状态显示）
│   ├── index.css                # 全局样式
│   ├── main.tsx                 # 主窗口入口
│   └── overlay-main.tsx         # 悬浮窗入口
├── src-tauri                    # 后端源码
│   ├── capabilities             # Tauri 权限配置
│   │   └── default.json
│   ├── icons                    # 应用图标
│   │   └── icon.ico
│   ├── src
│   │   ├── asr                  # ASR 模块（重构后的架构）
│   │   │   ├── http             # HTTP 模式 ASR
│   │   │   │   ├── doubao.rs
│   │   │   │   ├── qwen.rs
│   │   │   │   └── sensevoice.rs
│   │   │   ├── realtime         # 实时流式 ASR
│   │   │   │   ├── doubao.rs
│   │   │   │   └── qwen.rs
│   │   │   ├── mod.rs
│   │   │   ├── race_strategy.rs # 并发请求竞速策略
│   │   │   └── utils.rs
│   │   ├── pipeline             # 处理管道
│   │   │   ├── normal.rs        # 听写模式管道
│   │   │   ├── assistant.rs     # AI 助手模式管道
│   │   │   └── mod.rs
│   │   ├── audio_recorder.rs    # 录音（非流式）
│   │   ├── streaming_recorder.rs # 录音（流式）
│   │   ├── audio_utils.rs       # 音频工具（VAD、RMS、波形）
│   │   ├── beep_player.rs       # 提示音播放
│   │   ├── clipboard_manager.rs # 剪贴板管理（AI 助手）
│   │   ├── config.rs            # 配置管理
│   │   ├── hotkey_service.rs    # 全局快捷键（支持 73 键）
│   │   ├── lib.rs               # Tauri 主入口
│   │   ├── llm_post_processor.rs # LLM 后处理（听写模式）
│   │   ├── assistant_processor.rs # LLM 处理（AI 助手模式）
│   │   ├── main.rs              # Rust 主函数
│   │   ├── test_api.rs          # API 测试工具
│   │   └── text_inserter.rs     # 文本插入
│   ├── build.rs                 # 构建脚本
│   ├── Cargo.toml               # Rust 依赖配置
│   └── tauri.conf.json          # Tauri 配置
├── CLAUDE.md                    # Claude Code 项目指南
├── LICENSE                      # MIT 许可证
├── README.md                    # 项目说明
├── package.json                 # 前端依赖配置
└── vite.config.ts               # Vite 构建配置
```

---

## ⚙️ 配置说明

### 配置文件位置
```
%APPDATA%\PushToTalk\config.json
```

### 配置文件格式示例
```json
{
  "asr_config": {
    "primary": {
      "provider": "qwen",
      "mode": "realtime",
      "dashscope_api_key": "sk-your-dashscope-key"
    },
    "fallback": {
      "provider": "siliconflow",
      "mode": "http",
      "siliconflow_api_key": "sk-your-siliconflow-key"
    },
    "enable_fallback": true
  },
  "dual_hotkey_config": {
    "dictation": {
      "keys": [
        {"key": "ControlLeft"},
        {"key": "MetaLeft"}
      ],
      "mode": "Press",
      "enable_release_lock": true,
      "release_mode_keys": [
        {"key": "F2"}
      ]
    },
    "assistant": {
      "keys": [
        {"key": "AltLeft"},
        {"key": "Space"}
      ],
      "mode": "Press",
      "enable_release_lock": false,
      "release_mode_keys": null
    }
  },
  "llm_enabled": true,
  "llm_api_key": "sk-your-llm-key",
  "llm_base_url": "https://open.bigmodel.cn/api/paas/v4",
  "llm_model": "glm-4-flash",
  "llm_system_prompt": "你是一个专业的文本润色助手...",
  "llm_presets": [
    {
      "name": "文本润色",
      "prompt": "去除口语化表达，修正语法和标点..."
    },
    {
      "name": "中译英",
      "prompt": "将中文翻译成地道的英文..."
    }
  ],
  "assistant_config": {
    "enabled": true,
    "endpoint": "https://api.openai.com/v1/chat/completions",
    "model": "gpt-4",
    "api_key": "sk-your-assistant-key",
    "qa_system_prompt": "You are a helpful AI assistant...",
    "text_processing_system_prompt": "You are an expert text editor..."
  },
  "minimize_to_tray": true,
  "transcription_mode": "Dictation"
}
```

### 获取 API Key

| 服务商 | 用途 | 获取地址 | 费用 |
|--------|------|----------|------|
| 阿里云 DashScope | Qwen ASR | [控制台](https://bailian.console.aliyun.com/?tab=model#/api-key) | 大量免费额度（2025/03 前） |
| 豆包（字节跳动） | Doubao ASR | [录音识别](https://console.volcengine.com/ark/region:ark+cn-beijing/tts/recordingRecognition) / [流式识别](https://console.volcengine.com/ark/region:ark+cn-beijing/tts/speechRecognition) | 按量计费 |
| 硅基流动 | SenseVoice ASR | [账户管理](https://cloud.siliconflow.cn/me/account/ak) | 免费 |
| 智谱 AI | GLM-4-Flash LLM | [模型文档](https://docs.bigmodel.cn/cn/guide/models/free/glm-4-flash-250414) | 免费 |

---

## 🎯 使用技巧

### 最佳实践

1. **录音环境** - 在安静环境下录音，清晰发音，距离麦克风 10-30cm
2. **文本插入** - 确保目标窗口处于活动状态，光标可见
3. **快捷键使用** - 按住完整组合键再说话，避免部分按键误触
4. **ASR 引擎选择**
   - 实时模式：延迟低，适合短句（< 30 秒）
   - HTTP 模式：稳定性好，适合长段录音
   - 启用智能兜底：最大化成功率
5. **LLM 预设** - 针对不同场景创建多个预设，快速切换
6. **松手模式** - 长时间说话时使用松手模式，防止手指疲劳或误停
7. **AI 助手模式** - 选中文本后按快捷键，用自然语言描述你想要的效果

### 常见问题

**Q: 按快捷键没有反应？**
- A: 确保以管理员身份运行应用，Windows 要求管理员权限才能使用全局快捷键

**Q: 转录失败？**
- A: 检查网络连接和 API Key 是否有效。应用会自动重试最多 2 次，并在主引擎失败时切换到备用引擎

**Q: 转录一直处于"转录中"状态？**
- A: 应用有 6 秒超时机制，超时后会自动重试。如果持续失败，请检查：
  - 网络连接是否正常
  - API 服务是否可用
  - API Key 是否有效
  - 是否启用了智能兜底

**Q: 文本未插入？**
- A: 确保目标应用窗口处于前台且光标可见。如果使用松手模式，悬浮窗会自动隐藏 150ms 后再插入文本，以确保目标窗口获得焦点

**Q: 悬浮窗不显示？**
- A: 检查是否被其他窗口遮挡，或尝试重启应用

**Q: 开机自启动设置失败？**
- A: 需要以管理员身份运行应用才能修改 Windows 注册表

**Q: 历史记录在哪里？**
- A: 在主界面切换到"历史记录"标签页即可查看，支持搜索和清空

**Q: 快捷键冲突怎么办？**
- A: 在设置界面自定义快捷键，支持 73 种按键的任意组合

**Q: AI 助手模式没有捕获选中的文本？**
- A: 确保在按快捷键前已经选中文本，应用会等待 100ms 后自动复制选中内容。如果仍然失败，应用会重试最多 3 次

**Q: 松手模式按钮不工作？**
- A: 确保点击悬浮窗上的按钮（❌ 取消或 ✓ 完成），或再次按 F2 键结束录音

**Q: 更新下载失败？**
- A: 应用内置 6 个镜像源，会自动尝试其他源。如果都失败，请检查网络连接或稍后重试

---

## 📊 性能指标

| 指标 | 实时模式 (Realtime) | HTTP 模式 |
|------|-------------------|-----------|
| **首字延迟** | < 500ms | ~1.5s |
| **转录精度** | 98%+ (Qwen3/Doubao) | 98%+ (SenseVoice/Qwen) |
| **内存占用** | ~65MB（录音时） | ~60MB（录音时） |
| **网络消耗** | 持续小包传输（~16KB/s） | 单次大包传输（~100-500KB） |
| **超时重试** | 6s 超时，最多 2 次重试 | 6s 超时，最多 2 次重试 |
| **智能兜底** | 主引擎失败时自动切换到备用引擎 | 主引擎失败时自动切换到备用引擎 |
| **并行竞速** | 主引擎重试期间备用引擎并行运行 | 主引擎重试期间备用引擎并行运行 |

---

## 🔄 更新日志

### v0.0.14 (当前版本)

**新增功能：**
- ✨ **松手模式（Toggle/Release Mode）** - 按一次开始录音，再按一次结束，防止长时间说话时误停
- 🤖 **AI 助手模式** - 用语音命令处理选中的文本，或直接提问获得答案
- ⌨️ **自定义快捷键** - 支持 73 种按键的任意组合（修饰键、字母、数字、功能键、方向键等）
- 🔄 **智能兜底** - 主 ASR 引擎失败时自动切换到备用引擎（SiliconFlow SenseVoice）
- 🔄 **并行竞速** - 主引擎重试时备用引擎并行运行，返回最快成功的结果
- 🔄 **自动更新** - 内置 6 个镜像源，自动检查并安装更新
- 🎨 **悬浮窗优化** - 三种视觉状态（普通录音、锁定录音、转录中），实时波形可视化
- 🔊 **音频反馈** - 录音开始/结束时播放提示音
- 🛡️ **Ghost Key Detection** - Windows Win32 API 验证按键状态，防止卡键
- 🔧 **配置自动迁移** - 自动从旧版本配置迁移到新版本

**架构改进：**
- 🏗️ **Pipeline 架构** - 分离听写模式和 AI 助手模式的处理流程
- 🏗️ **双处理器设计** - AssistantProcessor 和 LlmPostProcessor 独立配置
- 🏗️ **剪贴板管理器** - RAII ClipboardGuard 确保剪贴板自动恢复
- 🏗️ **双系统提示词** - AI 助手模式支持 Q&A 和文本处理两种提示词
- 🏗️ **Focus Management** - 悬浮窗隐藏 + 延迟确保文本插入成功
- 🏗️ **Atomic State Management** - 无锁并发控制，使用原子标志防止竞态条件

**Bug 修复：**
- 🐛 修复悬浮窗卡死问题
- 🐛 修复停止服务时的异常状态检测
- 🐛 修复松手模式下文本无法插入的问题（焦点管理）
- 🐛 修复快捷键卡键问题（Ghost Key Detection）
- 🐛 修复自动更新创建多实例问题（删除 MSI 支持）
- 🐛 修复配置迁移兼容性问题

---

## 📈 路线图

### 计划中的功能

- [ ] 支持 macOS 和 Linux 平台
- [ ] 支持更多 ASR 引擎（Azure、Google、AWS）
- [ ] 支持更多 LLM 模型（Claude、Gemini、DeepSeek）
- [ ] 语音唤醒功能（无需按键，语音激活）
- [ ] 多语言支持（界面本地化）
- [ ] 云端配置同步
- [ ] 插件系统（支持第三方扩展）
- [ ] 语音命令宏（录制和回放常用命令序列）
- [ ] 实时翻译模式（边说边翻译）

---

## 🙏 致谢

感谢以下开源项目和服务：

- [Tauri](https://tauri.app/) - 强大的桌面应用框架
- [Alibaba Cloud](https://www.aliyun.com/) - 提供 Qwen ASR 服务
- [ByteDance](https://www.volcengine.com/) - 提供 Doubao ASR 服务
- [SiliconFlow](https://siliconflow.cn/) - 提供 SenseVoice ASR 服务
- [ZhipuAI](https://www.zhipuai.cn/) - 提供 GLM-4-Flash LLM 服务
- [Rust Audio](https://github.com/RustAudio) - 音频处理库
- 所有贡献者和用户的支持

---

## 📄 许可证

MIT License

---

## 💬 社区与支持

- **问题反馈**：[GitHub Issues](https://github.com/your-repo/issues)
- **功能建议**：[GitHub Discussions](https://github.com/your-repo/discussions)

---

<div align="center">

**⭐ 如果这个项目对你有帮助，请给它一个 Star！**

Made with ❤️ by PushToTalk Team

</div>
