# rsHell 全量重建设计

日期：2026-08-01

## 1. 产品定义

rsHell 是面向 Windows、macOS 和 Linux 的原生桌面终端模拟器与 SSH 会话管理器。产品体验参考 Xshell 和 MobaXterm，但当前路线只覆盖本地终端、SSH、SFTP 及其直接相关的效率工具，不包含 RDP、VNC、串口或远程桌面协议。

本次工作是全量重建。旧代码只作为行为和平台知识来源；新实现不兼容旧内部 API，也不保留旧模块结构。用户数据通过显式导入迁移，而不是让新架构适配旧实现细节。

## 2. 成功标准

P0 完成时必须满足：

1. 在三个目标平台上构建并启动原生 GUI。
2. 能创建、编辑、复制、移动、搜索和删除 SSH 连接配置。
3. 能启动本地终端和 SSH 终端，正确处理输入、输出、窗口尺寸、滚动、选择、复制和粘贴。
4. SSH 支持密码、私钥、SSH agent 和 keyboard-interactive，并默认执行严格的主机密钥验证。
5. 支持标签页、水平/垂直分屏、关闭、重连和会话状态反馈。
6. 密码不写入数据库或日志，只保存在系统凭据库；私钥口令遵循同一规则。
7. 配置写入具备事务性，应用异常退出不会留下半写数据。
8. 能一次性导入旧版 `connections.json` 和用户的 OpenSSH config，导入失败不会修改现有数据。
9. 单元测试、集成测试、SSH smoke、格式检查和 clippy 在 CI 中通过。

## 3. 技术路线

采用 Rust 2024 和 GTK4/Relm4。保留原生桌面技术栈是因为它已经覆盖三个目标平台，具备无 WebView 的原生输入和窗口能力，并可复用现有 GTK 发布知识。终端模拟器通过内部 `TerminalEngine` adapter 接入，不自行实现 VT 状态机。

首选引擎是 `wezterm-term`，从旧 revision `05343b3` 升级到设计时的 WezTerm 主线 revision `d69264df66fdcc928c7a30c673df108984fda821`，并保持 commit pin 以获得可复现构建。第一实施波次用固定 VT fixture、宽字符、鼠标、全屏 TUI、resize 和吞吐基准验证该 revision。只有在编译依赖无法隔离、关键行为不正确或性能门不通过时，才改用 `alacritty_terminal` adapter；GTK libvte 不作为主候选，因为 Windows 不是其可靠的一等目标平台。

不选择 Tauri/xterm.js，因为 WebView、IME、剪贴板和 Linux WebKitGTK 会形成新的平台差异；不选择 wgpu/egui 全自绘，因为文本输入、无障碍和复杂桌面控件的成本会延迟 SSH 核心交付。

## 4. 工作区与组件边界

仓库重建为 Cargo workspace：

```text
crates/
  rshell-core/       领域模型、验证规则、用例与应用事件
  rshell-storage/    SQLite、迁移、凭据库和导入器
  rshell-session/    PTY、SSH 传输、终端引擎 adapter 和会话 actor
  rshell-ui/         GTK4/Relm4 窗口、组件、渲染和输入适配
  rshell-platform/   平台启动、路径、外部程序和发布运行时适配
src/
  main.rs            组合根与进程入口
resources/
  style.css          应用样式
```

### 4.1 `rshell-core`

只依赖通用 Rust 库，不依赖 GTK、PTY、SQLite 或具体 SSH 实现。它定义：

- `ConnectionProfile`、`ConnectionGroup`、`CredentialRef`、`HostKeyPolicy`。
- `TerminalProfile`、`ColorScheme`、`KeyBinding`。
- `WorkspaceState`、`TabState`、`PaneTree`、`SessionDescriptor`。
- 创建、修改、排序、搜索、导入预览等用例。
- UI 命令与应用事件的稳定协议。

所有领域规则在这一层可无 GUI 测试。

### 4.2 `rshell-storage`

使用 SQLite 保存连接、分组、标签、终端配置和应用设置。schema 通过单调递增版本迁移；每个写用例在单个事务中完成。系统凭据库只通过 `CredentialVault` trait 暴露，数据库保存不可逆推出密码的引用标识。

导入器包括：

- 旧版 rsHell `connections.json`：解析、校验、预览、事务导入；发现旧明文密码时先写凭据库，全部成功后才提交数据库。
- OpenSSH `~/.ssh/config`：导入 Host、HostName、User、Port、IdentityFile、ProxyJump 等可静态解析字段；通配 Host 只作为模板预览，不自动变成连接。

