# rsHell P0 全量重建实施计划

> **For agentic workers:** Use the subagent-driven-development skill to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Windows、macOS、Linux 上交付完整 P0：原生 GTK GUI、事务化连接管理、安全凭据、可用的本地/SSH 终端、标签与分屏、原子导入以及可自动证明的三平台质量门。

**Architecture:** 根二进制只做组合与进程启动；`rshell-core` 定义纯领域模型、用例、端口和 UI 协议，`rshell-storage`、`rshell-session`、`rshell-platform` 分别实现持久化、会话 actor 和平台能力，`rshell-ui` 只消费 view model/事件并发送命令。终端状态只由单会话 actor 持有，SQLite 只由存储工作线程持有，凭据只通过系统凭据库引用进入 session；旧实现从第一项任务起不再进入 Cargo 组合根，并在新真实路径接通时删除。

**Tech Stack:** Rust 2024；GTK4 0.10.3 / Relm4 0.10.1；SQLite via `rusqlite =0.40.1`；系统凭据库 via `keyring =4.1.5`；`secrecy =0.10.3` / `zeroize =1.9.0`；Tokio；`portable-pty`；system OpenSSH；native SSH via `russh =0.62.4` + `ring`；首选 `wezterm-term` 固定 revision `d69264df66fdcc928c7a30c673df108984fda821`。

**Global Constraints:**
- “本次工作是全量重建。旧代码只作为行为和平台知识来源；新实现不兼容旧内部 API，也不保留旧模块结构。”
- “采用 Rust 2024 和 GTK4/Relm4。”
- “终端模拟器通过内部 `TerminalEngine` adapter 接入，不自行实现 VT 状态机。”
- `wezterm-term` 和 `wezterm-surface` 必须固定在 `d69264df66fdcc928c7a30c673df108984fda821`；compatibility spike 未通过时删除该实现并仅保留 `alacritty_terminal =0.26.0` adapter，产品中不得并存双引擎。
- “默认严格验证 SSH 主机密钥；首次连接显示算法与 SHA-256 指纹并要求显式确认。”
- “不提供静默接受所有主机密钥的全局开关。”
- “密码和私钥口令使用零化秘密容器，禁止 `Debug` 输出。”
- “系统 OpenSSH 参数按独立 argv 构造，不通过 shell 拼接；host 和 command 分隔符经过注入测试。”
- “每个写用例在单个事务中完成。”
- SQLite 与系统凭据库之间采用可恢复操作日志和补偿状态机，不宣称两个资源可原子提交；任何中断都不得暴露半条连接或数据库明文。
- “UI 永不持有终端内部锁。”
- 只实现 P0；ProxyJump 只允许作为 OpenSSH 导入语义被识别并委托给 system OpenSSH，端口转发、代理、SFTP、工作区恢复以及所有 P1/P2 功能不得进入代码任务。
- 所有平台命令示例使用 `pwsh` 语法；不得在实施中以 shell 字符串拼接代替 argv。
- 不得安装本机依赖；CI 工作流自身声明的 runner 依赖安装不等于本机安装。
- 当前未授权任何 Git 写操作。每个 commit 步骤仅在用户后续单独授权后执行；未授权时跳过，不得运行 `git add`、`git commit`、`git push`、`git tag` 或其他 Git 写命令。

---

## 0. 已锁定决策与完成定义

### 0.1 依赖方向

```text
src/main.rs (composition root)
  ├── rshell-core
  ├── rshell-storage ──> rshell-core, rshell-platform
  ├── rshell-session ──> rshell-core, rshell-platform
  ├── rshell-ui ───────> rshell-core, rshell-platform
  └── rshell-platform

禁止：rshell-ui -> rshell-storage / rshell-session
禁止：rshell-core -> GTK / PTY / SQLite / russh
```

### 0.2 稳定接口（后续任务必须使用这些名称）

`rshell-core` 的领域与端口：

```rust
pub struct ConnectionId(pub Uuid);
pub struct GroupId(pub Uuid);
pub struct TerminalProfileId(pub Uuid);
pub struct SessionId(pub Uuid);
pub struct PaneId(pub Uuid);
pub struct CredentialRef(pub String);

pub enum TransportKind { SystemOpenSsh, NativeSsh }
pub enum AuthenticationKind { Password, PublicKey, Agent, KeyboardInteractive }
pub enum HostKeyPolicy { Strict }

pub struct ConnectionProfile {
    pub id: ConnectionId,
    pub group_id: Option<GroupId>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub transport: TransportKind,
    pub authentication: AuthenticationKind,
    pub credential_ref: Option<CredentialRef>,
    pub identity_file: Option<PathBuf>,
    pub host_key_policy: HostKeyPolicy,
    pub remote_command: Option<String>,
    pub note: String,
    pub tags: BTreeSet<String>,
    pub terminal_overrides: TerminalOverrides,
    pub position: i64,
}

pub enum SecretUpdate {
    Unchanged,
    Set(SecretString),
    Clear,
}

pub trait ConnectionRepository: Send + Sync {
    fn load_catalog(&self) -> Result<ConnectionCatalog, RepositoryError>;
    fn apply(&self, mutation: CatalogMutation) -> Result<ConnectionCatalog, RepositoryError>;
    fn load_terminal_profiles(&self) -> Result<Vec<TerminalProfile>, RepositoryError>;
    fn save_terminal_profile(&self, profile: TerminalProfile) -> Result<(), RepositoryError>;
    fn load_settings(&self) -> Result<AppSettings, RepositoryError>;
    fn save_settings(&self, settings: AppSettings) -> Result<(), RepositoryError>;
}

pub trait CredentialPort: Send + Sync {
    fn apply_catalog(
        &self,
        mutation: CatalogMutation,
        secret: SecretUpdate,
    ) -> Result<ConnectionCatalog, CredentialOperationError>;
    fn get(&self, key: &CredentialRef) -> Result<Option<SecretString>, CredentialOperationError>;
}

pub trait ImportPort: Send + Sync {
    fn preview(&self, source: ImportSourceKind, path: &Path) -> Result<ImportPreviewView, ImportError>;
    fn commit(
        &self,
        preview: ImportPreviewId,
        selected: &BTreeSet<ImportCandidateId>,
    ) -> Result<ImportReportView, ImportError>;
    fn cancel(&self, preview: ImportPreviewId) -> Result<(), ImportError>;
}

pub struct SessionBinding {
    pub id: SessionId,
    pub events: async_channel::Receiver<SessionUiEvent>,
    pub frames: tokio::sync::watch::Receiver<Option<Arc<RenderFrame>>>,
}

pub trait SessionPort: Send + Sync {
    fn launch_local(&self, pane: PaneId, terminal: ResolvedTerminalProfile) -> Result<SessionBinding, SessionFailure>;
    fn launch_ssh(
        &self,
        pane: PaneId,
        profile: ConnectionProfile,
        terminal: ResolvedTerminalProfile,
        initial_size: TerminalSize,
        secret: Option<SecretString>,
    ) -> Result<SessionBinding, SessionFailure>;
    fn command(&self, session: SessionId, command: SessionUiCommand) -> Result<(), SessionFailure>;
    fn shutdown_all(&self) -> Result<(), SessionFailure>;
}
```

这些 port trait 及其签名中的 `CredentialOperationError`、`ImportError`、`ReconcileReport`、preview/report view 均由 `rshell-core` 定义；storage/session crate 只能实现或映射它们，不能让 core 引用具体 coordinator、importer 或 manager 类型。`CredentialOperationError` 只含 `Vault(VaultFailure)`、`Repository(RepositoryError)`、`ReconciliationRequired`，`VaultFailure` 只含 `Unavailable/NoEntry/Denied/Platform`；`ReconcileReport` 只含 completed/cleaned/pending 计数。进程启动唯一由 Task 19 的 composition root 拥有：platform configure、storage open/migrate、credential reconcile、catalog/settings load 完成后，才把纯 `AppBootstrapState` 和上述 ports 交给 `ApplicationService::start`。`ApplicationService` 不重复执行任何进程级初始化。

上述 core 名称的精确定义：

```rust
pub struct ConnectionGroup {
    pub id: GroupId,
    pub parent_id: Option<GroupId>,
    pub name: String,
    pub position: i64,
}

pub struct ConnectionCatalog {
    pub groups: BTreeMap<GroupId, ConnectionGroup>,
    pub connections: BTreeMap<ConnectionId, ConnectionProfile>,
}

pub enum CatalogMutation {
    Create(ConnectionProfile),
    Update(ConnectionProfile),
    Duplicate { source: ConnectionId, destination: Option<GroupId> },
    Move { connection: ConnectionId, destination: Option<GroupId>, position: usize },
    Delete(ConnectionId),
    CreateGroup(ConnectionGroup),
    RenameGroup { group: GroupId, name: String },
    MoveGroup { group: GroupId, parent: Option<GroupId>, position: usize },
    DeleteGroup(GroupId),
    SetTags { connection: ConnectionId, tags: BTreeSet<String> },
}

pub enum CatalogOutcome {
    Connection(ConnectionId),
    Group(GroupId),
    Updated,
    Deleted,
}

impl CatalogOutcome {
    pub fn connection_id(self) -> Result<ConnectionId, DomainError>;
}

pub struct TerminalProfile {
    pub id: TerminalProfileId,
    pub name: String,
    pub settings: TerminalSettingsV1,
}
impl TerminalProfile {
    pub fn p0_default() -> Self;
}

pub struct AppSettings {
    pub default_terminal_profile: TerminalProfileId,
    pub color_scheme: ColorScheme,
    pub key_bindings: Vec<KeyBinding>,
}

pub struct TerminalSettingsV1 {
    pub terminal_type: String,
    pub initial_cols: u16,
    pub initial_rows: u16,
    pub scrollback_lines: usize,
    pub font_family: String,
    pub font_size: f32,
    pub color_scheme: ColorScheme,
    pub key_bindings: Vec<KeyBinding>,
    pub left_alt_as_meta: bool,
    pub right_alt_as_meta: bool,
    pub enable_csi_u: bool,
    pub enable_kitty_keyboard: bool,
    pub mouse_reporting: bool,
    pub scroll_on_output: bool,
    pub scroll_on_keypress: bool,
    pub answerback: String,
}

#[derive(Default)]
pub struct TerminalOverrides {
    pub terminal_type: Option<String>,
    pub initial_cols: Option<u16>,
    pub initial_rows: Option<u16>,
    pub scrollback_lines: Option<usize>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub color_scheme: Option<ColorScheme>,
    pub key_bindings: Option<Vec<KeyBinding>>,
    pub left_alt_as_meta: Option<bool>,
    pub right_alt_as_meta: Option<bool>,
    pub enable_csi_u: Option<bool>,
    pub enable_kitty_keyboard: Option<bool>,
    pub mouse_reporting: Option<bool>,
    pub scroll_on_output: Option<bool>,
    pub scroll_on_keypress: Option<bool>,
    pub answerback: Option<String>,
}

pub struct ResolvedTerminalProfile {
    pub terminal_type: String,
    pub cols: u16,
    pub rows: u16,
    pub scrollback_lines: usize,
    pub font_family: String,
    pub font_size: f32,
    pub color_scheme: ColorScheme,
    pub key_bindings: Vec<KeyBinding>,
    pub left_alt_as_meta: bool,
    pub right_alt_as_meta: bool,
    pub enable_csi_u: bool,
    pub enable_kitty_keyboard: bool,
    pub mouse_reporting: bool,
    pub scroll_on_output: bool,
    pub scroll_on_keypress: bool,
    pub answerback: String,
}

pub enum SplitAxis { Horizontal, Vertical }
pub enum PaneTree {
    Leaf { pane_id: PaneId, session_id: Option<SessionId> },
    Split { axis: SplitAxis, ratio: f32, first: Box<PaneTree>, second: Box<PaneTree> },
}
pub struct TabState { pub id: Uuid, pub title: String, pub pane_tree: PaneTree, pub active_pane: PaneId }
pub struct WorkspaceState { pub tabs: Vec<TabState>, pub active_tab: Option<Uuid> }

#[derive(thiserror::Error, Debug)]
pub enum RepositoryError {
    #[error("storage unavailable")]
    Unavailable,
    #[error("storage busy")]
    Busy,
    #[error("storage constraint: {0}")]
    Constraint(String),
    #[error("storage data is corrupt")]
    Corrupt,
}
```

稳定 terminal/workspace protocol 的精确定义：

```rust
pub enum UiCommand {
    ApplyCatalog { mutation: CatalogMutation, secret: SecretUpdate },
    SearchConnections(String),
    NewLocalTab,
    StartLocal { pane: PaneId },
    Connect { pane: PaneId, connection: ConnectionId },
    Split { pane: PaneId, axis: SplitAxis },
    ClosePane(PaneId),
    CloseTab(Uuid),
    Session { session: SessionId, command: SessionUiCommand },
    SaveTerminalProfile(TerminalProfile),
    SaveSettings(AppSettings),
    PreviewImport { source: ImportSourceKind, path: PathBuf },
    CommitImport { preview: ImportPreviewId, selected: BTreeSet<ImportCandidateId> },
    CancelImport { preview: ImportPreviewId },
    Respond { session: SessionId, interaction: InteractionId, response: InteractionResponse },
    Shutdown,
}

impl UiCommand {
    pub fn secret_update(&self) -> Option<&SecretUpdate>;
}

pub enum AppEvent {
    CatalogChanged(ConnectionCatalog),
    SearchResults(Vec<ConnectionId>),
    WorkspaceChanged(WorkspaceState),
    Session { session: SessionId, event: SessionUiEvent },
    SettingsChanged(AppSettings),
    ImportPreview(ImportPreviewView),
    ImportCompleted(ImportReportView),
    InteractionRequired { session: SessionId, request: InteractionRequest },
    OperationFailed(AppFailure),
    ShutdownComplete,
}

pub enum SessionUiCommand {
    Input(TerminalInput), Paste(SecretString), Resize(TerminalSize), Scroll(i32),
    Search(SearchQuery), Select(SelectionRange), CopySelection,
    Respond { interaction: InteractionId, response: InteractionResponse },
    Reconnect, Shutdown,
}

pub enum SessionUiEvent {
    State(SessionState), Frame(Arc<RenderFrame>), Search(Vec<SearchMatch>),
    Copy(String), InteractionRequired(InteractionRequest), Exited(ExitStatus),
    Failed(SessionFailure), Crashed(String),
}

pub struct ImportPreviewView {
    pub id: ImportPreviewId,
    pub source: ImportSourceKind,
    pub groups: Vec<ConnectionGroup>,
    pub candidates: Vec<ImportCandidateView>,
    pub warnings: Vec<ImportWarningView>,
}
```

`ImportSourceKind` 只有 `LegacyRshellJson`、`OpenSshConfig`；`ImportPreviewId`/`ImportCandidateId` 是 UUID newtype。`ImportCandidateView` 只含显示字段、`has_secret: bool`、`selectable: bool` 和 warnings；不含 secret。`AppFailure`/`SessionFailure` 只允许 `Validation`、`Storage`、`Vault`、`HostKey`、`Authentication`、`Network`、`Pty`、`Subprocess`、`Platform`、`Backpressure`、`Crashed` 分类及脱敏 context。

`rshell-storage` 的秘密与导入边界：

```rust
pub trait CredentialVault: Send + Sync {
    fn get(&self, key: &CredentialRef) -> Result<Option<SecretString>, VaultError>;
    fn put(&self, key: &CredentialRef, value: &SecretString) -> Result<(), VaultError>;
    fn delete(&self, key: &CredentialRef) -> Result<(), VaultError>;
}

pub struct ImportPreview {
    pub id: ImportPreviewId,
    pub source: ImportSource,
    pub groups: Vec<ConnectionGroup>,
    pub connections: Vec<ImportCandidate>,
    pub warnings: Vec<ImportWarning>,
    secrets: ImportSecretBag,
}

pub enum ImportSource {
    LegacyRshellJson { primary: PathBuf, recovered_from_backup: bool },
    OpenSshConfig { root: PathBuf },
}

pub struct ImportCandidate {
    pub id: ImportCandidateId,
    pub profile: ConnectionProfile,
    pub source_label: String,
    pub selectable: bool,
    pub warnings: Vec<ImportWarning>,
}

pub enum ImportWarning {
    RecoveredFromBackup,
    SecurityPolicyUpgraded,
    WildcardTemplate,
    DependsOnOpenSshConfig,
    MultipleIdentityFiles,
    UnsupportedDirective(String),
}

pub struct ImportReport {
    pub imported_groups: usize,
    pub imported_connections: usize,
    pub skipped_candidates: usize,
}

pub trait Importer: Send + Sync {
    fn preview(&self, source: &Path) -> Result<ImportPreview, ImportError>;
}

impl CredentialCoordinator {
    pub fn apply_catalog(
        &self,
        mutation: CatalogMutation,
        secret: SecretUpdate,
    ) -> Result<ConnectionCatalog, CredentialOperationError>;
    pub fn commit_import(
        &self,
        preview: ImportPreview,
        selected: &BTreeSet<ImportCandidateId>,
    ) -> Result<ImportReport, ImportError>;
    pub fn reconcile(&self) -> Result<ReconcileReport, CredentialOperationError>;
}
```

