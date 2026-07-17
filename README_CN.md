# FirstSet

> **Windows Server 安装后的第一站。**

[English](README.md) | [简体中文](README_CN.md)

FirstSet 是面向 Windows Server 的首次配置工具。它将容易出错的 PowerShell 操作封装
进可重复运行、带实时执行日志的 Rust GUI，并提供英语和简体中文界面。

> FirstSet 不会绕过微软授权。Windows 激活信息、RDS CAL、批量许可协议号和 Key Pack
> ID 均须由使用者合法取得。工作组服务器可以配置为每用户模式，但 RD Licensing 无法
> 跟踪或报告本地用户的 CAL 使用情况。FirstSet 会给出警告，但不会阻断配置。每设备仍是
> 微软支持的工作组授权模式。

## 功能

- **基础访问**：可选择配置 Windows KMS 激活、OpenSSH 和 RDP；OpenSSH 配置会自动修复
  `sshd` 服务身份，并确认服务进程实际监听 TCP 22；自动维护关联的 TCP 22
  与 TCP/UDP 3389 Windows 防火墙规则；显示活动网卡的 IP、MAC、网关和 DNS 信息。
- **RDS 角色**：检测远程桌面会话主机、RD 授权服务和授权管理工具的当前状态；补装缺失
  组件，并在 Windows 明确要求时提示重启。
- **RDS 授权**：检测域或工作组环境，激活 RD License Server，配置授权模式和许可证
  服务器，并安装合法购买的 RDS CAL。工作组服务器选择每用户模式时只提示警告，不阻断
  执行。
- **本地用户**：创建 `user01` 到 `user20` 等编号本地账号，将其加入内置 Remote
  Desktop Users 组，并把初始凭据写入管理员桌面。
- 四项功能相互独立，没有固定执行顺序，可按服务器当前状态选择使用。
- 所有会修改系统的功能均支持预演模式。配置值通过子进程环境传给 PowerShell，不会出现
  在命令行参数中。
- 界面语言选择会保存到 `config.toml`。
- 在 Windows 上，`config.toml` 和生成的账号文件仅授予 Administrators 与 SYSTEM
  访问权限。

## 使用方法

1. 从 GitHub Actions 或 Release 下载并解压 Windows x64 构建产物。
2. 将 `config.example.toml` 复制为 `config.toml`，放在 `FirstSet.exe` 同目录；也可以
   首次启动后直接在 GUI 中填写。切勿提交 `config.toml`。
3. 双击 `FirstSet.exe`，使用管理员账号确认 Windows UAC 提示。
4. 检查各项功能时保持 **预演模式 / Dry run** 开启；确认配置无误后再关闭并正式执行。
5. 安装 RDS 角色后，如果程序提示需要重启，请先使用重启按钮，再配置 RDS 授权。
6. 创建本地用户后，请安全分发凭据并删除桌面上的明文账号文件。

也可以显式指定其他配置文件：

```powershell
.\FirstSet.exe --config D:\secure\config.toml
```

默认文件位置：

- 配置文件：EXE 同目录的 `config.toml`。
- 账号清单：当前管理员桌面的 `firstset-users-*.txt`。

## RDS CAL 授权

FirstSet 支持 Windows RDS 授权提供程序公开的两种微软许可证安装入口：

- `Agreement`：合法的 7 位批量许可协议号或注册号、协议类型、CAL 数量和产品版本。
- `KeyPackId`：通过微软 Clearinghouse 在线或电话流程取得的 35 位 Key Pack ID。

没有合法 CAL 信息时，请将 `cal_install_method` 保持为 `None`，FirstSet 会拒绝安装
CAL。安装 Windows Server 的 RDS 角色并不意味着 20 个用户可以在没有所需 CAL 权益的
情况下长期并发使用。

产品版本 ID：`5` 为 Windows Server 2016、`6` 为 Windows Server 2019、`7` 为
Windows Server 2022。请确保该值同时匹配购买的 CAL 版本和目标 Windows Server 版本。

微软参考文档：

- [配置远程桌面会话主机授权](https://learn.microsoft.com/windows-server/remote/remote-desktop-services/rds-license-session-hosts)
- [RDS 客户端访问许可证](https://learn.microsoft.com/windows-server/remote/remote-desktop-services/rds-client-access-license)
- [Win32_TSLicenseKeyPack](https://learn.microsoft.com/windows/win32/termserv/win32-tslicensekeypack)

## 从源码构建

需要 Rust 1.92 或更高版本。

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

Windows Release 构建会嵌入 `requireAdministrator` 应用清单并静态链接 MSVC 运行库，
因此刚装完的服务器不需要预先安装 `VCRUNTIME140.dll`。CI 会在发布前检查最终 EXE 的
PE 依赖。系统配置脚本位于 `assets/scripts/`，兼容 Windows PowerShell 5.1。GUI 通过
WGPU 使用 Direct3D 12，可在启用 RDS 角色后的远程显示驱动环境中运行，并可使用
Windows WARP 进行软件渲染。

## 安全边界

- 防火墙来源地址默认为 `Any`。生产环境建议限制为 VPN/VPC 网段或固定管理出口 IP。
- FirstSet 不会修改云厂商安全组；仍需在云平台侧针对相同可信来源放行 22 和 3389。
- FirstSet 不会创建 RD Gateway、Connection Broker、高可用或负载均衡。单机 20 个以上
  会话的实际容量取决于 CPU、内存、存储 IOPS 和应用的多会话兼容性。
- 切勿向公开仓库提交真实产品密钥、协议号、CAL 标识、用户密码、私钥或包含敏感值的
  `config.toml`。

## 许可证

FirstSet 使用 MIT License，详见 [LICENSE-MIT](LICENSE-MIT)。