### 4.3 `rshell-session`

每个会话由一个 actor 独占以下可变资源：

- PTY master/child 或原生 SSH channel。
- `TerminalEngine`（首选实现封装 `wezterm_term::Terminal`）。
- 选择、滚动和输入协议状态。
- 生命周期与重连状态。

外部通过有界命令通道发送 `Input`、`Resize`、`Scroll`、`Search`、`CopySelection`、`Shutdown` 等命令。actor 发布 `SessionEvent` 和 `Arc<RenderFrame>`；连续输出时合并脏帧，最多按显示刷新节奏通知 UI。UI 永不持有终端内部锁，因此不再存在跨线程锁顺序契约。

传输适配器：

- `LocalPtyTransport`：启动用户默认 shell 或显式命令。
- `OpenSshTransport`：调用系统 OpenSSH，适合复用用户 agent、config 和企业认证环境。
- `NativeSshTransport`：提供应用内密码、keyboard-interactive 和可控的主机密钥交互。

传输差异通过能力描述暴露，不伪装成完全相同的认证行为。

### 4.4 `rshell-ui`

UI 由小型 Relm4 组件组成：

- `MainWindow`
- `ConnectionSidebar`
- `ConnectionEditor`
- `SessionTabBar`
- `PaneHost`
- `TerminalView`
- `SettingsWindow`
- `TransferPanel`（P1）

组件只接收 view model 和发送命令，不直接访问数据库、凭据库或 PTY。`TerminalView` 负责 GDK 输入到终端命令的映射，以及 `RenderFrame` 到 Cairo/Pango 的绘制。

### 4.5 `rshell-platform`

集中处理 Windows DPI 与便携运行时、系统配置目录、默认 shell、浏览文件、外部编辑器、剪贴板差异和平台错误码。其他 crate 不包含散落的 `cfg(windows)` 业务分支。

## 5. 数据模型

P0 数据库包含以下核心表：

- `connection_groups(id, parent_id, name, position)`
- `connections(id, group_id, name, host, port, username, transport, credential_ref, host_key_policy, identity_file, remote_command, note, position, created_at, updated_at)`
- `connection_tags(connection_id, tag)`
- `terminal_profiles(id, name, settings_json, created_at, updated_at)`
- `app_settings(key, value_json)`
- `schema_migrations(version, applied_at)`

终端 profile 的可扩展设置保存在版本化 JSON 中，连接和分组等需要约束、排序和搜索的数据使用普通列。会话运行时状态不持久化到主数据库；异常恢复只保存可安全重建的 workspace 描述，不保存终端屏幕或明文输入。

## 6. 关键数据流

### 6.1 启动

1. 平台层配置进程运行时。
2. storage 打开数据库并在事务中执行迁移。
3. core 加载连接树、设置和可恢复 workspace。
4. UI 构建窗口；无可恢复会话时创建一个本地终端。

### 6.2 SSH 连接

1. UI 提交连接 ID。
2. core 读取连接与终端 profile，形成不可变 `LaunchRequest`。
3. storage 从凭据库读取所需秘密，并只把 `SecretString` 交给 session 层。
4. session actor 执行 DNS、主机密钥校验和认证，通过事件请求用户确认或补充凭据。
5. 成功后 actor 启动终端泵并发布帧；失败时发布可分类、可重试的错误。
6. 启动请求结束后尽快销毁临时秘密。

### 6.3 终端渲染

1. reader 将字节送入 actor 持有的 `TerminalEngine`。
2. actor 通过 `TerminalEngine` 根据 dirty rows 构建不可变 `RenderFrame`。
3. UI 主线程接收最新帧并 queue draw；过时帧自动丢弃。
4. 尺寸变化由 UI 发送 cells/pixels，actor 顺序更新模拟器和 PTY。

## 7. 功能优先级与排期顺序

### P0：可靠的终端与 SSH 管理器

- 连接 CRUD、分组、标签、搜索、复制与排序。
- 本地终端、系统 OpenSSH、原生 SSH。
- 密码、密钥、agent、keyboard-interactive、主机指纹确认。
- 标签页、水平/垂直分屏、重连和状态显示。
- 滚动缓冲、搜索、选择、复制粘贴、字体、配色和快捷键。
- 全局终端 profile 与连接覆盖。
- 旧 rsHell JSON 和 OpenSSH config 导入。
- SQLite、系统凭据库、三平台打包与自动测试。