`apply_catalog` 是所有 create/update/duplicate/move/delete/group/tag mutation 的唯一应用层写入口。对 delete/clear/update，SQLite 的可见引用变更与最后一个引用对应的 `delete_old/prepared` journal 插入必须位于同一个 `BEGIN IMMEDIATE` 事务；事务提交后再操作 vault，成功后清 journal，失败或崩溃由 `reconcile` 幂等完成。UI/ApplicationService 不得直接调用 repository 的低级 `apply` 绕过 coordinator。

`ImportSecretBag` 是 storage-private、不可 clone/serialize/debug 的 `BTreeMap<ImportCandidateId, SecretString>`。`rshell-storage::ports::ImportPortAdapter` 独占 `BTreeMap<ImportPreviewId, PendingImport { preview: ImportPreview, expires_at: Instant }>`：preview 时保存完整对象并只向 core/UI 返回 `ImportPreviewView`；commit 先按 ID 从 map 中一次性 remove 再提交；`cancel` 立即 remove；每次 port 调用和 60 秒清理 tick 都清除超过 15 分钟的项。ApplicationService 只保留 view/ID，从不持有 `ImportPreview` 或 secret bag；被 remove/过期的对象 drop 时秘密立即 zeroize。`VaultError` 精确分类 `Unavailable/NoEntry/Denied/Platform`；`ImportError` 精确分类 `Read/Parse/Validation/Conflict/Vault/Storage/AlreadyImported/ReconciliationRequired/PreviewExpired`；二者的 `Display` 不含源文件 secret。

Task 2 把 terminal size/input/search/selection、immutable render frame、session state/failure 和 interaction request/response 定义在 `rshell-core`；`rshell-session` 只消费这些纯值类型。`rshell-session` 自己定义的 adapter 与 actor 接口如下：

```rust
pub trait TerminalEngine: Send {
    fn advance(&mut self, bytes: &[u8]) -> Result<EngineDelta, EngineError>;
    fn resize(&mut self, size: TerminalSize) -> Result<(), EngineError>;
    fn render(&mut self, viewport: Viewport) -> Result<Arc<RenderFrame>, EngineError>;
    fn encode_input(&mut self, input: TerminalInput) -> Result<Vec<u8>, EngineError>;
    fn encode_mouse(&mut self, input: TerminalMouseEvent) -> Result<Vec<u8>, EngineError>;
    fn scroll(&mut self, delta_rows: i32) -> Result<(), EngineError>;
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchMatch>, EngineError>;
    fn selected_text(&self, range: SelectionRange) -> Result<String, EngineError>;
}

#[async_trait]
pub trait SessionTransport: Send {
    fn capabilities(&self) -> TransportCapabilities;
    async fn connect(
        &mut self,
        request: &TransportRequest,
        interactions: InteractionBroker,
    ) -> Result<(), TransportError>;
    async fn next_event(&mut self) -> Result<TransportEvent, TransportError>;
    async fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    async fn resize(&mut self, size: TerminalSize) -> Result<(), TransportError>;
    async fn shutdown(&mut self) -> Result<(), TransportError>;
}

pub enum SessionCommand {
    Input(TerminalInput),
    Paste(SecretString),
    Resize(TerminalSize),
    Scroll(i32),
    Search(SearchQuery),
    Select(SelectionRange),
    CopySelection,
    Respond(InteractionId, InteractionResponse),
    Reconnect,
    Shutdown,
}

pub enum SessionEvent {
    StateChanged(SessionState),
    FrameReady(Arc<RenderFrame>),
    SearchCompleted(Vec<SearchMatch>),
    CopyReady(String),
    InteractionRequired(InteractionRequest),
    Exited(ExitStatus),
    Failed(SessionFailure),
    Crashed(String),
}

pub struct SessionClient {
    pub id: SessionId,
    pub commands: tokio::sync::mpsc::Sender<SessionCommand>,
    pub events: tokio::sync::broadcast::Receiver<SessionEvent>,
    pub frames: tokio::sync::watch::Receiver<Option<Arc<RenderFrame>>>,
}
```

上述共享值类型与 session-private 名称的精确定义（以代码注释分界 ownership）：

```rust
// rshell-core shared protocol types.
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub dpi: u32,
}

pub struct Viewport { pub top_stable_row: i64, pub rows: u16 }
pub struct SelectionRange { pub start: CellPosition, pub end: CellPosition, pub rectangular: bool }
pub struct SearchQuery { pub needle: String, pub case_sensitive: bool, pub regex: bool }
pub struct SearchMatch { pub start: CellPosition, pub end: CellPosition }
pub struct CellPosition { pub stable_row: i64, pub column: u16 }

pub struct RenderFrame {
    pub generation: u64,
    pub size: TerminalSize,
    pub viewport_top: i64,
    pub rows: Arc<[RenderRow]>,
    pub cursor: Option<RenderCursor>,
    pub title: String,
}
pub struct RenderRow { pub stable_row: i64, pub wrapped: bool, pub cells: Arc<[RenderCell]> }
pub struct RenderCell {
    pub text: String,
    pub width: u8,
    pub foreground: Color,
    pub background: Color,
    pub attributes: CellAttributes,
}
pub enum Color { Default, Ansi(u8), Rgb(u8, u8, u8) }
pub struct CellAttributes { pub bold: bool, pub italic: bool, pub underline: bool, pub strike: bool, pub reverse: bool }
pub struct RenderCursor { pub position: CellPosition, pub shape: CursorShape, pub visible: bool }
pub enum CursorShape { Block, Beam, Underline }
pub struct ExitStatus { pub code: Option<i32>, pub success: bool }
pub enum SessionFailure {
    Validation, Storage, Vault, HostKeyRejected, HostKeyChanged, Authentication,
    Network, Pty, SshChannel, Subprocess, Platform, Backpressure, Timeout, Crashed,
}
pub enum SessionState {
    Created, Connecting, AwaitingHostKey, AwaitingAuthentication, Connected,
    Reconnecting, Closing, Exited, Failed, Crashed,
}
pub enum TerminalInput {
    CommittedText(String),
    Key { code: KeyCode, modifiers: KeyModifiers },
}
pub struct TerminalMouseEvent {
    pub kind: MouseEventKind,
    pub button: Option<MouseButton>,
    pub cell: CellPosition,
    pub pixel_x: u32,
    pub pixel_y: u32,
    pub modifiers: KeyModifiers,
}
pub enum InteractionRequest {
    HostKey(HostKeyPrompt), Password(AuthPrompt), PrivateKeyPassphrase(AuthPrompt),
    KeyboardInteractive(KeyboardInteractivePrompt),
}
pub enum InteractionResponse {
    HostKey(HostKeyDecision), Secret(SecretString), Answers(Vec<SecretString>), Cancel,
}

// rshell-session private types.
pub struct EngineDelta { pub outbound: Vec<u8>, pub dirty: bool }
pub enum TransportRequest {
    Local(LocalLaunch),
    SystemSsh(SshLaunchRequest),
    NativeSsh(SshLaunchRequest),
}
pub struct SshLaunchRequest {
    pub profile: ConnectionProfile,
    pub terminal: ResolvedTerminalProfile,
    pub initial_size: TerminalSize,
    pub secret: Option<SecretString>,
}
pub enum TransportEvent { Output(Vec<u8>), Exit(ExitStatus), Failure(TransportError) }
pub struct TransportCapabilities {
    pub agent: bool,
    pub public_key: bool,
    pub managed_password: bool,
    pub keyboard_interactive: bool,
    pub host_key_prompt: bool,
}
pub enum LocalLaunch {
    DefaultShell,
    Command { program: PathBuf, args: Vec<OsString>, cwd: PathBuf, env: BTreeMap<OsString, OsString> },
}
pub struct InteractionBroker {
    request_tx: tokio::sync::mpsc::Sender<(InteractionId, InteractionRequest)>,
    pending: Arc<std::sync::Mutex<BTreeMap<InteractionId, tokio::sync::oneshot::Sender<InteractionResponse>>>>,
}
impl InteractionBroker {
    pub async fn request(&self, request: InteractionRequest) -> Result<InteractionResponse, TransportError>;
    pub fn respond(&self, id: InteractionId, response: InteractionResponse) -> Result<(), TransportError>;
}
```

`KeyCode/KeyModifiers/MouseEventKind/MouseButton` 是 core 中显式枚举的 GDK-independent 值；`HostKeyPrompt` 含 interaction ID、host、port、algorithm、SHA-256 和 `changed=false`，`AuthPrompt` 含 ID/label/echo，`KeyboardInteractivePrompt` 含 ID/name/instruction/ordered prompts。`EngineError` 只含 `InvalidSize/InvalidInput/Unsupported/Backend`；`TransportError` 只含 `Network/HostKey/Authentication/Pty/SshChannel/Subprocess/Timeout/Closed`。

`rshell-ui` 只持有此端口：

```rust
pub trait UiCommandPort: Send + Sync {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError>;
}

pub struct MainWindowInit {
    pub commands: Arc<dyn UiCommandPort>,
    pub events: async_channel::Receiver<AppEvent>,
    pub initial: AppViewModel,
}
```

### 0.3 文件总图

| 路径 | 责任 |
|---|---|
| `Cargo.toml` / `Cargo.lock` | workspace、精确依赖与唯一引擎选择 |
| `src/main.rs` | 进程运行时、迁移/reconcile、应用服务与 Relm4 组合根 |
| `crates/rshell-core/src/{connection,terminal,render,workspace,protocol,application,error}.rs` | 纯领域、终端值类型、用例、端口、事件协议 |
| `crates/rshell-storage/src/{database,migrations,catalog,vault,credentials,error}.rs` | SQLite 工作线程、事务、系统凭据库、恢复状态机 |
| `crates/rshell-storage/src/import/{legacy,openssh,mod}.rs` | 两种预览与原子导入 |
| `crates/rshell-session/src/engine/{mod,wezterm}.rs` 或 `engine/{mod,alacritty}.rs` | 唯一终端引擎 adapter |
| `crates/rshell-session/src/{actor,render,selection,auth,host_keys,manager,error}.rs` | actor、帧、交互、安全错误与会话管理 |
| `crates/rshell-session/src/transport/{mod,local,system_ssh,native_ssh}.rs` | 三种 transport |
| `crates/rshell-platform/src/{paths,process,shell,security,clipboard,error}.rs` | 平台差异、文件权限、默认 shell、便携运行时 |
| `crates/rshell-ui/src/{main_window,connection_sidebar,connection_editor,session_tab_bar,pane_host,terminal_view,settings_window,import_dialog,interaction_dialog}.rs` | 小型 Relm4 组件 |
| `resources/style.css` | 唯一 CSS 资源，通过 `include_str!` 嵌入 |
| `scripts/qa/p0-smoke.ps1` | agent 可执行的真实表面验收 |
| `.github/workflows/{ci,release}.yml` | 三平台质量门与打包 |

## 1. 依赖波次

| 波次 | 可执行任务 | 前置 | 波次完成门 |
|---|---|---|---|
| W0 | Task 1 | 无 | 新 workspace 的 core catalog 测试通过，旧 `src/` 已不参与构建 |
| W1 | Task 2 | 1 | 终端 profile、pane tree 和稳定协议通过纯测试 |
| W2 | Task 3、4、5 | 2（Task 4/5 只需 1） | 引擎已唯一选定；平台和 SQLite 基础可独立测试 |
| W3 | Task 6、9 | 5；2+3 | 凭据崩溃恢复与 fake transport actor 均通过 |
| W4 | Task 7、8、10、11、12 | 6；9；4 | 两种导入、本地/system SSH、host/auth 单元闭环通过 |
| W5 | Task 13 | 6+9+12 | native SSH 三种认证、严格 host key、PTY/resize 通过本地 server |
| W6 | Task 14 | 7+8+10+11+13 | 应用服务以 fake UI 走通 P0 命令/事件和启动流 |
| W7 | Task 15、16 | 14；3+14 | 连接 UI 与 TerminalView 分别通过 view-model/GTK 测试 |
| W8 | Task 17、18 | 15+16 | tab/split/reconnect/status、settings/import/auth UI 通过 |
| W9 | Task 19 | 17+18 | 新组合根启动；所有旧 Rust 实现被删除 |
| W10 | Task 20、21 | 19 | 真实 GTK/SSH/vault smoke 与三平台 CI/release 通过 |
| W11 | Task 22 | 20+21 | 全部 P0 验收、清理与文档一致性通过 |

---

### Task 1: 用经过测试的连接领域替换单包构建入口

