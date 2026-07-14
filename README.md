# FirstSet

Windows Server 安装后的首次配置工具。它把容易出错的 PowerShell
操作收进一个可重复运行、带执行日志的 Rust 客户端中，支持简体中文和英语界面。

> **The first setup tool for Windows Server.**

> 本工具不会绕过微软授权。Windows 激活信息、RDS CAL、批量许可协议号或 Key
> Pack ID 均须由使用者合法取得。工作组服务器可以配置 Per User 模式，但无法跟踪和
> 报告本地用户的 CAL；程序会给出警告但不会阻断。Per Device 仍是微软支持的工作组方式。

## 功能

- **基础访问**：可勾选 Windows KMS 激活、OpenSSH、RDP；自动联动 TCP 22 与
  TCP/UDP 3389 防火墙规则，并显示活动网卡的 IP、MAC、网关和 DNS。
- **RDS 角色**：自动检测并显示 RD Session Host、RD Licensing 和管理工具状态；缺失时
  可补装，安装结果需要时可重启。
- **RDS 授权**：检测域/工作组环境，激活 RD License Server，配置授权模式和许可证
  服务器，并安装合法 RDS CAL。工作组的 Per User 模式仅提示无法跟踪，不会阻断执行。
- **本地用户**：创建 `user01` 到 `user20` 等本地用户，加入内置 Remote Desktop Users
  组，并把初始账号输出到管理员桌面。
- 四项功能相互独立，没有固定执行顺序，可按服务器实际状态选择使用。
- 所有功能均支持 dry-run；配置值通过进程环境传给 PowerShell，不出现在命令行。
- 语言可在右上角切换，选择会保存到 `config.toml`。
- `config.toml` 和账号清单在 Windows 上仅授予 Administrators 与 SYSTEM 访问权限。

## 使用方法

1. 从 GitHub Actions 或 Release 下载 Windows x64 构建产物并解压。
2. 将 `config.example.toml` 复制为 `config.toml`，放在 EXE 同目录；也可以首次启动后
   直接在 GUI 中填写。不要提交 `config.toml`。
3. 双击 EXE。程序清单会触发 Windows UAC，请使用管理员身份确认。
4. 先保持 **预演模式 / Dry run** 勾选，分别检查需要使用的功能；确认配置后取消勾选再正式执行。
5. 安装 RDS 角色后，如果程序提示需要重启，可点击重启按钮。
6. 创建本地用户后，及时安全分发并删除桌面的明文初始账号文件。

也可指定其他配置文件：

```powershell
.\FirstSet.exe --config D:\secure\config.toml
```

文件位置：

- 配置：默认位于 EXE 同目录的 `config.toml`。
- 用户清单：当前管理员桌面的 `firstset-users-*.txt`。

## RDS CAL 说明

RDS 授权功能只支持两种微软公开的安装入口：

- `Agreement`：合法的 7 位批量许可协议/注册号、协议类型、CAL 数量和产品版本。
- `KeyPackId`：通过微软 Clearinghouse 在线或电话流程取得的 35 位 Key Pack ID。

没有合法 CAL 信息时，请将 `cal_install_method` 保持为 `None`；程序会拒绝执行 CAL
安装。Windows Server 的 RDS 角色可以安装，并不代表 20 个用户可以免 CAL 长期并发。

产品版本 ID：`5` 为 Windows Server 2016、`6` 为 2019、`7` 为 2022。请以采购的
CAL 版本和目标 Windows Server 版本为准。

参考微软官方文档：

- [License Remote Desktop session hosts](https://learn.microsoft.com/windows-server/remote/remote-desktop-services/rds-license-session-hosts)
- [RDS Client Access Licenses](https://learn.microsoft.com/windows-server/remote/remote-desktop-services/rds-client-access-license)
- [Win32_TSLicenseKeyPack](https://learn.microsoft.com/windows/win32/termserv/win32-tslicensekeypack)

## 从源码构建

需要 Rust 1.92 或更高版本。

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

正式 Windows EXE 会嵌入 `requireAdministrator` 应用清单。系统修改逻辑位于
`assets/scripts/`，兼容 Windows PowerShell 5.1。GUI 使用 Direct3D 12/WGPU，兼容
启用 RDS 角色后的远程显示驱动，并可使用 Windows WARP 软件渲染。

## 安全边界

- 防火墙源地址默认是 `Any`，生产环境建议改成 VPN/VPC 网段或固定管理出口 IP。
- 本工具不会修改云厂商安全组；仍需在云平台侧按相同来源放行 22/3389。
- 不会创建 RD Gateway、Connection Broker、高可用或负载均衡。单机 20+ 会话的实际
  容量取决于 CPU、内存、存储 IOPS 和应用的多会话兼容性。
- 不要在公开仓库提交真实产品密钥、协议号、CAL 标识或用户密码。

## License

MIT.