### P1：专业 SSH 工作流

- ProxyJump/bastion 与 HTTP/SOCKS 代理配置。
- 本地、远程和动态端口转发。
- 与当前 SSH 会话关联的 SFTP 双栏/侧栏传输。
- 命令片段、广播输入、会话日志和快速命令。
- 可选择排除秘密的配置导入导出。
- 工作区保存与恢复。

### P2：可扩展效率能力

- 端到端加密的跨设备配置同步。
- 宏与任务自动化。
- 受限插件接口。
- 大规模连接资产的批量操作和审计辅助。

每一优先级只有在前一级质量门通过后开始，不用未完成的高级功能阻塞 P0。

## 8. 安全设计

- 默认严格验证 SSH 主机密钥；首次连接显示算法与 SHA-256 指纹并要求显式确认。
- 不提供静默接受所有主机密钥的全局开关。
- 密码和私钥口令使用零化秘密容器，禁止 `Debug` 输出。
- 日志在结构化字段入口统一脱敏，连接错误不得包含认证响应。
- 系统 OpenSSH 参数按独立 argv 构造，不通过 shell 拼接；host 和 command 分隔符经过注入测试。
- 导入含明文秘密的数据时，只有凭据库与数据库事务均成功才报告完成；失败时不留下部分连接。
- 数据库权限按平台限制为当前用户；导出默认不含秘密。

## 9. 错误处理

库边界使用 `thiserror` 定义可分类错误；应用组合根可使用 `anyhow` 添加上下文。错误分类至少包括：配置验证、存储、凭据库、主机密钥、认证、网络、PTY、子进程和平台集成。

UI 对可恢复错误提供原地重试或编辑连接；不可恢复错误保持会话错误页并允许复制诊断信息。后台线程/actor panic 被转换为会话崩溃事件，不能带走 GTK 主循环。

## 10. 测试与验收

### 自动测试

- `rshell-core`：领域规则、pane tree、搜索、配置解析。
- `rshell-storage`：迁移、事务回滚、凭据失败、旧 JSON/OpenSSH 导入 fixture。
- `rshell-session`：fake transport 生命周期、resize 顺序、关闭幂等、重连状态机。
- 终端金丝雀：向 `TerminalEngine` 输入固定 VT 字节流，断言 frame cells、颜色、光标和宽字符；同一 fixture 用于验证候选 adapter。
- SSH smoke：本地临时 SSH server 分别验证系统 OpenSSH 和原生 SSH。
- UI view-model 测试：连接编辑、tab/pane 操作、错误与认证提示。

### CI 质量门

- `cargo fmt -- --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 三平台构建；支持环境中运行 SSH smoke。

### P0 真实表面场景

1. 新建密码连接，确认凭据库写入、数据库无明文、连接成功并执行命令。
2. 新建密钥连接，确认 agent/私钥路径、首次主机指纹确认和再次连接行为。
3. 本地终端运行交互 shell，执行彩色输出、宽字符、全屏 TUI、resize、复制粘贴和搜索。
4. 两个 tab 与水平/垂直 split 同时运行，关闭和重连不会残留子进程。
5. 导入旧 JSON 后，连接数量、分组、认证引用和终端覆盖保持一致；失败 fixture 完整回滚。

## 11. 实施策略

采用“核心优先的干净替换”：

1. 建立 workspace、纯领域模型和测试，定义稳定协议。
2. 建立 SQLite/凭据库与导入器。
3. 建立 actor 会话运行时、本地 PTY 和终端帧。
4. 建立最小 GTK shell，接通本地终端真实路径。
5. 接入两种 SSH transport 与认证/主机密钥交互。
6. 完成连接管理、tab/split、设置和数据导入 UI。
7. 完成三平台 CI、打包、smoke 和 P0 手工验收。
8. 删除剩余旧实现和不再使用的依赖、资源与测试。

实现期间允许短暂保留旧文件用于行为对照，但旧代码不进入新组合根。每个波次都必须保持 workspace 可编译并有对应测试，避免一次性删除后长期不可运行。

## 12. 非目标

- 自研 SSH 协议或 VT 解析器。
- RDP、VNC、串口、X server 或远程桌面聚合。
- 在 P0 中实现插件市场、云同步或团队服务端。
- 为兼容旧内部类型而污染新组件边界。
- 在功能正确性与安全门通过前追求完整的 Xshell/MobaXterm 功能数量。