**Dependencies:** 无。

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/rshell-core/Cargo.toml`
- Create: `crates/rshell-core/src/lib.rs`
- Create: `crates/rshell-core/src/error.rs`
- Create: `crates/rshell-core/src/connection.rs`
- Create: `crates/rshell-core/tests/connection_catalog.rs`
- Retain unbuilt for behavior reference: `src/*.rs`

**Interfaces:**
- Consumes: 设计第 4.1、5、7 节；旧 `ConnectionProfile` 的 JSON 字段/默认值只作为 Task 7 的迁移知识。
- Produces: `ConnectionId`、`GroupId`、`CredentialRef`、`ConnectionProfile`、`ConnectionGroup`、`ConnectionCatalog`、`CatalogMutation`、`DomainError`，以及 `ConnectionCatalog::{apply,search,validate}`。

- [ ] **Step 1: 写连接目录失败测试并同时建立 virtual workspace**

```rust
#[test]
fn crud_copy_move_tags_search_and_stable_order_are_one_value_unit() {
    let mut catalog = fixture_catalog();
    let created = catalog.apply(CatalogMutation::Create(profile("Prod", "db.example"))).unwrap().connection_id().unwrap();
    let copy = catalog.apply(CatalogMutation::Duplicate { source: created, destination: Some(group("Archive")) }).unwrap().connection_id().unwrap();
    catalog.apply(CatalogMutation::Move { connection: copy, destination: Some(group("Ops")), position: 0 }).unwrap();
    catalog.apply(CatalogMutation::SetTags { connection: copy, tags: BTreeSet::from(["critical".into(), "database".into()]) }).unwrap();
    assert_eq!(catalog.search("CRITICAL"), vec![copy]);
    assert_ne!(created, copy);
    assert_eq!(catalog.ordered_ids(Some(group("Ops"))), vec![copy]);
}

#[test]
fn rejects_invalid_destination_group_cycle_and_nonempty_group_delete() {
    assert!(matches!(profile("bad", "-oProxyCommand=x").validate(), Err(DomainError::InvalidHost)));
    assert!(matches!(profile_with_port(0).validate(), Err(DomainError::InvalidPort)));
    assert!(matches!(catalog_with_cycle().validate(), Err(DomainError::GroupCycle(_))));
    assert!(matches!(catalog_with_nonempty_group().delete_group(group("Ops")), Err(DomainError::GroupNotEmpty(_))));
}
```

- [ ] **Step 2: 运行测试证明失败**

Run: `cargo test -p rshell-core --test connection_catalog --locked`

Expected: FAIL，原因是 `rshell-core`/领域类型尚不存在；不得出现旧 `src/app.rs` 的编译输出。

- [ ] **Step 3: 实现 catalog 的完整不变量**

```rust
impl ConnectionCatalog {
    pub fn apply(&mut self, mutation: CatalogMutation) -> Result<CatalogOutcome, DomainError>;
    pub fn search(&self, query: &str) -> Vec<ConnectionId>;
    pub fn ordered_ids(&self, group: Option<GroupId>) -> Vec<ConnectionId>;
    pub fn delete_group(&mut self, group: GroupId) -> Result<(), DomainError>;
    pub fn validate(&self) -> Result<(), DomainError>;
}
```

实现规则固定为：trim 文本；host 非空且不得以 `-` 开头；port 为 `1..=65535`；名称/host/username/tag 使用 Unicode lowercase 搜索；Native 只允许 Password/PublicKey/KeyboardInteractive，System 只允许 Agent/PublicKey；PublicKey 必须有 identity path，Password 必须有 credential ref，KeyboardInteractive 可无预存 ref并由 UI prompt；复制生成新 UUID、保留同一只读 credential ref、名称后缀为 `copy`（Task 6 用引用计数保证最后一个 profile 删除后才删 vault entry）；position 在每组内重排为连续 `0..n`；父组不得形成环；非空组不可删除；删除连接同时删除其 tag 关联但不隐式删除组。根 `Cargo.toml` 此时只有 `[workspace]` 与 `rshell-core` member，旧根 package 从构建图中退出。

- [ ] **Step 4: 运行领域测试和边界回归**

Run: `cargo test -p rshell-core --locked`

Expected: PASS；测试输出包含上述 happy path 与边界测试，0 failed。

- [ ] **Step 5: 检查当前波次可独立构建**

Run: `cargo check --workspace --all-targets --locked`

Expected: exit 0；构建图只有 `rshell-core`，不编译旧根模块。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.toml Cargo.lock crates/rshell-core; if ($LASTEXITCODE -eq 0) { git commit -m "feat: establish connection domain workspace" -m "Replace the legacy build entry with a tested connection catalog and domain invariants." }`

Expected: 获得授权时生成一个 `feat:` commit；未授权时完全跳过。

---

### Task 2: 固定终端 profile、pane tree 与应用协议

**Dependencies:** Task 1。

**Files:**
- Create: `crates/rshell-core/src/terminal.rs`
- Create: `crates/rshell-core/src/workspace.rs`
- Create: `crates/rshell-core/src/protocol.rs`
- Create: `crates/rshell-core/src/render.rs`
- Modify: `crates/rshell-core/src/lib.rs`
- Modify: `crates/rshell-core/Cargo.toml`
- Test: `crates/rshell-core/tests/terminal_profiles.rs`
- Test: `crates/rshell-core/tests/pane_tree.rs`
- Test: `crates/rshell-core/tests/protocol_secrets.rs`

**Interfaces:**
- Consumes: Task 1 IDs/profile；`SecretString` 只出现在不派生 `Debug` 的命令中。
- Produces: `TerminalSettingsV1`、`TerminalOverrides`、`ResolvedTerminalProfile`、`PaneTree`、`SplitAxis`、`WorkspaceState`、`TerminalSize`、input/search/selection 值、`RenderFrame`、`SessionState/SessionFailure`、`UiCommand`、`AppEvent`、`InteractionRequest/Response`。

- [ ] **Step 1: 写 profile 合并、pane 变换和秘密脱敏失败测试**

```rust
#[test]
fn connection_overrides_merge_over_global_and_clamp_geometry() {
    let global = TerminalSettingsV1::default();
    let resolved = global.resolve(&TerminalOverrides { initial_rows: Some(0), initial_cols: Some(1000), font_size: Some(80.0), ..Default::default() });
    assert_eq!((resolved.cols, resolved.rows, resolved.font_size), (999, 1, 72.0));
}

#[test]
fn close_leaf_collapses_only_its_parent_split() {
    let tree = PaneTree::leaf(pane(1)).split(pane(1), SplitAxis::Horizontal, pane(2), 0.5).unwrap();
    assert_eq!(tree.close(pane(2)).unwrap(), PaneTree::leaf(pane(1)));
}

#[test]
fn secret_commands_never_expose_value_in_debug_output() {
    let command = UiCommand::ApplyCatalog { mutation: CatalogMutation::Update(draft().into_profile()), secret: SecretUpdate::Set("do-not-log".into()) };
    assert!(!format!("{command:?}").contains("do-not-log"));
}
```

- [ ] **Step 2: 运行三组测试证明失败**

Run: `cargo test -p rshell-core --tests --locked`

Expected: FAIL，缺少 terminal/workspace/protocol 类型。

- [ ] **Step 3: 实现版本化设置和二叉 pane tree**

```rust
pub struct TerminalSettingsV1 {
    pub terminal_type: String,
    pub initial_cols: u16,
    pub initial_rows: u16,
    pub scrollback_lines: usize,
    pub font_family: String,
    pub font_size: f32,
    pub color_scheme: ColorScheme,
    pub key_bindings: Vec<KeyBinding>,
    pub left_alt_as_meta: bool,
    pub right_alt_as_meta: bool,
    pub enable_csi_u: bool,
    pub enable_kitty_keyboard: bool,
    pub mouse_reporting: bool,
    pub scroll_on_output: bool,
    pub scroll_on_keypress: bool,
    pub answerback: String,
}

pub enum PaneTree {
    Leaf { pane_id: PaneId, session_id: Option<SessionId> },
    Split { axis: SplitAxis, ratio: f32, first: Box<PaneTree>, second: Box<PaneTree> },
}
```

默认保持旧行为知识：`xterm-256color`、120x36、6000 scrollback、15pt、左右 Alt-as-meta=true、CSI-u=false、Kitty keyboard=false、mouse reporting=true、scroll-on-output=true、scroll-on-keypress=false、answerback=`rsHell`；geometry clamp `1..=999`、scrollback `100..=1_000_000`、font `6..=72`。split ratio 只接受 `0.1..=0.9`；close 最后一个 pane 返回 `WorkspaceError::LastPane`，由 tab close 命令处理。

- [ ] **Step 4: 定义完整 P0 命令/事件，不让 UI 触及基础设施**

`UiCommand` 明确包含连接 create/update/duplicate/move/search/delete、local/connect、tab close、pane split/close、input/resize/scroll/search/select/copy/paste、reconnect、terminal profile save、两种 import preview/commit、host/auth response。`AppEvent` 明确包含 catalog/settings/workspace snapshot、session state/frame、可分类错误、import preview/report、interaction request；手写 `Debug` 时把 `SecretUpdate::Set`、paste 和 auth response 输出为 `[REDACTED]`。

- [ ] **Step 5: 运行测试和序列化稳定性检查**

Run: `cargo test -p rshell-core --locked`

Expected: PASS；profile round-trip 保留 `version: 1`，pane/property tests 通过，测试日志不含 fixture secret。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-core Cargo.lock; if ($LASTEXITCODE -eq 0) { git commit -m "feat: define terminal workspace protocol" -m "Add versioned terminal profiles, pane-tree invariants, and redacted UI commands and events." }`

Expected: 授权后提交；否则跳过。

---

### Task 3: 执行固定 WezTerm revision compatibility spike 并只保留胜出引擎

**Dependencies:** Task 2。

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/rshell-session/Cargo.toml`
- Create: `crates/rshell-session/src/lib.rs`
- Create: `crates/rshell-session/src/error.rs`
- Create: `crates/rshell-session/src/render.rs`
- Create: `crates/rshell-session/src/selection.rs`
- Create: `crates/rshell-session/src/engine/mod.rs`
- Create initially: `crates/rshell-session/src/engine/wezterm.rs`
- Conditional replacement on no-go: delete `crates/rshell-session/src/engine/wezterm.rs`, create `crates/rshell-session/src/engine/alacritty.rs`
- Test: `crates/rshell-session/tests/engine_contract.rs`
- Test fixture: `crates/rshell-session/tests/fixtures/vt/canary.json`
- Benchmark: `crates/rshell-session/benches/terminal_engine.rs`
- Create: `crates/rshell-session/TERMINAL_ENGINE.md`

**Interfaces:**
- Consumes: Task 2 `ResolvedTerminalProfile`、`RenderFrame/RenderRow/RenderCell`、`TerminalSize`、`Viewport`、`SelectionRange`、`SearchQuery/SearchMatch`；固定 `TerminalEngine` trait（第 0.2 节）。
- Produces: 唯一 `DefaultTerminalEngine`、session-private frame builder、`EngineDelta { outbound, dirty }`。

- [ ] **Step 1: 写与具体引擎无关的金丝雀测试**

```rust
#[test]
fn fixed_vt_fixture_covers_split_sequences_color_cursor_and_wide_cells() {
    let fixture = Fixture::load("tests/fixtures/vt/canary.json");
    let mut engine = DefaultTerminalEngine::new(fixture.size, fixture.profile).unwrap();
    for chunk in fixture.chunks { engine.advance(chunk.as_bytes()).unwrap(); }
    let frame = engine.render(Viewport::bottom()).unwrap();
    assert_eq!(frame.plain_text(), fixture.expected_text);
    assert_eq!(frame.cursor, fixture.expected_cursor);
    assert_eq!(frame.rows[1].cells[3].width, 2);
    assert_eq!(frame.rows[0].cells[0].foreground, Color::Ansi(1));
}

#[test]
fn selection_handles_wrapped_scrollback_and_wide_cell_boundaries() {
    let engine = fixture_engine_with_wrapped_wide_text();
    assert_eq!(engine.selected_text(selection_across_scrollback()).unwrap(), "first line\n宽字符");
}

#[test]
fn dirty_rows_mouse_resize_and_alternate_screen_obey_contract() {
    let result = run_contract_fixture("tests/fixtures/vt/canary.json");
    assert_eq!(result.dirty_rows, BTreeSet::from([0, 1, 23]));
    assert_eq!(result.mouse_bytes, b"\x1b[<0;10;5M");
    assert_eq!(result.scrollback_rows_after_resize, 1000);
    assert_eq!(result.primary_text_after_alt_exit, "primary screen");
}
```

`canary.json` 固定包含：被拆分的 CSI/OSC、16/256/RGB color、组合字符、宽字符、自动换行、主/备用屏、光标样式、SGR mouse、1000 行 scrollback、缩小/放大 resize 和全屏 TUI 进入/退出序列；expected frame/cursor/attribute/dirty rows 均写入 fixture。

- [ ] **Step 2: 以固定 revision 建立首选 adapter 并证明测试先失败**

```toml
wezterm-term = { git = "https://github.com/wez/wezterm.git", package = "wezterm-term", rev = "d69264df66fdcc928c7a30c673df108984fda821" }
wezterm-surface = { git = "https://github.com/wez/wezterm.git", package = "wezterm-surface", rev = "d69264df66fdcc928c7a30c673df108984fda821" }
```

Run: `cargo test -p rshell-session --test engine_contract --locked`

Expected: FAIL，adapter 尚未把公开 `Terminal::{new,advance_bytes,resize,screen,cursor_pos}`、`Line::visible_cells`、`Cell::{str,width,attrs}` 映射到契约。

- [ ] **Step 3: 实现 WezTerm adapter、选择、搜索与 immutable frame**

只使用该 revision 的公开 API；`TerminalConfiguration` 从 resolved profile 映射 scrollback、CSI-u、Kitty keyboard、mouse、answerback 和 color palette；writer collector 在每次 `advance`/input 后 drain 到 `EngineDelta.outbound`；dirty tracking 使用 line `SequenceNo`；selection 使用 stable row、wrapped flag、cell width，不调用 WezTerm GUI/render 私有层。`RenderFrame` 的 rows/cells 使用 `Arc<[T]>`，frame 发布后不再可变。

- [ ] **Step 4: 执行 correctness、三平台可编译性和性能门**

Run: `cargo test -p rshell-session --test engine_contract --locked`

Expected: 8 个 contract 场景全部 PASS。

Run: `cargo bench -p rshell-session --bench terminal_engine --locked`

Expected: release benchmark 连续 5 个 sample 均满足：100 MiB deterministic trace 中位吞吐不低于 40 MiB/s；120x40 全 dirty frame 的 p95 低于 16 ms；1000 行 scrollback correctness hash 与 fixture 固定 SHA-256 相同。

CI matrix 在 Task 21 运行 `cargo check -p rshell-session --all-targets --locked`，三个发布 target 必须都成功。

- [ ] **Step 5: 根据单一判据完成 go/no-go，不保留双实现**

GO 当且仅当：全部 contract 通过、只用公开 API、三 target 可构建、吞吐与 frame latency 两个性能门通过、无需 fork/upstream patch/target-specific workaround。任一项失败即 no-go：从 manifest 删除两个 WezTerm git dependency，删除 `engine/wezterm.rs`，固定 `alacritty_terminal = "=0.26.0"`，以 `Term::new`、`Processor::advance`、`Term::{resize,damage,reset_damage,selection_to_string}` 和 grid/cell API实现同一 trait，并重新运行同一 fixture/benchmark。`TERMINAL_ENGINE.md` 记录实际命令、数值、唯一选择和失败判据；最终树只能存在一个 adapter 文件和一个引擎 dependency。

- [ ] **Step 6: 验证没有条件编译的运行时双引擎**

Run: `cargo tree -p rshell-session --locked | rg 'wezterm-term|alacritty_terminal'`

Expected: 只匹配胜出引擎；若 WezTerm 胜出还必须显示 revision `d69264df66fdcc928c7a30c673df108984fda821` 的 lock source。

- [ ] **Step 7: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.toml Cargo.lock crates/rshell-session; if ($LASTEXITCODE -eq 0) { git commit -m "feat: select terminal engine adapter" -m "Lock the compatibility-tested terminal engine behind the rsHell render contract." }`

Expected: 授权后提交唯一 adapter；否则跳过。

---

### Task 4: 集中三平台路径、运行时、权限与 shell 能力

**Dependencies:** Task 1；可与 Task 3、5 并行。

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/rshell-platform/Cargo.toml`
- Create: `crates/rshell-platform/src/lib.rs`
- Create: `crates/rshell-platform/src/error.rs`
- Create: `crates/rshell-platform/src/paths.rs`
- Create: `crates/rshell-platform/src/process.rs`
- Create: `crates/rshell-platform/src/shell.rs`
- Create: `crates/rshell-platform/src/security.rs`
- Create: `crates/rshell-platform/src/clipboard.rs`
- Test: `crates/rshell-platform/tests/platform_contract.rs`

**Interfaces:**
- Consumes: app ID `io.github.hugefiver.rshell`。
- Produces: `PlatformPaths::discover()`、`ProcessRuntime::configure()`、`DefaultShell::discover()`、`restrict_to_current_user(&Path)`、`durable_replace_user_file(source: &Path, destination: &Path)`、`ClipboardPolicy`。

- [ ] **Step 1: 写平台 contract 失败测试**

```rust
#[test]
fn paths_are_separate_and_legacy_json_is_discoverable() {
    let p = PlatformPaths::for_test(temp.path());
    assert_eq!(p.database_file(), temp.path().join("rshell.db"));
    assert_eq!(p.legacy_connections(), temp.path().join("connections.json"));
    assert_ne!(p.cache_dir(), p.config_dir());
}

#[test]
fn database_permissions_exclude_other_users() {
    let file = create_and_restrict();
    assert!(current_platform_acl(file).only_current_user());
}
```

- [ ] **Step 2: 运行测试证明失败**

Run: `cargo test -p rshell-platform --locked`

Expected: FAIL，平台接口不存在。

- [ ] **Step 3: 实现唯一平台分支位置**

`paths.rs` 返回 config/data/cache、`rshell.db`、旧 `connections.json(.bak)`、OpenSSH config/known_hosts；`shell.rs` 优先 `RSHELL_SHELL`，Windows 回退 `powershell.exe`，Unix 回退 `$SHELL`/`/bin/sh`。`process.rs` 搬迁旧 Windows per-monitor-v2 DPI 与 portable GTK path/schema/pixbuf/fontconfig 配置，并保留必要的 GIO 抑制；其他 crate 不再出现平台业务 `cfg`。

`security.rs` 在 Unix 创建后设置 mode `0600`；Windows 用当前 process token SID 创建显式 DACL，移除 Everyone/Users 写权限，并在测试读取 ACL 复核。`durable_replace_user_file` 要求 source/destination 同目录，flush+fsync source、原子替换、fsync parent（平台允许时）并重新验证权限；故障前保留原 destination。`clipboard.rs` 固定 CRLF/LF paste 规范化、NUL 拒绝和 UTF-8 text MIME 优先级。

- [ ] **Step 4: 运行平台测试与 cfg 搜索**

Run: `cargo test -p rshell-platform --locked`

Expected: 当前平台全部 PASS。

Run: `rg -n '#\[cfg\((windows|target_os)' crates --glob '!rshell-platform/**'`

Expected: 当前仅有第三方生成内容时才可能输出；自有其他 crate 为 0 matches。

- [ ] **Step 5: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.toml Cargo.lock crates/rshell-platform; if ($LASTEXITCODE -eq 0) { git commit -m "feat: centralize platform services" -m "Add tested paths, shell discovery, process setup, file permissions, and clipboard policy." }`

Expected: 授权后提交；否则跳过。

---

### Task 5: 建立 SQLite 迁移、事务 repository 与可测试存储线程

**Dependencies:** Task 1、Task 4 的 `restrict_to_current_user`；可先用 test path 并在同波次集成。

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/rshell-storage/Cargo.toml`
- Create: `crates/rshell-storage/src/lib.rs`
- Create: `crates/rshell-storage/src/error.rs`
- Create: `crates/rshell-storage/src/database.rs`
- Create: `crates/rshell-storage/src/migrations.rs`
- Create: `crates/rshell-storage/src/catalog.rs`
- Create: `crates/rshell-storage/migrations/0001_initial.sql`
- Test: `crates/rshell-storage/tests/database.rs`
- Test: `crates/rshell-storage/tests/catalog.rs`

**Interfaces:**
- Consumes: Task 1 `ConnectionRepository/CatalogMutation`；Task 4 permissions。
- Produces: `SqliteRepository::open(path)`、所有 repository 方法、`DatabaseWorker::shutdown()`；内部 `credential_operations` 表供 Task 6。

- [ ] **Step 1: 写迁移、事务 rollback 和 catalog round-trip 失败测试**

```rust
#[test]
fn migration_is_monotonic_idempotent_and_records_version() {
    let db = open_temp();
    db.migrate().unwrap();
    db.migrate().unwrap();
    assert_eq!(db.schema_versions().unwrap(), vec![1]);
    assert_eq!(db.load_terminal_profiles().unwrap(), vec![TerminalProfile::p0_default()]);
}

#[test]
fn failed_multi_row_mutation_rolls_back_every_visible_change() {
    let repo = seeded_repo();
    repo.inject_failure_after_statement(2);
    assert!(repo.apply(batch_move_and_update()).is_err());
    assert_eq!(repo.load_catalog().unwrap(), original_catalog());
}
```

- [ ] **Step 2: 运行测试证明失败**

Run: `cargo test -p rshell-storage --test database --test catalog --locked`

Expected: FAIL，SQLite schema/repository 尚不存在。

- [ ] **Step 3: 创建精确 schema 和 BEGIN IMMEDIATE 写路径**

`0001_initial.sql` 创建设计列出的 `connection_groups`、`connections`、`connection_tags`、`terminal_profiles`、`app_settings`、`schema_migrations`，外加仅用于崩溃恢复的：

```sql
CREATE TABLE credential_operations (
    operation_id TEXT PRIMARY KEY NOT NULL,
    credential_ref TEXT NOT NULL,
    action TEXT NOT NULL CHECK(action IN ('put_new', 'delete_old')),
    state TEXT NOT NULL CHECK(state IN ('prepared', 'vault_applied')),
    created_at TEXT NOT NULL
);
```

启用 `PRAGMA foreign_keys=ON`、WAL、`busy_timeout=5000ms`；migration 1 以固定 UUID `00000000-0000-0000-0000-000000000001` 插入 Task 2 的 P0 default terminal profile，并让 `app_settings.default_terminal_profile` 引用它；每个 mutation 在 `transaction_with_behavior(TransactionBehavior::Immediate)` 内完成。`rusqlite::Connection` 只存在于一个 bounded-command storage thread，transaction 不跨线程；错误分类为 migration/constraint/io/busy/corrupt。

- [ ] **Step 4: 实现字段映射、versioned JSON 和约束**

`terminal_profiles.settings_json` 必须是含整数 `version: 1` 以及 Task 2 全部 terminal setting 字段的 JSON object；数据库只保存 `credential_ref`，任何 secret 字节写入参数都由测试 fail。`connections` 在设计列出的字段上增加 P0 必需的 `authentication TEXT NOT NULL`、`terminal_profile_id TEXT` 和 `terminal_overrides_json TEXT NOT NULL`，后两项分别关联全局 profile 和保存版本化连接覆盖；连接/group/tag CRUD、复制、移动、排序、search 结果与 Task 1 catalog 一致；所有 foreign key cascade 行为由测试显式断言。

- [ ] **Step 5: 运行存储测试并检查数据库内容**

Run: `cargo test -p rshell-storage --test database --test catalog --locked`

Expected: PASS；失败注入后 visible tables byte-for-byte 等于原 snapshot；DB mode/ACL 测试通过。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.toml Cargo.lock crates/rshell-storage; if ($LASTEXITCODE -eq 0) { git commit -m "feat: add transactional sqlite storage" -m "Persist the P0 catalog and settings through monotonic migrations and a single database worker." }`

Expected: 授权后提交；否则跳过。

---

### Task 6: 接入系统凭据库和可崩溃恢复的凭据操作日志

**Dependencies:** Task 5。

**Files:**
- Create: `crates/rshell-storage/src/vault.rs`
- Create: `crates/rshell-storage/src/credentials.rs`
- Modify: `crates/rshell-storage/src/lib.rs`
- Modify: `crates/rshell-storage/src/catalog.rs`
- Modify: `crates/rshell-storage/Cargo.toml`
- Modify: `Cargo.lock`
- Test: `crates/rshell-storage/tests/credentials.rs`
- Test: `crates/rshell-storage/tests/system_vault.rs`

**Interfaces:**
- Consumes: Task 5 DB worker/`credential_operations`；第 0.2 节 `CredentialVault`。
- Produces: `SystemCredentialVault`、`MemoryCredentialVault` fault fixture、`CredentialCoordinator::{apply_catalog,commit_import,reconcile}`。

- [ ] **Step 1: 写 happy、明确失败、结果未知和重启恢复测试**

```rust
#[test]
fn password_create_commits_reference_but_never_secret_to_sqlite_or_logs() {
    let secret = SecretString::from("vault-only-secret");
    let catalog = coordinator.apply_catalog(CatalogMutation::Create(new_password_profile()), SecretUpdate::Set(secret)).unwrap();
    let profile = catalog.connection_named("password profile").unwrap();
    assert!(vault.contains(profile.credential_ref.as_ref().unwrap()));
    assert!(!database_bytes().contains("vault-only-secret"));
    assert!(!captured_logs().contains("vault-only-secret"));
}

#[test]
fn crash_after_vault_put_reconciles_orphan_without_visible_connection() {
    let state = run_until(CrashPoint::AfterVaultPutBeforeCatalogCommit);
    let restarted = restart(state);
    restarted.coordinator.reconcile().unwrap();
    assert_eq!(restarted.repo.load_catalog().unwrap(), original_catalog());
    assert!(restarted.vault.is_empty());
}
```

另写：unchanged 不读取/重写凭据；复制 profile 共享 ref，删除/clear 其中一个时 vault 仍保留，最后一个引用移除后才清理；update 使用新 ref 切换，旧 ref 删除失败留 journal；`fail_after_mutation` 重启按 journal+vault 实态收敛；delete missing 幂等。

- [ ] **Step 2: 运行 credential 测试证明失败**

Run: `cargo test -p rshell-storage --test credentials --locked`

Expected: FAIL，vault/coordinator 不存在。

- [ ] **Step 3: 实现系统 keyring 与不可泄漏错误**

固定依赖：

```toml
keyring = { version = "=4.1.5", default-features = false, features = ["v1"] }
secrecy = "=0.10.3"
zeroize = "=1.9.0"
```

service 为 `io.github.hugefiver.rshell`，key 为 `credential_ref`；使用 `Entry::{new,set_secret,get_secret,delete_credential}`。`NoEntry` 映射 `Ok(None)`，其余映射可分类 `VaultError`；读取的 `Vec<u8>` 转成 `SecretString` 后立即 zeroize。所有含 secret 的类型不派生 `Debug/Serialize/Clone`，错误只含 operation/ref/error category。

- [ ] **Step 4: 实现 prepare → vault → finalize/reconcile 状态机**

create/import：先事务写 `put_new/prepared` journal，不插连接；写 vault；标 `vault_applied`；最后一个 SQLite 事务插连接并删除 journal。update：为新 secret 生成新 ref，完成 put 后在同一事务切换 profile ref，并在旧 ref 已无 profile 引用时插入 `delete_old/prepared`；事务提交后删除旧 secret 并清 journal。clear/delete：同一个事务移除可见引用并在引用计数变为 0 时插入 `delete_old/prepared`，提交后才删除 vault secret。move/group/tag/duplicate 同样走 `apply_catalog`，其中 duplicate 共享 ref 且不触碰 vault。任何进程中断由 `reconcile()` 幂等收敛；应用启动必须先 reconcile 再加载 catalog。

- [ ] **Step 5: 运行 fault matrix 和真实 vault probe 的编译门**

Run: `cargo test -p rshell-storage --test credentials --locked`

Expected: PASS；每个 crash point 都只收敛到 old 或 new 完整状态。

Run: `cargo test -p rshell-storage --test system_vault --no-run --locked`

Expected: PASS；真实 probe 已编译但默认不触碰用户凭据库。Task 20/21 在隔离 runner 上以随机 ref 运行 ignored probe 并自动删除。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.lock crates/rshell-storage; if ($LASTEXITCODE -eq 0) { git commit -m "feat: secure credentials in system vault" -m "Add redacted secrets and a crash-recoverable SQLite-to-vault operation state machine." }`

Expected: 授权后提交；否则跳过。

---

### Task 7: 预览并原子导入旧 `connections.json`

**Dependencies:** Task 6。

**Files:**
- Create: `crates/rshell-storage/src/import/mod.rs`
- Create: `crates/rshell-storage/src/import/legacy.rs`
- Modify: `crates/rshell-storage/src/lib.rs`
- Fixture: `crates/rshell-storage/tests/fixtures/legacy/valid.json`
- Fixture: `crates/rshell-storage/tests/fixtures/legacy/plaintext-password.json`
- Fixture: `crates/rshell-storage/tests/fixtures/legacy/corrupt.json`
- Fixture: `crates/rshell-storage/tests/fixtures/legacy/connections.json.bak`
- Test: `crates/rshell-storage/tests/import_legacy.rs`

**Interfaces:**
- Consumes: `CredentialCoordinator::commit_import`、Task 2 `TerminalOverrides`、旧字段映射。
- Produces: `LegacyJsonImporter`、`ImportPreview/ImportWarning/ImportReport` 的 legacy 实现。

- [ ] **Step 1: 写完整字段、备份恢复、明文迁移和全回滚失败测试**

```rust
#[test]
fn preview_is_pure_and_commit_preserves_ids_groups_auth_and_terminal_overrides() {
    let before = snapshot_all_state();
    let preview = importer.preview(fixture("valid.json")).unwrap();
    assert_eq!(snapshot_all_state(), before);
    let selected = preview.connections.iter().map(|candidate| candidate.id).collect();
    let report = coordinator.commit_import(preview, &selected).unwrap();
    assert_eq!(report.imported_connections, 3);
    assert_eq!(loaded("prod").terminal_overrides.scrollback_lines, Some(12000));
}

#[test]
fn vault_failure_on_second_plaintext_secret_rolls_back_entire_import() {
    vault.fail_before_call(2);
    let before = snapshot_all_state();
    let preview = importer.preview(fixture("plaintext-password.json")).unwrap();
    let selected = preview.connections.iter().map(|candidate| candidate.id).collect();
    assert!(coordinator.commit_import(preview, &selected).is_err());
    assert_eq!(snapshot_visible_state(), before.visible);
    restart_and_reconcile();
    assert_eq!(snapshot_all_state(), before);
}
```

- [ ] **Step 2: 运行测试证明失败**

Run: `cargo test -p rshell-storage --test import_legacy --locked`

Expected: FAIL，legacy importer 不存在。

- [ ] **Step 3: 实现精确旧 schema 与 preview**

解析 `folders[]` 及 `connections[]` 的 `id,name,folder_id,host,port,user,password,identity_file,remote_command,note,backend,accept_new_host,terminal`；缺省 port=22、backend=`system_open_ssh`，空文本 trim。认证映射固定为：任何非空旧 password→Native+Password；否则非空 identity→按旧 backend 映射 System/Native+PublicKey；否则 `system_open_ssh`→System+Agent；否则 `wez_term_ssh`→Native+KeyboardInteractive。旧 terminal 的 type/cols/rows/scrollback/color/font/Alt/CSI-u/Kitty keyboard/mouse/scroll/answerback 映射到同名 Task 2 overrides，delete/backspace 映射到 key bindings；P0 不支持的旧 Kitty graphics flag 产生 warning 且不启用。旧 `accept_new_host` 不转换为弱策略，全部变为 `HostKeyPolicy::Strict` 并产生安全升级 warning。旧 password 只进入 `SecretString`/import secret bag；preview 返回 `has_secret: true`，不返回内容。primary 缺失/损坏时可预览 `.bak` 并附 recovery warning；二者都坏则纯失败。

- [ ] **Step 4: 实现全批次 journal + 单事务 catalog commit**

所有 ID 冲突在 vault 写前检测；每个明文 secret 使用新 `CredentialRef`；全部 put 成功后，一个 `BEGIN IMMEDIATE` 插入 groups/connections/tags/profile overrides、写入 `app_settings` key `import.legacy.sha256:<hex-digest>` 并清 `put_new` journal。任一 vault/constraint/commit 失败都不改变 visible catalog；reconcile 清理 orphan。再次导入同一 source fingerprint 返回 `ImportError::AlreadyImported`，避免静默重复。

- [ ] **Step 5: 运行测试并对 fixture 扫描数据库/日志**

Run: `cargo test -p rshell-storage --test import_legacy --locked`

Expected: PASS；valid、backup、duplicate、bad UUID、bad port、credential failure、DB failure 全通过，数据库文件/日志不含 fixture password。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-storage; if ($LASTEXITCODE -eq 0) { git commit -m "feat: import legacy connection data" -m "Preview and atomically migrate legacy JSON, terminal overrides, and plaintext credentials." }`

Expected: 授权后提交；否则跳过。

---

### Task 8: 静态解析并原子导入 OpenSSH config

**Dependencies:** Task 6。

**Files:**
- Create: `crates/rshell-storage/src/import/openssh.rs`
- Modify: `crates/rshell-storage/src/import/mod.rs`
- Fixture: `crates/rshell-storage/tests/fixtures/openssh/config`
- Fixture: `crates/rshell-storage/tests/fixtures/openssh/included.conf`
- Fixture: `crates/rshell-storage/tests/fixtures/openssh/cycle-a.conf`
- Fixture: `crates/rshell-storage/tests/fixtures/openssh/cycle-b.conf`
- Test: `crates/rshell-storage/tests/import_openssh.rs`

**Interfaces:**
- Consumes: `Importer`/`ImportPreview`、catalog transaction。
- Produces: `OpenSshConfigImporter` 和 `OpenSshCandidate { host_pattern, host_name, user, port, identity_file, proxy_jump, importable }`。

- [ ] **Step 1: 写 precedence、wildcard、include、ProxyJump 和 rollback 测试**

```rust
#[test]
fn literal_hosts_import_and_wildcards_remain_preview_templates() {
    let preview = importer.preview(fixture("config")).unwrap();
    assert!(preview.connections.iter().any(|c| c.name == "production" && c.host == "10.0.0.8"));
    assert!(preview.connections.iter().any(|c| c.pattern == "*.corp" && !c.importable));
}

#[test]
fn proxyjump_candidate_is_system_only_and_retains_host_alias() {
    let c = preview_candidate("bastion-target");
    assert_eq!(c.profile.transport, TransportKind::SystemOpenSsh);
    assert_eq!(c.profile.host, "bastion-target");
    assert!(c.warnings.contains(&ImportWarning::DependsOnOpenSshConfig));
}
```

- [ ] **Step 2: 运行测试证明失败**

Run: `cargo test -p rshell-storage --test import_openssh --locked`

Expected: FAIL，OpenSSH parser 不存在。

- [ ] **Step 3: 实现 P0 静态解析边界**

支持 comments、quoted values、`Host`、`HostName`、`User`、`Port`、多 `IdentityFile`（P0 选第一项并 warning）、`ProxyJump`、`Include`；OpenSSH “first obtained value wins” 应用 global + matching block。Include 相对当前 config 目录解析、canonical path 去重、最大深度 8、cycle 直接错误。含 `* ? !` 的 Host 只生成不可勾选模板 preview。`Match`、`ProxyCommand` 和动态 token 产生精确 warning，不自行执行。

含 ProxyJump 的 literal host 只允许以 `SystemOpenSsh` 导入，`profile.host` 保存原 `Host` alias，让 system OpenSSH 继续从原 config 应用跳板；preview 明示依赖原文件。无 ProxyJump 的候选保存解析后的 `HostName/User/Port/IdentityFile`。这只保存/委托导入语义，不实现 P1 proxy 配置。

- [ ] **Step 4: 使用同一 coordinator 原子提交**

导入前验证 host/port/重复名称/ID；该来源没有 secret，不触碰 vault；全部连接在单个 `BEGIN IMMEDIATE` 中插入。任一候选失败时整批不落库；用户可在 preview 取消具体候选后重新提交一份不可变 selection。

- [ ] **Step 5: 运行 fixture 与注入攻击测试**

Run: `cargo test -p rshell-storage --test import_openssh --locked`

Expected: PASS；quoted path、IPv6、include cycle、wildcard、option-like host、port 0/65536、ProxyCommand 字符串均有确定结果，且 parser 从不启动进程。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-storage; if ($LASTEXITCODE -eq 0) { git commit -m "feat: import openssh configurations" -m "Preview static OpenSSH hosts, preserve system-only jump semantics, and commit selected entries atomically." }`

Expected: 授权后提交；否则跳过。

---

### Task 9: 建立无共享终端锁的 bounded session actor

**Dependencies:** Task 2、3。

**Files:**
- Create: `crates/rshell-session/src/actor.rs`
- Create: `crates/rshell-session/src/manager.rs`
- Create: `crates/rshell-session/src/transport/mod.rs`
- Modify: `crates/rshell-session/src/lib.rs`
- Modify: `crates/rshell-session/src/error.rs`
- Test: `crates/rshell-session/tests/actor_lifecycle.rs`
- Test support: `crates/rshell-session/tests/support/fake_transport.rs`

**Interfaces:**
- Consumes: `TerminalEngine`、`SessionCommand/Event`。
- Produces: `SessionTransport`、`TransportFactory`、`SessionManager::{launch,command,shutdown_all}`、`SessionClient`；Task 14 的 core `SessionPort` adapter 把 client event/frame receivers 暴露为 `SessionBinding`。

- [ ] **Step 1: 写生命周期、resize 顺序、frame coalescing、幂等 close 和 panic 测试**

```rust
#[tokio::test]
async fn actor_orders_engine_resize_before_transport_resize() {
    let log = SharedLog::default();
    let client = launch_fake(log.clone()).await;
    client.commands.send(SessionCommand::Resize(size(132, 43))).await.unwrap();
    assert_eq!(log.wait_for(2).await, ["engine.resize(132,43)", "transport.resize(132,43)"]);
}

#[tokio::test]
async fn continuous_output_keeps_latest_frame_and_never_exceeds_refresh_rate() {
    let client = launch_burst(10_000).await;
    let frames = collect_for(Duration::from_millis(250), client.frames).await;
    assert!(frames.len() <= 16);
    assert_eq!(frames.last().unwrap().plain_text(), "line 10000");
}

#[tokio::test]
async fn transport_panic_becomes_crashed_event_and_runtime_survives() {
    let manager = manager_with_panicking_first_transport();
    let mut first = manager.launch(test_request()).await.unwrap();
    assert!(matches!(next_event(&mut first).await, SessionEvent::Crashed(_)));
    let mut second = manager.launch(test_request()).await.unwrap();
    assert!(matches!(next_event(&mut second).await, SessionEvent::StateChanged(SessionState::Connected)));
}
```

- [ ] **Step 2: 运行 actor 测试证明失败**

Run: `cargo test -p rshell-session --test actor_lifecycle --locked`

Expected: FAIL，actor/transport/manager 不存在。

- [ ] **Step 3: 实现单 owner 与 bounded channels**

每个 actor 独占 `Box<dyn SessionTransport>`、`Box<dyn TerminalEngine>`、selection/search/viewport/lifecycle；command channel capacity=128，event broadcast capacity=64，frame 使用 `watch<Option<Arc<RenderFrame>>>` 丢弃过时帧。输出 dirty 后按最多 60 Hz publish，state/interaction/exit 不 coalesce。queue full 返回 `SessionFailure::Backpressure`，输入不静默丢弃。

- [ ] **Step 4: 实现状态机和异常隔离**

状态只能按 `Created→Connecting→{AwaitingHostKey|AwaitingAuthentication|Connected}→{Reconnecting|Closing|Exited|Failed|Crashed}` 转移；Reconnect 总是先 shutdown 旧 transport 再由 factory 新建；Shutdown 幂等且 await child/channel completion。manager 监控 `JoinError` 并发 `Crashed`，保持 runtime 可启动其他 session。任何 UI-facing type 都不暴露 terminal/master mutex 或回调闭包。

- [ ] **Step 5: 运行 lifecycle 和 loom/property 边界**

Run: `cargo test -p rshell-session --test actor_lifecycle --locked`

Expected: PASS；重复 close/reconnect、queue full、EOF/nonzero exit、panic、10k burst 全部稳定，无 hang。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-session; if ($LASTEXITCODE -eq 0) { git commit -m "feat: add isolated session actors" -m "Own transport and terminal state per actor with bounded commands, coalesced frames, and crash events." }`

Expected: 授权后提交；否则跳过。

---

### Task 10: 接通 LocalPtyTransport 的真实输入、输出、resize 与清理

**Dependencies:** Task 4、9。

**Files:**
- Create: `crates/rshell-session/src/transport/local.rs`
- Modify: `crates/rshell-session/src/transport/mod.rs`
- Modify: `crates/rshell-session/Cargo.toml`
- Modify: `Cargo.lock`
- Test: `crates/rshell-session/tests/local_pty.rs`
- Fixture binary source: `crates/rshell-session/tests/fixtures/pty_echo.rs`

**Interfaces:**
- Consumes: `DefaultShell`、`SessionTransport`、`TerminalSize`。
- Produces: `LocalPtyTransport::launch(LocalLaunch)`；`LocalLaunch::{DefaultShell,Command { program,args,cwd,env }}`。

- [ ] **Step 1: 写跨平台 echo、color/wide bytes、resize、EOF 与重复关闭测试**

```rust
#[tokio::test]
async fn local_pty_round_trips_input_and_reports_resize_before_clean_exit() {
    let mut t = LocalPtyTransport::launch(fixture_command()).await.unwrap();
    t.connect(&request(size(80, 24)), broker()).await.unwrap();
    t.write(b"hello\r").await.unwrap();
    assert!(next_output(&mut t).await.contains_bytes(b"hello"));
    t.resize(size_with_pixels(100, 30, 900, 600)).await.unwrap();
    assert_eq!(next_resize_report(&mut t).await, (100, 30));
    t.shutdown().await.unwrap();
    t.shutdown().await.unwrap();
}
```

- [ ] **Step 2: 运行测试证明失败**

Run: `cargo test -p rshell-session --test local_pty --locked`

Expected: FAIL，LocalPtyTransport 不存在。

- [ ] **Step 3: 实现 portable PTY bridge**

通过 `portable-pty` 启动 child；设置 `TERM` 为 resolved profile terminal type；default shell 使用 platform 服务；显式 argv/cwd/env，不经 shell 拼接。blocking reader 在 actor 私有 helper 中只拥有 reader handle 并向 bounded transport-event channel 送 bytes；master/child/control 保留在 transport。resize 使用 checked u16/u32 转换，拒绝 0/overflow；shutdown 发送 EOF、等待限时、随后 kill+wait，确保无 zombie。

- [ ] **Step 4: 运行测试和重复资源泄漏检查**

Run: `cargo test -p rshell-session --test local_pty --locked -- --test-threads=1`

Expected: PASS；循环 100 次 launch/resize/shutdown 后 child count 回到基线，Windows broken-pipe/995 与 Unix EIO 映射正常 EOF。

- [ ] **Step 5: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.lock crates/rshell-session; if ($LASTEXITCODE -eq 0) { git commit -m "feat: launch local pty sessions" -m "Bridge portable PTYs into session actors with ordered resize and deterministic child cleanup." }`

Expected: 授权后提交；否则跳过。

---

### Task 11: 接入安全 argv 的 system OpenSSH transport

**Dependencies:** Task 4、9。

**Files:**
- Create: `crates/rshell-session/src/transport/system_ssh.rs`
- Modify: `crates/rshell-session/src/transport/mod.rs`
- Test: `crates/rshell-session/tests/system_ssh.rs`

**Interfaces:**
- Consumes: `ConnectionProfile`、`SessionTransport`、platform executable lookup。
- Produces: `SystemOpenSshTransport`、`build_system_ssh_argv(&ConnectionProfile) -> Result<Vec<OsString>, TransportError>`。

- [ ] **Step 1: 写 argv、capabilities 与注入邻近回归测试**

```rust
#[test]
fn argv_is_strict_separate_and_places_destination_after_option_terminator() {
    let argv = build_system_ssh_argv(&profile("user", "host", 2222, Some("printf 'a b'"))).unwrap();
    assert_eq!(argv, ["-p","2222","-o","StrictHostKeyChecking=ask","--","user@host","printf 'a b'"].map(OsString::from).to_vec());
}

#[test]
fn option_like_host_nul_and_newline_are_rejected_not_escaped() {
    for host in ["-oProxyCommand=bad", "good\nProxyCommand bad", "a\0b"] {
        assert!(build_system_ssh_argv(&profile_host(host)).is_err());
    }
}

#[test]
fn system_transport_advertises_agent_but_not_managed_password() {
    assert_eq!(caps(), TransportCapabilities { agent: true, public_key: true, managed_password: false, keyboard_interactive: false, host_key_prompt: true });
}
```

- [ ] **Step 2: 运行测试证明失败**

Run: `cargo test -p rshell-session --test system_ssh --locked`

Expected: FAIL，builder/transport 不存在。

- [ ] **Step 3: 实现 system ssh PTY transport**

program 固定从 platform lookup 得到 `ssh`/`ssh.exe`；参数使用 `CommandBuilder`/`OsString` 独立加入：非 22 `-p`；identity file 用 `-i path -o IdentitiesOnly=yes`；严格 host key 用 `-o StrictHostKeyChecking=ask`；`--` 后为 `[user@]host` 和可选单个 remote command arg。不得传保存密码、不得写 askpass helper；agent/config/企业认证由系统 OpenSSH 原生继承。PTY bridge、resize、shutdown 与 local transport 共享私有 helper，而不是复制生命周期逻辑。

- [ ] **Step 4: 运行单元和 fake executable 集成测试**

Run: `cargo test -p rshell-session --test system_ssh --locked`

Expected: PASS；fake executable 记录 argv 原样，空格/引号/分号不在本地展开；nonzero exit 分类为 `Subprocess`，正常 EOF 为 `Exited`。

- [ ] **Step 5: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-session; if ($LASTEXITCODE -eq 0) { git commit -m "feat: add system openssh transport" -m "Launch OpenSSH through strict argv construction while preserving agent and user-config behavior." }`

Expected: 授权后提交；否则跳过。

---

### Task 12: 固定严格主机密钥和认证交互协议

**Dependencies:** Task 4、6、9。

**Files:**
- Create: `crates/rshell-session/src/host_keys.rs`
- Create: `crates/rshell-session/src/auth.rs`
- Modify: `crates/rshell-session/src/lib.rs`
- Modify: `crates/rshell-session/src/error.rs`
- Modify: `crates/rshell-session/Cargo.toml`
- Modify: `Cargo.lock`
- Test: `crates/rshell-session/tests/host_keys.rs`
- Test: `crates/rshell-session/tests/auth.rs`

**Interfaces:**
- Consumes: `CredentialRef/SecretString`、platform known_hosts path、actor `InteractionBroker`。
- Produces: `KnownHostsVerifier::verify`、`AuthPlan::from_profile`、`HostKeyPrompt`、`KeyboardInteractivePrompt`。

- [ ] **Step 1: 写 known/unknown/changed 与秘密生命周期失败测试**

```rust
#[tokio::test]
async fn unknown_key_requires_algorithm_and_sha256_confirmation_before_learning() {
    let prompt = verifier.verify("host", 22, key()).await.unwrap_unknown();
    assert_eq!(prompt.algorithm, "ssh-ed25519");
    assert!(prompt.sha256.starts_with("SHA256:"));
    assert!(!known_hosts_contains(key()));
    broker.respond(prompt.id, HostKeyDecision::AcceptAndStore).await;
    assert!(known_hosts_contains(key()));
}

#[tokio::test]
async fn changed_key_is_rejected_without_accept_path() {
    assert!(matches!(verifier.verify("host", 22, changed_key()).await, Err(HostKeyError::Changed { .. })));
}

#[test]
fn auth_debug_and_error_never_contain_password_or_passphrase() {
    let secret = "never-print-this";
    let values = auth_debug_outputs(SecretString::from(secret));
    assert!(values.iter().all(|value| !value.contains(secret)));
    assert!(values.iter().all(|value| value.contains("[REDACTED]")));
}
```

- [ ] **Step 2: 运行测试证明失败**

Run: `cargo test -p rshell-session --test host_keys --test auth --locked`

Expected: FAIL，verifier/auth plan 不存在。

- [ ] **Step 3: 实现严格 verifier 与 interaction broker**

`russh::keys::known_hosts::check_known_hosts_path`：known=true 继续；unknown 发送含 host/port/algorithm/SHA-256 的 `InteractionRequired` 并只接受 `AcceptAndStore` 或 `Reject`；接受后在同目录创建受限 temp 副本、对 temp 调 `learn_known_hosts_path`、flush+fsync，再用 Task 4 `durable_replace_user_file` 替换 known_hosts，避免中断留下半行。`KeyChanged` 直接 `SessionFailure::HostKeyChanged`，P0 没有接受按钮。所有 profile 默认且唯一为 `HostKeyPolicy::Strict`。

- [ ] **Step 4: 实现 auth plan 和 prompt response**

Password 从 vault 一次读取后 move 给 native transport；PublicKey 传 path 和可选 passphrase secret；Agent 只可选择 system transport；KeyboardInteractive 将每个 prompt 的 echo flag/label 发 UI，非 echo 答案为 `SecretString`。认证结束或失败立即 drop/zeroize secret；结构化错误只含 auth kind、host 和 category。

- [ ] **Step 5: 运行安全测试和 known_hosts 权限测试**

Run: `cargo test -p rshell-session --test host_keys --test auth --locked`

Expected: PASS；未知 key 不响应时 timeout/reject，changed key 无接受路径，known_hosts mode/ACL 限当前用户，测试日志不含秘密。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.lock crates/rshell-session; if ($LASTEXITCODE -eq 0) { git commit -m "feat: enforce ssh trust and auth prompts" -m "Require explicit first-use host-key confirmation and redact all managed authentication secrets." }`

Expected: 授权后提交；否则跳过。

---

### Task 13: 接入单一 `russh` native transport 与本地 SSH server contract

**Dependencies:** Task 9、12。

**Files:**
- Create: `crates/rshell-session/src/transport/native_ssh.rs`
- Modify: `crates/rshell-session/src/transport/mod.rs`
- Modify: `crates/rshell-session/Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/rshell-session/tests/support/ssh_server.rs`
- Test: `crates/rshell-session/tests/native_ssh.rs`

**Interfaces:**
- Consumes: `SessionTransport`、`KnownHostsVerifier`、`AuthPlan`、interaction broker。
- Produces: `NativeSshTransport`，capabilities `{ managed_password:true, public_key:true, agent:false, keyboard_interactive:true, host_key_prompt:true }`。

- [ ] **Step 1: 写四条端到端认证/PTY 失败测试**

```rust
#[tokio::test]
async fn password_auth_confirms_unknown_key_then_echoes_and_resizes_pty() {
    let result = run_native_case(NativeCase::PasswordWithUnknownHost).await.unwrap();
    assert_eq!(result.prompt_count, 1);
    assert_eq!(result.echoed, b"password-ok\r\n");
    assert_eq!(result.last_size, (132, 43));
}

#[tokio::test]
async fn encrypted_private_key_uses_vault_passphrase_and_reuses_known_host() {
    let result = run_native_case(NativeCase::EncryptedKeyTwice).await.unwrap();
    assert_eq!(result.successful_connections, 2);
    assert_eq!(result.prompt_count, 1);
}

#[tokio::test]
async fn keyboard_interactive_round_trips_multiple_echo_flags() {
    let result = run_native_case(NativeCase::KeyboardInteractive).await.unwrap();
    assert_eq!(result.prompt_echo_flags, vec![true, false]);
    assert_eq!(result.server_answers, vec!["user-visible", "one-time-code"]);
}

#[tokio::test]
async fn wrong_password_changed_key_and_disconnect_map_to_distinct_failures() {
    assert_eq!(failure_for(NativeCase::WrongPassword).await, SessionFailure::Authentication);
    assert_eq!(failure_for(NativeCase::ChangedHostKey).await, SessionFailure::HostKeyChanged);
    assert_eq!(failure_for(NativeCase::ResetDuringConnect).await, SessionFailure::Network);
}
```

- [ ] **Step 2: 固定 library 并运行测试证明失败**

```toml
russh = { version = "=0.62.4", default-features = false, features = ["ring"] }
```

Run: `cargo test -p rshell-session --test native_ssh --locked`

Expected: FAIL，NativeSshTransport 不存在；依赖必须解析到 0.62.4。

- [ ] **Step 3: 实现 connect、认证与 PTY channel**

在 session runtime 的 Tokio thread 中调用 `russh::client::connect`；handler `check_server_key` 只委托 Task 12 verifier。按 AuthPlan 调用 `authenticate_password`、`load_secret_key` + `PrivateKeyWithHashAlg` + `authenticate_publickey`，或 keyboard-interactive start/respond。认证成功后 `channel_open_session`、`request_pty(true, term, cols, rows, pixel_width, pixel_height, &[])`、可选 `Channel::exec` 否则 `request_shell(true)`；channel stream 映射 output/input，resize 调 `window_change`，shutdown 顺序为 eof→close→disconnect。

- [ ] **Step 4: 实现可分类错误和 secret drop 观测**

DNS/timeout/connection reset→Network；fingerprint/changed→HostKey；auth false→Authentication；channel/PTY request→SshChannel；actor timeout→Timeout。测试使用 drop probe 证明认证完成后 transport 不再持有 password/passphrase；russh error Debug 不直接发 UI。

- [ ] **Step 5: 运行 native contract 与三平台 compile contract**

Run: `cargo test -p rshell-session --test native_ssh --locked -- --test-threads=1`

Expected: PASS；三认证、首次/再次 host key、resize、remote command、EOF 全通过。

Run: `cargo check -p rshell-session --all-targets --locked`

Expected: exit 0；无 OpenSSL/libssh2 link requirement。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.lock crates/rshell-session; if ($LASTEXITCODE -eq 0) { git commit -m "feat: add native ssh transport" -m "Connect russh password, key, and keyboard-interactive authentication to strict host verification and PTY sessions." }`

Expected: 授权后提交；否则跳过。

---

### Task 14: 组合 P0 应用用例、启动流和 UI command/event bus

**Dependencies:** Task 5–13。

**Files:**
- Create: `crates/rshell-core/src/application.rs`
- Modify: `crates/rshell-core/src/lib.rs`
- Modify: `crates/rshell-core/src/protocol.rs`
- Create: `crates/rshell-storage/src/ports.rs`
- Create: `crates/rshell-session/src/ports.rs`
- Modify: `crates/rshell-storage/src/lib.rs`
- Modify: `crates/rshell-session/src/lib.rs`
- Test: `crates/rshell-core/tests/application.rs`
- Test support: `crates/rshell-core/tests/support/fake_ports.rs`

**Interfaces:**
- Consumes: core-owned `ConnectionRepository`、`CredentialPort`、`ImportPort`、`SessionPort` trait objects，以及 composition root 已加载的 `AppBootstrapState { catalog, settings }`；storage/session 的 `ports.rs` 只负责为具体实现适配这些 core traits。
- Produces: `ApplicationService::start(AppDependencies, AppBootstrapState) -> Result<ApplicationHandle, AppError>`、`ApplicationHandle::{ui_port,event_receiver,initial_view_model,shutdown}`。

- [ ] **Step 1: 写启动、CRUD→launch、interaction、split/reconnect 和错误恢复测试**

```rust
#[tokio::test]
async fn initialized_application_opens_local_session_without_repeating_bootstrap() {
    let app = start_with(recording_ports(), bootstrap_state()).await.unwrap();
    assert_eq!(calls(), ["session.launch_local"]);
    assert_eq!(app.initial_view_model().workspace.tab_count(), 1);
}

#[tokio::test]
async fn new_local_tab_launches_before_committing_workspace_and_emits_snapshot() {
    let result = run_new_local_tab().await;
    assert_eq!(result.calls, ["session.launch_local", "workspace.commit", "event.workspace_changed"]);
    assert_eq!(result.workspace.tabs.len(), 2);
    assert_eq!(result.workspace.active_tab, Some(result.new_tab_id));
    assert_eq!(result.workspace.tabs[1].pane_tree.session_ids(), vec![result.new_session_id]);
}

#[tokio::test]
async fn connect_loads_secret_once_moves_it_to_session_and_forwards_only_redacted_events() {
    let result = run_connect_with_secret("application-secret").await;
    assert_eq!(result.vault_reads, 1);
    assert_eq!(result.session_launches, 1);
    assert!(result.event_debug.iter().all(|line| !line.contains("application-secret")));
}

#[tokio::test]
async fn session_binding_forwards_frame_and_interaction_response_to_same_actor() {
    let result = run_binding_interaction_round_trip().await;
    assert_eq!(result.forwarded_frame_generation, 7);
    assert_eq!(result.response_session, result.launched_session);
    assert_eq!(result.response_interaction, result.requested_interaction);
}

#[tokio::test]
async fn cancelling_import_drops_storage_owned_secret_preview() {
    let result = preview_then_cancel_secret_import().await;
    assert_eq!(result.core_secret_objects, 0);
    assert_eq!(result.storage_pending_previews, 0);
    assert!(result.secret_drop_observed);
}

#[tokio::test]
async fn failed_save_or_import_keeps_prior_view_model_and_emits_retryable_error() {
    let result = run_storage_failure_cases().await;
    assert_eq!(result.after_save_failure, result.before);
    assert_eq!(result.after_import_failure, result.before);
    assert!(result.failures.iter().all(|failure| failure.retryable));
}
```

- [ ] **Step 2: 运行 application tests 证明失败**

Run: `cargo test -p rshell-core --test application --locked`

Expected: FAIL，ApplicationService/ports 不存在。

- [ ] **Step 3: 实现单一 command loop 和 use-case dispatch**

ApplicationService 在非 GTK worker 上串行消费 `UiCommand`；所有 connection mutations 只调 core-owned `CredentialPort::apply_catalog`，成功后才发新 catalog snapshot，绝不直接调用 repository raw `apply`；Connect 解析 profile/profile override、通过 port 读取所需 credential、构造不可变 launch request并 move 给 SessionPort。每次 launch 返回 `SessionBinding`，ApplicationService 为该 binding 转发 state/interaction/exit events 与 latest frame 到 `AppEvent`；`UiCommand::Respond` 被转换成同一 session 的 `SessionUiCommand::Respond`，再由 SessionPort adapter 转成 actor `SessionCommand::Respond`。`UiCommand::NewLocalTab` 分配新 tab/pane ID，先调用 `SessionPort::launch_local`；只有 launch 成功才把绑定的 session ID 写入新 leaf、追加并激活 tab、发出 `WorkspaceChanged`，失败时 workspace 保持不变并发 `OperationFailed`。其他 tab/pane 状态同样先验证，session launch 成功后再提交 view model。Import preview 只在 view model 保存 view/ID；commit/cancel 调 `ImportPort`，窗口关闭和应用 shutdown 必须逐个 cancel 未提交 ID。UI command queue bounded=256；full 返回可见 busy error。

- [ ] **Step 4: 实现启动/关机和可恢复错误策略**

Task 14 只接收已经初始化的 `AppBootstrapState`，启动 command/event loop 并创建一个 local tab；platform configure→DB open/migrate→credential reconcile→load catalog/settings 的唯一实现和顺序测试属于 Task 19 composition root。P0 不持久化 workspace；每次正常启动均创建 local terminal。关闭先禁止新命令，再请求 sessions shutdown；storage worker 的最终关闭由 Task 19 composition root 在 ApplicationHandle 完成后执行。可恢复错误附 retry/edit connection action；不可恢复 session 错误保留 error pane 和可复制的脱敏诊断。

- [ ] **Step 5: 运行 app contract 和全 workspace 测试**

Run: `cargo test -p rshell-core --test application --locked`

Expected: PASS；每类 UiCommand 都有明确 event 或错误，fake port call order 固定。

Run: `cargo test --workspace --locked`

Expected: 当前全部 crate PASS。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-core crates/rshell-storage crates/rshell-session; if ($LASTEXITCODE -eq 0) { git commit -m "feat: orchestrate p0 application flows" -m "Route UI commands through transactional storage, credentials, and isolated session ports." }`

Expected: 授权后提交；否则跳过。

---

### Task 15: 建立真实 GTK shell、连接侧栏与编辑器

**Dependencies:** Task 14。

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/rshell-ui/Cargo.toml`
- Create: `crates/rshell-ui/src/lib.rs`
- Create: `crates/rshell-ui/src/command_port.rs`
- Create: `crates/rshell-ui/src/main_window.rs`
- Create: `crates/rshell-ui/src/connection_sidebar.rs`
- Create: `crates/rshell-ui/src/connection_editor.rs`
- Create: `crates/rshell-ui/src/view_model.rs`
- Test: `crates/rshell-ui/tests/connection_view_models.rs`
- Test: `crates/rshell-ui/tests/component_dependencies.rs`

**Interfaces:**
- Consumes: `MainWindowInit`、`UiCommand/AppEvent/AppViewModel`。
- Produces: `MainWindow`、`ConnectionSidebar`、`ConnectionEditor` Relm4 components。

- [ ] **Step 1: 写连接 UI reducer 和 secret-edit 失败测试**

```rust
#[test]
fn editor_validates_before_send_and_preserves_unchanged_secret() {
    let mut vm = editor_for(existing_password_profile());
    vm.password_field.clear();
    let command = vm.save_command().unwrap();
    assert!(matches!(command.secret_update(), Some(SecretUpdate::Unchanged)));
    vm.mark_password_edited();
    let command = vm.save_command().unwrap();
    assert!(matches!(command.secret_update(), Some(SecretUpdate::Clear)));
}

#[test]
fn sidebar_commands_cover_create_edit_copy_move_search_and_delete() {
    let commands = exercise_sidebar();
    assert_matches_exact_p0_catalog_commands(commands);
}
```

- [ ] **Step 2: 运行 UI tests 证明失败**

Run: `cargo test -p rshell-ui --test connection_view_models --test component_dependencies --locked`

Expected: FAIL，UI crate/components 不存在。

- [ ] **Step 3: 实现小组件和可访问连接工作流**

MainWindow 只组合 children；sidebar 呈现 group tree、tags、case-insensitive search、context actions；editor 有 name/host/port/user/transport/auth/identity/remote command/note/tags/profile override。port 输入只接受 1–65535；transport capability 禁用不支持组合（System+managed password/keyboard）；password/passphrase entry 不回填旧 secret、不保存到 view model snapshot、关闭即清空。

- [ ] **Step 4: 强制 UI dependency boundary**

`rshell-ui/Cargo.toml` 只依赖 core/platform/GTK/Relm4/PangoCairo，不依赖 storage/session/rusqlite/keyring/russh/portable-pty。`component_dependencies` 读取 `cargo metadata` 并断言此边界；组件所有动作只调用 `UiCommandPort::try_send`。

- [ ] **Step 5: 运行 reducer/component compile tests**

Run: `cargo test -p rshell-ui --locked`

Expected: PASS；CRUD/search/move/copy/delete、invalid form、unchanged/clear secret 均有测试。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.toml Cargo.lock crates/rshell-ui; if ($LASTEXITCODE -eq 0) { git commit -m "feat: add connection management ui" -m "Build Relm4 connection components over the command port without infrastructure coupling." }`

Expected: 授权后提交；否则跳过。

---

### Task 16: 实现 TerminalView 绘制、输入、滚动、搜索、选择与剪贴板

**Dependencies:** Task 3、14、15。

**Files:**
- Create: `crates/rshell-ui/src/terminal_view.rs`
- Create: `crates/rshell-ui/src/terminal_input.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `resources/style.css`
- Test: `crates/rshell-ui/tests/terminal_input.rs`
- Test: `crates/rshell-ui/tests/terminal_view_model.rs`
- Test: `crates/rshell-ui/tests/terminal_draw.rs`

**Interfaces:**
- Consumes: immutable `RenderFrame`、Terminal-related `UiCommand`、`ClipboardPolicy`。
- Produces: `TerminalView` 和 `TerminalViewModel::{apply_frame,key,mouse,resize,selection}`。

- [ ] **Step 1: 写键鼠映射、pixel/cell resize、frame replacement 和剪贴板边界测试**

```rust
#[test]
fn resize_reports_cells_and_pixels_without_zero_or_overflow() {
    let cmd = model(font_metrics(9, 18)).resize(901, 541, 2.0).unwrap();
    assert_eq!(cmd, UiCommand::Session { session: session(1), command: SessionUiCommand::Resize(size_with_pixels(100, 30, 1802, 1082, 192)) });
}

#[test]
fn stale_frame_is_dropped_and_newest_generation_draws_wide_cursor_correctly() {
    let mut model = TerminalViewModel::default();
    model.apply_frame(frame_with_wide_cursor(5));
    model.apply_frame(frame_with_text(4, "stale"));
    assert_eq!(model.frame().generation, 5);
    assert_eq!(model.cursor_rect().width, model.cell_width() * 2);
}

#[test]
fn paste_normalizes_newlines_rejects_nul_and_is_redacted() {
    let command = model().paste("a\r\nb").unwrap();
    assert!(!format!("{command:?}").contains("a\r\nb"));
    assert!(model().paste("a\0b").is_err());
    assert_eq!(extract_paste(command).expose_secret(), "a\nb");
}
```

- [ ] **Step 2: 运行 TerminalView tests 证明失败**

Run: `cargo test -p rshell-ui --test terminal_input --test terminal_view_model --test terminal_draw --locked`

Expected: FAIL，view/input mapper 不存在。

- [ ] **Step 3: 实现 Pango/Cairo immutable-frame renderer**

GTK DrawingArea 只读取最新 `Arc<RenderFrame>`；按 dirty rows queue draw；绘制 foreground/background、bold/italic/underline/strike/reverse、cursor、selection/search overlays 和 wide/combining cells。字体/颜色来自 resolved profile；frame generation 递增，旧 frame 丢弃。绘制函数不调用 engine/session/storage，不持锁。

- [ ] **Step 4: 实现输入与可访问交互**

GDK key→`TerminalInput` 保留 Ctrl/Alt/Shift/Super、IME committed text；mouse reporting mode 时发 terminal mouse，否则滚动 viewport；drag selection 按 cell 坐标发 Select；copy 等待 `CopyReady` 后写 GDK clipboard；paste 立即 wrap `SecretString` 后发送。Ctrl+Shift+F 打开 session search，Enter/Shift+Enter 导航 match；resize 顺序只发一个含 cell+pixel+DPI 的命令。

- [ ] **Step 5: 运行绘制测试和 GTK offscreen snapshot assertion**

Run: `cargo test -p rshell-ui --test terminal_input --test terminal_view_model --test terminal_draw --locked`

Expected: PASS；固定 frame 的 pixel hash、wide cell、selection/search/cursor、HiDPI resize 均符合 fixture；无 GTK critical warning。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-ui resources/style.css; if ($LASTEXITCODE -eq 0) { git commit -m "feat: render interactive terminal frames" -m "Draw immutable terminal frames and map GTK input, selection, search, resize, and clipboard commands." }`

Expected: 授权后提交；否则跳过。

---

### Task 17: 接通标签页、水平/垂直分屏、状态、关闭与重连

**Dependencies:** Task 16。

**Files:**
- Create: `crates/rshell-ui/src/session_tab_bar.rs`
- Create: `crates/rshell-ui/src/pane_host.rs`
- Modify: `crates/rshell-ui/src/main_window.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Test: `crates/rshell-ui/tests/workspace_view_model.rs`
- Test: `crates/rshell-core/tests/session_workflows.rs`

**Interfaces:**
- Consumes: `PaneTree/WorkspaceState`、session state/frame events、`UiCommand::NewLocalTab` 与 split/close/reconnect commands。
- Produces: `SessionTabBar`、`PaneHost`、`SessionPaneViewModel`。

- [ ] **Step 1: 写两个 tab、双方向 nested split、close/reconnect 回归测试**

```rust
#[test]
fn two_tabs_with_horizontal_and_vertical_splits_keep_independent_active_panes() {
    let workspace = workspace_with_two_split_tabs();
    assert_eq!(workspace.tabs.len(), 2);
    assert_eq!(workspace.tabs[0].pane_tree.axes(), vec![SplitAxis::Horizontal]);
    assert_eq!(workspace.tabs[1].pane_tree.axes(), vec![SplitAxis::Vertical]);
    assert_ne!(workspace.tabs[0].active_pane, workspace.tabs[1].active_pane);
}

#[test]
fn new_tab_button_uses_the_production_new_local_tab_command() {
    let emitted = click_new_tab_button();
    assert_eq!(emitted, UiCommand::NewLocalTab);
}

#[tokio::test]
async fn close_then_reconnect_never_reuses_old_actor_or_leaves_transport_alive() {
    let result = close_then_reconnect().await;
    assert_ne!(result.old_session_id, result.new_session_id);
    assert!(result.old_shutdown_completed_before_new_launch);
    assert_eq!(result.live_old_transports, 0);
}

#[test]
fn failed_session_stays_as_error_pane_with_retry_edit_and_copy_diagnostics_actions() {
    let pane = failed_pane(SessionFailure::Authentication);
    assert_eq!(pane.actions(), vec![PaneAction::Retry, PaneAction::EditConnection, PaneAction::CopyDiagnostics, PaneAction::Close]);
}
```

- [ ] **Step 2: 运行 workspace tests 证明失败**

Run: `cargo test -p rshell-ui --test workspace_view_model --locked; if ($LASTEXITCODE -eq 0) { cargo test -p rshell-core --test session_workflows --locked }`

Expected: FAIL，tab/pane components 或 workflows 不完整。

- [ ] **Step 3: 实现 pane tree 到 GTK Paned 的纯投影**

每个 `PaneTree::Split` 对应 horizontal/vertical `gtk::Paned`，ratio 变更回发状态但 P0 不持久化；leaf 持有一个 TerminalView 或 pending/error page。工具栏/菜单/快捷键的生产“新建标签” handler 只发送 `UiCommand::NewLocalTab`；收到 `WorkspaceChanged` 后才创建/激活可见 tab。active pane/tab 由 click/focus 更新；关闭 leaf 先发 session Shutdown，收到终止或 timeout 后折叠树；最后 leaf 关闭 tab。

- [ ] **Step 4: 实现状态与重连 UX**

tab/pane 显示 Connecting、AwaitingHostKey、AwaitingAuthentication、Connected、Reconnecting、Exited、Failed、Crashed；Retry 创建新 actor，旧 ID 不复用；Edit 选中对应 connection；Copy diagnostics 只复制分类、host、timestamp、error chain 中的脱敏文本。应用关闭调用 Task 14 shutdown all。

- [ ] **Step 5: 运行工作区回归和 actor leak assertions**

Run: `cargo test -p rshell-ui --test workspace_view_model --locked; if ($LASTEXITCODE -eq 0) { cargo test -p rshell-core --test session_workflows --locked }`

Expected: PASS；两 tab、H/V split、last pane、pending/error、close/reconnect、actor count 回零均通过。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-core crates/rshell-ui; if ($LASTEXITCODE -eq 0) { git commit -m "feat: add terminal tabs and splits" -m "Project pane trees into tabs and split views with deterministic close, reconnect, and status handling." }`

Expected: 授权后提交；否则跳过。

---

### Task 18: 完成 settings、导入、主机密钥和认证对话

**Dependencies:** Task 7、8、12、17。

**Files:**
- Create: `crates/rshell-ui/src/settings_window.rs`
- Create: `crates/rshell-ui/src/import_dialog.rs`
- Create: `crates/rshell-ui/src/interaction_dialog.rs`
- Modify: `crates/rshell-ui/src/main_window.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Test: `crates/rshell-ui/tests/settings_view_model.rs`
- Test: `crates/rshell-ui/tests/import_view_model.rs`
- Test: `crates/rshell-ui/tests/interaction_view_model.rs`

**Interfaces:**
- Consumes: terminal profiles/settings、ImportPreview/Report、InteractionRequest/Response。
- Produces: `SettingsWindow`、`ImportDialog`、`InteractionDialog`。

- [ ] **Step 1: 写 profile override、纯 preview、host/auth prompt 失败测试**

```rust
#[test]
fn connection_override_can_inherit_or_replace_each_global_terminal_field() {
    let overrides = exercise_every_override_toggle();
    assert_eq!(overrides.inherited_field_count(), 0);
    assert_eq!(overrides.explicit_field_count(), TerminalOverrides::FIELD_COUNT);
    assert_eq!(overrides.clear_all().explicit_field_count(), 0);
}

#[test]
fn import_preview_cannot_commit_wildcards_and_displays_all_warnings_before_confirm() {
    let vm = ImportViewModel::from(fixture_preview());
    assert!(!vm.candidate("*.corp").selectable);
    assert!(vm.visible_warnings().contains(&ImportWarningView::WildcardTemplate));
    assert_eq!(vm.commit_command().unwrap().selected, BTreeSet::from([candidate_id("production")]));
}

#[test]
fn changed_host_key_has_no_accept_action_and_unknown_key_shows_algorithm_and_sha256() {
    let changed = InteractionViewModel::host_key_changed();
    assert_eq!(changed.actions(), vec![InteractionAction::CopyDiagnostics, InteractionAction::Close]);
    let unknown = InteractionViewModel::unknown_host("ssh-ed25519", "SHA256:abc");
    assert_eq!(unknown.actions(), vec![InteractionAction::Reject, InteractionAction::AcceptAndStore]);
    assert_eq!(unknown.fingerprint(), "ssh-ed25519 SHA256:abc");
}

#[test]
fn keyboard_interactive_masks_non_echo_answers_and_clears_after_send() {
    let mut vm = keyboard_prompt_view(vec![true, false]);
    vm.set_answers(["visible", "secret"]);
    let command = vm.response_command().unwrap();
    assert_eq!(vm.answer_lengths(), vec![0, 0]);
    assert!(!format!("{command:?}").contains("secret"));
}
```

- [ ] **Step 2: 运行对话 view-model tests 证明失败**

Run: `cargo test -p rshell-ui --test settings_view_model --test import_view_model --test interaction_view_model --locked`

Expected: FAIL，对话组件不存在。

- [ ] **Step 3: 实现 settings 和连接覆盖**

全局 profile 编辑 terminal type、geometry、scrollback、font、scheme、keybindings、Alt-as-meta、CSI-u/Kitty keyboard、mouse reporting、scroll-on-output/key、answerback；连接 editor 每项有 inherit toggle。保存只发版本化 `SaveTerminalProfile`/`SaveSettings`；应用成功事件后才更新 UI。非法 key chord、重复 binding、超范围值在 UI 与 core 双重拒绝。

- [ ] **Step 4: 实现两种 import preview/commit UX**

文件选择由 platform service；preview 显示 source、groups、每候选、secret-present 标记和 warnings，不显示 secret；wildcard 不可选；ProxyJump 候选标记 “uses system OpenSSH config”；commit 期间禁用二次提交；失败保持 prior catalog，显示分类和 retry；成功显示 imported/skipped counts 并刷新 sidebar。用户取消或关闭 preview 对话时必须发送 `UiCommand::CancelImport`；收到 `PreviewExpired` 时清空 view 并要求重新 preview，不能复用已移除 ID。

- [ ] **Step 5: 实现安全 interaction 对话**

未知 key 对话固定显示 host:port、algorithm、SHA-256，按钮 Reject/Accept and store；changed key 错误页只有 Close/Copy diagnostics。Password/private-key passphrase/keyboard-interactive 非 echo 使用 password entry，值立即包装 secret 并清控件；取消发 Reject/Cancel response。对话关闭不记录输入历史。

- [ ] **Step 6: 运行全部 UI tests**

Run: `cargo test -p rshell-ui --locked`

Expected: PASS；settings/import/interaction 与既有 connection/terminal/tab tests 全通过。

- [ ] **Step 7: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-ui; if ($LASTEXITCODE -eq 0) { git commit -m "feat: complete p0 settings and dialogs" -m "Add terminal profiles, atomic import previews, and strict SSH trust and authentication interactions." }`

Expected: 授权后提交；否则跳过。

---

### Task 19: 切换新组合根并明确删除全部旧实现

**Dependencies:** Task 14–18。

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Replace: `src/main.rs`
- Delete: `src/lib.rs`
- Delete: `src/app.rs`
- Delete: `src/config.rs`
- Delete: `src/connection.rs`
- Delete: `src/credentials.rs`
- Delete: `src/ssh.rs`
- Delete: `src/storage.rs`
- Delete: `src/terminal.rs`
- Delete: `src/theme.rs`
- Delete: `resources/rshell.gresource.xml`
- Create: `crates/rshell-ui/src/startup_probe.rs`
- Test: `crates/rshell-ui/tests/startup.rs`

**Interfaces:**
- Consumes: `ProcessRuntime`、`SqliteRepository`、`SystemCredentialVault`、`CredentialCoordinator`、`SessionManager`、`ApplicationService`、`MainWindowInit`。
- Produces: root `rshell` binary、唯一进程 bootstrap、`--smoke-startup PATH` 真实 GTK 探针（`PATH` 是必填输出文件参数名，不是待填计划内容）。

- [ ] **Step 1: 写组合顺序和真实 GTK startup probe 失败测试**

```rust
#[test]
fn smoke_report_requires_realized_window_local_session_frame_and_clean_shutdown() {
    let report = run_smoke_startup();
    assert!(report.window_realized);
    assert!(report.local_session_connected);
    assert!(report.non_empty_render_frame);
    assert!(report.shutdown_clean);
}

#[test]
fn composition_root_bootstraps_each_process_service_exactly_once_in_order() {
    let report = run_instrumented_bootstrap();
    assert_eq!(report.calls, ["platform.configure", "storage.open", "storage.migrate", "credentials.reconcile", "catalog.load", "settings.load", "application.start"]);
}
```

- [ ] **Step 2: 运行 startup test 证明失败**

Run: `cargo test -p rshell-ui --test startup --locked`

Expected: FAIL，新 root/GTK probe 未组合。

- [ ] **Step 3: 添加最终 root package 并实现组合根**

根 `Cargo.toml` 同时包含 `[workspace]` 和 `[package] name="rshell"`，root dependency 只引用五个自有 crates、`anyhow`、`relm4`、`tracing`、`tracing-subscriber`。subscriber 只记录分类字段并应用统一 redaction；`main` 是唯一 bootstrap owner：`ProcessRuntime::configure()`→发现 paths→open/migrate SQLite→构造 vault/coordinator 并 `reconcile()`→load catalog/settings 形成 `AppBootstrapState`→构造 session/import/credential port adapters→`ApplicationService::start`→`RelmApp::new("io.github.hugefiver.rshell").run::<MainWindow>(init)`。任一步失败输出脱敏 category/context 并非零退出；每项初始化只能调用一次。

`--smoke-startup` 仍执行同一 GTK/Relm4/SQLite/local PTY/TerminalView 路径，只使用临时 config dir；窗口 realized 后等待第一个非空 frame，写 JSON report，正常关闭所有 actor/storage 后退出。它不是 fake UI，也不绕过组合根。

- [ ] **Step 4: 在本任务删除旧 Rust 文件和僵尸 GResource**

删除上列九个旧模块与 `src/lib.rs`，不做拆分式迁移、不从它们 re-export。保留 `resources/style.css`，UI 通过 `include_str!` 嵌入。根 Cargo 删除旧 `wezterm-ssh`、旧 WezTerm revision、`ssh2`/libssh2/OpenSSL 绑定和不再使用依赖；`Cargo.lock` 重新锁定最终 graph。

- [ ] **Step 5: 验证新组合根和旧代码零引用**

Run: `cargo check --workspace --all-targets --locked`

Expected: exit 0。

Run: `rg -n 'rshell::app|mod app|ConnectionStore|TerminalSessionHandle|wezterm_ssh|05343b' . --glob '!docs/**' --glob '!target/**'`

Expected: 0 matches。

- [ ] **Step 6: 运行真实 GTK startup probe**

Run: `$report = Join-Path $env:TEMP 'rshell-startup-report.json'; cargo run --locked -- --smoke-startup $report; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; $data = Get-Content -LiteralPath $report -Raw | ConvertFrom-Json; if (-not ($data.window_realized -and $data.local_session_connected -and $data.non_empty_render_frame -and $data.shutdown_clean)) { throw 'startup probe did not traverse the complete GTK path' }`

Expected: exit 0，四个字段均为 true，进程退出后无 child rshell/shell。

- [ ] **Step 7: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add Cargo.toml Cargo.lock src crates/rshell-ui resources; if ($LASTEXITCODE -eq 0) { git commit -m "feat: cut over to rebuilt application" -m "Wire the new workspace into the real GTK process and remove every legacy implementation module." }`

Expected: 授权后提交明确包含旧代码删除；否则跳过。

---

### Task 20: 建立 SSH、系统凭据库和 P0 真实表面 smoke

**Dependencies:** Task 19。

**Files:**
- Create: `crates/rshell-session/tests/ssh_smoke.rs`
- Modify: `crates/rshell-session/tests/support/ssh_server.rs`
- Modify: `crates/rshell-storage/tests/system_vault.rs`
- Create: `scripts/qa/p0-smoke.ps1`
- Create: `scripts/qa/assert-no-secrets.ps1`
- Create: `crates/rshell-ui/src/smoke_driver.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `crates/rshell-ui/src/main_window.rs`
- Modify: `src/main.rs`
- Create: `tests/fixtures/smoke/p0-scenario.json`
- Create: `tests/p0_acceptance.rs`

**Interfaces:**
- Consumes: 完整 root binary、system OpenSSH、native transport、真实 OS vault、startup probe。
- Produces: 单一 `scripts/qa/p0-smoke.ps1 -Mode Unit|Ssh|Gtk|Vault|All` 验收入口、root binary 的 `--smoke-p0 SCENARIO REPORT` 控制面和 JSON/JUnit evidence。

- [ ] **Step 1: 写 happy path 真实场景**

`p0_acceptance`/QA driver 必须依次证明：创建 Native password profile→随机 secret 进入真实 vault→SQLite byte scan 无 secret→首次 SHA-256 host prompt→明确接受→连接本地 russh server→执行 `printf` 并看到 frame；创建 encrypted-key profile 后 private-key+passphrase 连接；system OpenSSH 通过临时 key/agent 连接；local default shell 输出 color+宽字符、进入/退出 fixture TUI、resize、search、selection、copy/paste；两个 tab 同时含 H/V split，close/reconnect 后 server child/channel count 为 0；legacy/OpenSSH import count/group/auth ref/override 正确。

- [ ] **Step 2: 写 boundary 场景**

固定覆盖：host 以 `-` 开头、port 0/65536、1x1 与 999x999 resize、wide cell 中点 selection、10k output backpressure、unknown key reject、changed key、wrong password、keyboard-interactive cancel、vault fail-after-mutation、DB commit failure、损坏 primary+有效 backup、OpenSSH wildcard/include cycle、重复 Shutdown/Reconnect。每条都断言分类错误、prior visible state 和无 secret/log/process leak。

- [ ] **Step 3: 写邻近回归场景**

固定覆盖：system ssh remote command/identity path 含空格分号但不本地展开；未编辑空 password 不清凭据、编辑后清空才删除；actor panic 不终止 GTK；旧 EOF 平台错误仍映射 clean exit；latest frame 胜出、旧 generation 不回绘；Windows portable schema/pixbuf path；release binary 不依赖已删除 libssh2/旧 WezTerm revision。

- [ ] **Step 4: 先运行各 smoke 证明缺失断言失败，再完成 harness**

Run: `pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All`

Expected before completion: FAIL，并准确指出第一个尚未实现的 evidence 字段；不得以 skip 计成功。

在完成 harness 前先实现 `GtkSmokeDriver`：root flag `--smoke-p0 <scenario-json> <report-json>` 仍走 Task 19 的唯一 bootstrap 和真实 Relm4 `MainWindow`。`p0-scenario.json` 是版本化 action 数组，动作精确定义为 `wait_window_realized`、`new_tab`、`open_connection_editor`、`set_connection_field`、`submit_connection`、`select_connection`、`connect`、`respond_host_key`、`respond_auth`、`send_terminal_text`、`paste_text_from_env`、`resize_terminal`、`wait_frame_contains`、`split_horizontal`、`split_vertical`、`switch_tab`、`search_terminal`、`select_range`、`copy_selection`、`reconnect`、`preview_import`、`commit_import`、`cancel_import`、`close_all`。`new_tab` 只能调用 Task 17 的生产“新建标签” component handler，并等待它发出 `UiCommand::NewLocalTab` 及后续 `WorkspaceChanged`；不得由 driver 直接构造 workspace 或调用 SessionPort。secret 动作只引用环境变量名，不把值放入 scenario。其余动作同样只能向对应 Relm4 component 输入发送，触发与用户控件相同的 handler；不得直接调用 storage/session ports。每步默认 10 秒、全场景 120 秒，timeout 立即失败。观测只来自 realized widget state、组件 view model、真实 `RenderFrame`/`AppEvent` 和 actor/vault/journal cleanup counters；report 为每步时间、状态与脱敏 evidence，并附 GTK window PNG snapshot 路径。

report 字段与动作/证据固定绑定：`gtk`=`wait_window_realized`+widget realized+PNG；`local_terminal`=`new_tab`+`send_terminal_text`+`paste_text_from_env`+`resize_terminal`+宽字符 `wait_frame_contains`+`select_range`/`copy_selection`，断言 frame 内容、cell/pixel size 和 clipboard 文本；`native_password/native_key/native_keyboard_interactive/system_agent/host_key`=对应 connection editor/connect/respond actions 与本地 server auth/channel counters；`tabs_splits`=两个 `new_tab`、H/V split、switch/reconnect 后 pane tree 和 actor cleanup；`imports`=preview/commit/cancel 后 catalog count 与 pending preview count；`vault`=凭据 ref、数据库字节扫描和 cleanup；`cleanup`=`close_all` 后 actor/child/vault-temp/journal 全为 0。缺少任一 action 或观测即该字段失败，不得推断通过。

实现脚本时所有临时 config/known_hosts/key/vault ref 使用 `$env:TEMP` + UUID，`try/finally` 必须删除凭据、终止 server、删除 temp；脚本把 runtime OS 名保存到 `$platform`、当前 mode 保存到 `$mode`，启动本地 SSH server 后生成含动态端口和测试 fixture 路径的 scenario JSON，运行 `cargo run --locked -- --smoke-p0 $scenario $report`，每阶段写 `artifacts/p0-smoke/$platform-$mode.json`，secret 只通过子进程环境中的一次性值引用传递，不写 scenario/report/snapshot metadata。

- [ ] **Step 5: 运行完整真实表面 smoke**

Run: `pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All`

Expected: exit 0；report 中 `gtk`, `local_terminal`, `native_password`, `native_key`, `native_keyboard_interactive`, `system_agent`, `host_key`, `vault`, `imports`, `tabs_splits`, `cleanup` 全为 `passed`，没有 `skipped`。

Run: `pwsh -NoProfile -File scripts/qa/assert-no-secrets.ps1 -ArtifactRoot artifacts/p0-smoke`

Expected: exit 0；SQLite、stdout/stderr、JSON report、tracing log 均不含测试 secret。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add crates/rshell-session/tests crates/rshell-storage/tests scripts/qa tests; if ($LASTEXITCODE -eq 0) { git commit -m "test: add p0 real-surface smoke" -m "Exercise GTK, terminal, SSH, vault, import, split, and cleanup paths with security regressions." }`

Expected: 授权后提交；否则跳过。

---

### Task 21: 把格式、测试、SSH smoke、GTK probe 和打包放入三平台 CI

**Dependencies:** Task 20。

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/release.yml`
- Create: `scripts/qa/assert-package.ps1`
- Create/Test: `scripts/qa/workflow-contract.ps1`

**Interfaces:**
- Consumes: workspace commands、`p0-smoke.ps1`、root artifact。
- Produces: Linux x86_64、macOS arm64、Windows x86_64 required jobs 和可验证 package。

- [ ] **Step 1: 先锁定 CI 必须执行的命令和 expected gate**

每个平台用 `shell: pwsh` 执行：

```powershell
cargo fmt -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo check --workspace --all-targets --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --workspace --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo clippy --workspace --all-targets --locked -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All
```

Expected: 任一命令/真实场景失败则 job 失败；不得 `continue-on-error`。

- [ ] **Step 2: 更新三平台依赖和 real service setup**

保留 GTK：Linux `libgtk-4-dev` + Xvfb，macOS gtk4，Windows gvsbuild；删除旧 libssh2/OpenSSL/vcpkg 安装与 cache，因为 native SSH 为 russh/ring、system SSH 为 OS executable。Linux vault job 启动隔离 D-Bus/Secret Service，macOS 使用临时 unlocked keychain，Windows 使用 Credential Manager；每个 job 运行 ignored `system_vault` probe 并在 finally 删除随机条目。三个 runner 都确认 `ssh`/`ssh-keygen` 可用并运行 system/native SSH smoke。

- [ ] **Step 3: 更新 release 构建和 package 内容**

release matrix 使用 `cargo build --release --workspace --target ${{ matrix.target }} --locked`。Linux/macOS archive 包含 `rshell`、LICENSE、README；Windows zip 包含 `rshell.exe`、GTK DLL、glib schemas、gdk-pixbuf loaders/fontconfig 所需文件，但不再复制 OpenSSL/libssh2。打包后运行：

Run in workflow: `pwsh -NoProfile -File scripts/qa/assert-package.ps1 -Target $env:RSHELL_TARGET -Package $env:RSHELL_PACKAGE`

Expected: executable architecture 正确、所有 runtime file 存在、无 `ssh2`/旧 engine artifact；解包到全新 temp dir 后 `--smoke-startup` 通过。

- [ ] **Step 4: 验证工作流静态结构**

Run: `pwsh -NoProfile -File scripts/qa/workflow-contract.ps1`

Expected: exit 0；脚本解析两个 YAML 文本并断言三个 matrix OS、fmt/check/test/clippy/All smoke、release targets/package probe 全部存在，且不存在 `continue-on-error`、libssh2 或旧 revision。脚本只使用 PowerShell/.NET，不依赖待安装的 checker。

- [ ] **Step 5: 以 CI run 作为三平台验收证据**

Run after branch is available to GitHub Actions: `$runs = gh run list --workflow ci.yml --limit 1 --json databaseId,status,conclusion | ConvertFrom-Json; if ($runs.Count -ne 1) { throw 'CI run not found' }; gh run view $runs[0].databaseId --json jobs`

Expected: Linux、macOS、Windows jobs 均 `success`，且每个 job 的 fmt/check/test/clippy/SSH/vault/GTK steps 均 success。若当前环境没有 GitHub run 权限，此项是主代理执行阶段的外部 evidence requirement，不能改成 locally assumed pass。

- [ ] **Step 6: 可选 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add .github/workflows scripts/qa/assert-package.ps1; if ($LASTEXITCODE -eq 0) { git commit -m "ci: enforce p0 gates on three platforms" -m "Run workspace quality, real SSH/vault/GTK smoke, and package validation on Linux, macOS, and Windows." }`

Expected: 授权后提交；否则跳过。

---

### Task 22: 清理剩余旧资产并执行最终 P0 验收矩阵

**Dependencies:** Task 21。

**Files:**
- Modify: `README.md`
- Delete: `examples/minimal.rs`
- Delete: `examples/css_test.rs`
- Verify: `Cargo.toml`
- Verify: `Cargo.lock`
- Verify: all `crates/**`
- Verify: `resources/style.css`
- Verify: `.github/workflows/*.yml`
- Verify: `scripts/qa/*.ps1`

**Interfaces:**
- Consumes: 完整 P0 artifact/CI evidence。
- Produces: 与新架构、SQLite/vault 路径、认证能力和三平台命令一致的 README；零旧资产最终树。

- [ ] **Step 1: 更新用户/开发文档并删除旧 examples**

README 只描述 P0：本地/system/native SSH 能力差异；password/key/agent/keyboard-interactive 支持矩阵；严格 host key；SQLite 与系统 vault；连接/分组/tag/search/copy/move；tab/H/V split；旧 JSON/OpenSSH import；唯一 Alacritty 0.26 terminal-engine 的已记录 GO contract；三平台 prerequisites、全 feature/locked 的 `cargo` quality commands、terminal-engine gate 和 `pwsh scripts/qa/p0-smoke.ps1 -Mode All`。说明 `direct_session_child_count == 0` 只代表直接 PID 证据，Windows Job Object 的真实即时后代测试才是树证明，且报告 artifact 路径只保存 leaf 名。删除只验证旧 monolith/CSS 的两个 examples，不保留旧 API 文档。

- [ ] **Step 2: 执行全量静态清理检查**

Run: `rg -n 'src/(app|config|connection|credentials|ssh|storage|terminal|theme)\.rs|ConnectionStore|TerminalSessionHandle|libssh2' Cargo.toml Cargo.lock README.md src crates resources .github scripts`

Expected: 该旧源码/依赖搜索为 0 matches；workflow contract 另行证明唯一终端运行时为 `alacritty-terminal@0.26.0`，不得存在活动 WezTerm terminal runtime 命令或包内容。历史 importer alias 与 package/QA 的 WezTerm 禁止标记仍是负向验证，不是运行时依赖。

- [ ] **Step 3: 执行最终自动质量门**

Run: `cargo fmt --all -- --check; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; cargo check --workspace --all-targets --all-features --locked; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; cargo test --workspace --all-features --locked; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`

Expected: 全部 exit 0；0 failed；clippy 0 warnings。

- [ ] **Step 4: 再跑完整真实表面与 package 门**

Run: `pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; pwsh -NoProfile -File scripts/qa/assert-no-secrets.ps1 -ArtifactRoot artifacts/p0-smoke; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }`

Expected: recorded Alacritty 0.26 gate 输出 `decision=GO`；所有 P0 report 项 passed、无 skipped、无 secret、无 orphan process/credential/journal row。报告中的 `png_path` 与 `requested_png_path` 仅为 artifact leaf 名；`direct_session_child_count == 0` 仅为直接 PID 证据，Windows Job Object 的 `immediate_descendant_is_contained_before_first_user_marker` 为真实树证明。

- [ ] **Step 5: 审阅最终 diff 边界**

Run: `git rev-parse HEAD; git status --short; git diff --stat; git diff --check`

Expected: 只包含计划列出的 workspace/资源/CI/README 变化；`git diff --check` 无 whitespace error；不得出现 `tmp/`、测试 secret、真实 key、DB 或 smoke artifacts。静态本地结果不构成 hosted claim；后续 hosted CI/release 与 review artifacts 必须绑定同一 `HEAD` SHA，并以 artifact-relative report names 和该 SHA 的证据进行审阅。

- [ ] **Step 6: 可选最终 commit（仅在用户单独授权后执行）**

Run only after explicit authorization: `git add README.md examples Cargo.toml Cargo.lock src crates resources .github scripts tests; if ($LASTEXITCODE -eq 0) { git commit -m "docs: finalize rshell p0 rebuild" -m "Align documentation and repository contents with the verified P0 architecture and acceptance surface." }`

Expected: 仅在新授权后提交；当前授权状态下必须跳过。

---

## 2. P0 成功标准追踪

| 设计成功标准 | 实现任务 | 自动/真实证据 |
|---|---|---|
| 1. 三平台构建并启动原生 GUI | 4、15–21 | Task 19 startup JSON；Task 21 三平台 GTK/package jobs |
| 2. 创建、编辑、复制、移动、搜索、删除 SSH 配置 | 1、5、14、15、20 | catalog/repository/view-model tests；P0 smoke catalog scenario |
| 3. 本地与 SSH 终端输入、输出、尺寸、滚动、选择、复制粘贴 | 3、9–13、16、20 | engine contract、PTY/native tests、TerminalView tests、真实 local/SSH smoke |
| 4. password/private key/agent/keyboard-interactive + strict host key | 11–13、18、20 | system capabilities、russh local server、known_hosts tests、interaction UI、SSH smoke |
| 5. tab、H/V split、close、reconnect、状态 | 2、9、14、17、20 | pane tree、actor lifecycle、workspace view-model、真实 split cleanup |
| 6. secret 只在系统凭据库 | 6、7、12–14、18、20 | crash matrix、DB/log byte scan、真实 vault probe、no-secret script |
| 7. 配置写入事务性且异常退出无半写 | 5–7、14、20 | BEGIN IMMEDIATE rollback、journal crash points、reconcile smoke |
| 8. 原子导入旧 JSON 与 OpenSSH config | 7、8、18、20 | pure preview、fault injection、full rollback、真实 import counts |
| 9. unit/integration/SSH smoke/fmt/clippy/CI | 3–22 | Task 21 每个平台 required job 与 Task 22 最终命令 |

## 3. 验收场景分类

| 类别 | 必须通过的代表场景 | 任务 |
|---|---|---|
| Happy path | 新建 native password profile→vault→首次 host confirm→命令/frame；key+passphrase；system agent；local TUI；两 tab 与 H/V split；两种 import | 7、8、10–13、15–20 |
| 边界 | 端口/host 注入、1/999 geometry、宽字符边界、backpressure、changed key、错误认证、vault/DB 中断、损坏备份、wildcard/include cycle、幂等 close | 1–13、20 |
| 邻近回归 | argv 不经本地 shell、未改 password 保留、actor panic 不带走 GTK、平台 EOF、stale frame 丢弃、Windows portable runtime、无旧 link dependency | 4、6、9–11、16、19–22 |

## 4. 实施期间的 review / commit 边界

- 每项任务是一个独立测试循环和潜在 review 边界；只允许在该任务列出的测试通过后进入下一依赖任务。
- W2、W4 中无共享文件冲突的任务可并行；修改根 `Cargo.toml`/`Cargo.lock` 的结果必须由一个集成人合并并重跑 `cargo check --workspace --all-targets --locked`。
- Task 3 的引擎结论一旦锁定，后续不得重新引入第二 adapter；任何变更需重新跑完整 contract/benchmark。
- Task 19 是唯一旧源码删除边界；其前旧文件只作只读参考且不在 Cargo graph，其后不得从 backup 复制旧模块。
- Task 21 的三平台 GitHub run 是外部验收证据；本机单平台绿灯不能替代。
- 所有 commit 命令当前均为禁用状态，只有用户在后续对话单独授权对应 Git 写操作后才执行。

## 5. 已知风险、假设与主代理阻塞项

- **终端引擎风险:** 已转化为 Task 3 的硬 gate；WezTerm 失败会在同一任务切换并只保留 `alacritty_terminal =0.26.0`，不需要主代理预先选边。
- **跨资源一致性:** SQLite 与 OS vault 无分布式事务；计划用持久化 journal、新 credential ref、可见 catalog 单事务切换和启动 reconcile 达到崩溃可恢复，而不是作虚假原子承诺。
- **OpenSSH ProxyJump:** P0 只解析并把该候选固定为 system OpenSSH alias，依赖原 config；不实现 P1 jump/proxy 配置。
- **平台外部条件:** 三平台 runner 需要 GTK、system OpenSSH 和可启动的系统凭据服务；Task 21 显式准备并以失败关闭，不允许静默跳过。
- **主代理必须决定的阻塞项:** 无。

## 6. 计划自审结果

- 设计 P0 九项成功标准均已映射到任务与可执行证据。
- 所有跨任务类型/方法名称以第 0.2 节为唯一来源；后续任务未引入未声明的 UI→基础设施依赖。
- 旧代码删除位置固定在 Task 19；旧 release/examples 清理固定在 Task 21/22。
- P1/P2 未形成实现任务；仅对 OpenSSH import 中已有 ProxyJump 语义作 P0 委托处理。
- 所有命令可由 agent 执行；GitHub Actions run 是唯一需要仓库远端可用的外部证据，未将其伪装为本地结果。
- 当前计划 revision 的正式 plan-critic receipt 状态：`waiting for receipt`；planner 未调度 critic。
