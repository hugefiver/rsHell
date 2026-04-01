# rsHell

跨平台 SSH 终端管理器与 GUI 终端模拟器，使用 Rust + GTK4 构建。命名灵感来自 Xshell。

## 特性

- **本地终端** — 内置本地 Shell 会话，支持多标签页
- **SSH 连接管理** — 保存、分组、快速连接远程服务器
- **双 SSH 后端** — 支持系统 OpenSSH 和 WezTerm 原生 SSH
- **终端模拟** — 基于 wezterm-term，支持 6000 行回滚缓冲区
- **分屏布局** — 支持单屏、水平分屏、垂直分屏、三分屏、四宫格
- **跨平台** — 支持 Linux、macOS、Windows
- **Fluent UI 暗色主题** — 现代化深色界面

## 截图

<!-- TODO: 添加截图 -->

## 安装

### 从 Release 下载

前往 [Releases](https://github.com/hugefiver/rshell/releases) 页面下载对应平台的预编译二进制文件：

- `linux-x86_64`
- `macos-arm64`
- `windows-x86_64`

### 从源码构建

#### 前置依赖

- Rust 工具链 (edition 2024)
- GTK4 开发库
  - **Linux**: 通过包管理器安装 `libgtk-4-dev` 或同等包
  - **macOS**: `brew install gtk4`
  - **Windows**: 需要 [gvsbuild](https://github.com/wingtk/gvsbuild) 构建 GTK4，以及通过 vcpkg 安装 OpenSSL 和 libssh2

#### 构建

```bash
git clone https://github.com/hugefiver/rshell.git
cd rshell
cargo build --release
```

构建产物位于 `target/release/rshell`。

## 使用

```bash
# 直接启动
./rshell

# 指定默认 Shell（可选）
RSHELL_SHELL=/bin/zsh ./rshell
```

### 连接管理

1. 点击侧边栏的 **+** 按钮添加新连接
2. 填写主机名、端口、用户名等信息
3. 选择 SSH 后端（系统 OpenSSH 或 WezTerm SSH）
4. 连接将自动保存至本地配置文件

### 快捷操作

- 通过菜单栏新建本地标签页或 SSH 会话
- 侧边栏切换连接列表显示/隐藏
- 支持多种分屏布局切换

## 配置

连接配置保存在：

| 平台    | 路径                                          |
| ------- | --------------------------------------------- |
| Linux   | `~/.config/rshell/connections.json`           |
| macOS   | `~/Library/Application Support/rshell/connections.json` |
| Windows | `%LOCALAPPDATA%\rshell\connections.json`      |

## 开发

```bash
# 检查编译
cargo check

# 运行测试
cargo test --lib

# 代码检查
cargo clippy -- -D warnings

# 运行
cargo run
```

## 项目结构

```
src/
├── main.rs          # 入口：RelmApp → RshellApp
├── lib.rs           # 模块声明
├── app.rs           # 主界面组件（relm4 SimpleComponent）
├── terminal.rs      # PTY 会话管理、终端模拟
├── connection.rs    # 连接配置的存储与持久化
├── ssh.rs           # SSH 命令构建器
└── theme.rs         # 全局 CSS 主题加载
```

## 许可证

[MIT](LICENSE)
