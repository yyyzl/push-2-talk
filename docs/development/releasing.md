# 双平台发布

发布使用 `.github/workflows/release.yml`，平台回归继续由 `platform-check.yml` 执行。普通 main 推送不再额外执行旧的 Windows 单平台发布构建。

## 流程

1. 同步 package.json、package-lock.json、Cargo.toml、Cargo.lock 和 tauri.conf.json 的版本，在 `docs/releases/v<版本>.md` 编写面向用户的说明。
2. 在待合并分支手动运行 Release Pipeline。非标签运行只构建并保存 Actions 附件，不创建 GitHub Release。
3. 检查平台回归与正式构建都通过后合并，再推送对应 `v*` 标签。
4. 标签任务先建立 draft。三个原生 runner 分别构建 Windows x64、macOS Apple Silicon 和 macOS Intel，生成安装包及更新签名。
5. 所有构建成功后统一汇总：验证 Minisign/Ed25519 签名与应用公钥匹配，生成包含三个目标的 latest.json 和 SHA256SUMS.txt，再把全部产物与说明填入草稿。
6. 检查草稿附件、安装说明、签名状态和更新链接后，才公开发布。工作流不会自动取消 draft；已公开版本不允许覆盖重建。

更新清单同时保留 `windows-x86_64` 和 `windows-x86_64-nsis`，兼容旧版客户端；macOS 使用独立架构的 app 更新包。统一汇总避免并行上传 latest.json 丢掉其他平台。

## 构建与签名

- Tauri 更新签名来自 GitHub Actions secrets：`TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。不在日志输出密钥。
- 本轮 macOS 使用 `APPLE_SIGNING_IDENTITY=-` 进行 ad-hoc 签名，明确不做 Developer ID 签名或公证；发布说明必须告知首次打开及升级权限限制。
- macOS 原生构建分别运行在 `macos-15` 与 `macos-15-intel`，设置 `MACOSX_DEPLOYMENT_TARGET=12.0`。
- Opus 通过 `OPUS_NO_PKG=1` 强制走源码、静态编译，不复用 Homebrew 预编译库的系统版本要求。打包后校验应用签名、主程序架构及动态库引用。

产物校验的纯逻辑回归位于 `tests/releaseArtifacts.test.ts`。使用临时测试密钥覆盖缺包、缺签名、包体篡改、签名错配与说明篡改；不接触发布私钥。

## 数据恢复验证

Windows 安装切换测试应隔离用户数据。移动或恢复 WebView 目录前，必须等宿主和对应 WebView2 子进程全部退出。文件数量或散列一致不足以证明应用读取正常；恢复后还要启动应用，核对配置、词库及历史页面数量与时间范围。不要把空页面直接认定为数据丢失，更不要先删除 WebView 目录。
