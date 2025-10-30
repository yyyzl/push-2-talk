# PushToTalk - 语音输入助手

<div align="center">

**按住快捷键说话，松开自动转录并插入文本**

[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18-blue.svg)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)

</div>

---

## 📖 简介

PushToTalk 是一个桌面应用，通过全局快捷键实现快速语音输入。按住 **Ctrl+Win** 说话，松开后自动转录并插入文本到任何应用程序。

### ✨ 核心特性

- 🎤 **全局快捷键录音** - 在任何应用中按住 Ctrl+Win 即可录音
- 🤖 **智能语音识别** - 集成阿里云 Qwen ASR，支持中英文混合识别
- ⚡ **自动文本插入** - 转录完成后自动插入到光标位置
- 💾 **配置持久化** - API Key 安全保存，无需重复输入
- 🔄 **智能重试机制** - 请求超时自动重试，提高稳定性
- 🎯 **轻量高效** - 内存占用 < 60MB，启动速度 < 2秒

---

## 🎬 快速开始

### 安装

1. 下载最新版本的安装包（位于 `src-tauri/target/release/bundle/`）：
   - MSI 安装包：`PushToTalk_0.1.0_x64_en-US.msi`
   - NSIS 安装程序：`PushToTalk_0.1.0_x64-setup.exe`

2. 运行安装程序完成安装

3. ⚠️ **重要**：右键点击应用图标，选择"以管理员身份运行"

### 配置

1. 启动应用
2. 输入你的 [DashScope API Key](https://dashscope.console.aliyun.com/)
3. 点击"保存配置"
4. 点击"启动应用"

### 使用

1. 打开任何文本编辑器（记事本、Word、浏览器等）
2. 将光标放在要插入文本的位置
3. **按住 Ctrl+Win** 并开始说话
4. **松开按键** 停止录音
5. 等待几秒，转录的文本会自动插入

---

## 🛠️ 技术栈

### 前端
- **React 18** - UI 框架
- **TypeScript** - 类型安全
- **Tailwind CSS** - 样式框架
- **Vite** - 构建工具

### 后端 (Rust)
- **Tauri 2.0** - 跨平台桌面框架
- **rdev** - 全局键盘监听
- **cpal** - 实时音频录制
- **hound** - WAV 音频处理
- **reqwest** - HTTP 客户端
- **arboard** - 剪贴板操作
- **enigo** - 输入模拟

### AI 服务
- **Alibaba Qwen ASR** - 语音识别 API

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

生成的安装包位于：`src-tauri/target/release/bundle/`

### 测试 API

使用独立的测试工具验证 Qwen ASR API：

```bash
cd src-tauri
cargo run --bin test_api
```

详细说明请参考 [测试工具使用说明.md](./测试工具使用说明.md)

---

## 📁 项目结构

```
push-2-talk/
├── src/                        # React 前端
│   ├── App.tsx                 # 主界面组件
│   └── main.tsx                # 入口文件
├── src-tauri/                  # Rust 后端
│   ├── src/
│   │   ├── lib.rs              # Tauri 命令和状态管理
│   │   ├── audio_recorder.rs   # 音频录制模块
│   │   ├── hotkey_service.rs   # 快捷键监听模块
│   │   ├── qwen_asr.rs         # Qwen ASR API 客户端
│   │   ├── text_inserter.rs    # 文本插入模块
│   │   └── config.rs           # 配置管理模块
│   └── Cargo.toml              # Rust 依赖配置
├── MVP需求文档.md               # 需求规格说明
├── 测试工具使用说明.md          # API 测试工具文档
├── 项目进展.md                  # 开发进展记录
└── README.md                    # 本文件
```

---

## ⚙️ 配置说明

### 配置文件位置
```
%APPDATA%\PushToTalk\config.json
```

### 配置文件格式
```json
{
  "dashscope_api_key": "sk-your-api-key-here"
}
```

### 获取 API Key

1. 访问 [阿里云 DashScope 控制台](https://dashscope.console.aliyun.com/)
2. 注册/登录账号
3. 创建 API Key
4. 复制 Key 并粘贴到应用中

---

## 🎯 使用技巧

### 最佳实践

1. **录音环境** - 在安静环境下录音，清晰发音
2. **文本插入** - 确保目标窗口处于活动状态，光标可见
3. **快捷键使用** - 按住完整组合键（Ctrl+Win）再说话

### 常见问题

**Q: 按快捷键没有反应？**
- A: 确保以管理员身份运行应用

**Q: 转录失败？**
- A: 检查网络连接和 API Key 是否有效。应用会自动重试最多3次

**Q: 转录一直处于"转录中"状态？**
- A: 应用有10秒超时机制，超时后会自动重试。如果持续失败，请检查网络和API服务状态

**Q: 文本未插入？**
- A: 确保目标应用窗口处于前台且光标可见

---

## 📊 性能指标

| 指标 | 数值 |
|------|------|
| 按键响应延迟 | ~50ms |
| 音频录制延迟 | ~100ms |
| API 响应时间 | 1-3秒 (取决于网络) |
| 应用启动时间 | ~2秒 |
| 内存占用 | ~60MB |

---

## 🗺️ 开发路线图

### ✅ v0.1.0 - MVP (已完成)
- [x] 全局快捷键录音 (Ctrl+Win)
- [x] 阿里云 Qwen ASR 集成
- [x] 自动文本插入
- [x] 配置持久化
- [x] 基础 GUI 界面
- [x] 标点符号自动去除
- [x] API 请求超时机制（10秒）
- [x] 自动重试逻辑（最多2次）

### 🔄 v0.2.0 - 功能增强 (计划中)
- [ ] Toggle 录音模式
- [ ] 自定义快捷键
- [ ] 音频反馈（提示音）
- [ ] 历史记录功能

### 🔮 v0.3.0 - 音频优化 (计划中)
- [ ] 静音检测和移除
- [ ] 音频设备选择
- [ ] 音频质量调整

### 🎉 v1.0.0 - 完整版 (未来)
- [ ] 支持多个 ASR 服务提供商
- [ ] 流式实时转录
- [ ] AI 文本优化
- [ ] 跨平台支持 (macOS, Linux)

---

## 📝 相关文档

- [MVP需求文档.md](./MVP需求文档.md) - 完整的功能需求和技术设计
- [项目进展.md](./项目进展.md) - 开发进展和已完成功能
- [测试工具使用说明.md](./测试工具使用说明.md) - API 测试工具使用指南

---

## 🙏 致谢

感谢以下开源项目和服务：

- [Tauri](https://tauri.app/) - 强大的桌面应用框架
- [Alibaba Cloud](https://www.aliyun.com/) - 提供 Qwen ASR 服务
- [Rust Audio](https://github.com/RustAudio) - 音频处理库
- 所有贡献者和用户的支持

---

## 📄 许可证

MIT

---

<div align="center">

**⭐ 如果这个项目对你有帮助，请给它一个 Star！**

Made with ❤️ by PushToTalk Team

</div>
