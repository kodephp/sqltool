# SQLTool 项目规则

## 项目类型
Rust 数据库迁移与运维工具 (CLI + HTTP API + Library)

## 技术栈
- **语言**: Rust 1.96+
- **框架**: Tokio异步运行时, Axum Web框架
- **数据库**: MySQL, PostgreSQL, SQLite, Redis, MongoDB, Oracle
- **许可证**: Apache-2.0
- **当前版本**: 0.6.1

## 多语言 SDK
8 种语言官方 SDK，统一支持跨数据库迁移 + 智能分库分表：

| 语言 | 路径 | 演示 |
|------|------|------|
| Python | `sdks/python/sqltool_sdk.py` | `python3 sdks/python/sqltool_sdk.py` |
| Node.js | `sdks/node/sqltool_sdk.js` | `node sdks/node/sqltool_sdk.js` |
| Go | `sdks/go/sqltool_sdk.go` | `cd sdks/go/demo && go run .` |
| PHP | `sdks/php/sqltool_sdk.php` | `php sdks/php/sqltool_sdk.php` |
| Ruby | `sdks/ruby/sqltool_sdk.rb` | `ruby sdks/ruby/sqltool_sdk.rb` |
| Java | `sdks/java/SqlTool.java` | `cd sdks/java && javac *.java && java -cp . com.sqltool.sdk.demo.SqlToolDemo` |
| C# | `sdks/csharp/SqlToolSdk.cs` | `cd sdks/csharp && dotnet run` |
| Rust | `examples/rust/src/main.rs` | `cd examples/rust && cargo run` |

详细使用见 `sdks/SDK_USAGE.md`。

## CLI 子命令
```
sqltool transfer            数据迁移
sqltool backup              数据库备份
sqltool compare-data        数据对比
sqltool create-shard        分库分表
sqltool detect-slow-query   慢查询检测
sqltool spanning-query      跨分片查询
sqltool insert-log          插入日志
sqltool query-logs          查询日志
sqltool detect-sql-injection   SQL 注入检测
sqltool build-safe-sql      安全 SQL 构建
sqltool server              HTTP API 服务
```

## 代码规范

### 命名
- 模块: snake_case
- 结构体: PascalCase
- 方法: 简洁明了 (backup/restore/compare/migrate/sync/shard/query/detect/analyze)

### 错误处理
```rust
fn migrate(&self) -> Result<MigrationReport> {
    anyhow::Ok(())
}
```

## 发布流程

### 1. crates.io 发布（完整流程）

```bash
# 1.1 更新版本号（同时更新 Cargo.toml 头部注释和 README）
vim Cargo.toml
#   - version = "0.6.2"
#   - license = "Apache-2.0"
#   - 顶部注释中更新版本号和功能描述

# 1.2 更新 README 中的版本引用
#   - 标题 v0.6.x
#   - 徽章 crates.io v0.6.x
#   - 测试徽章数字（如有变化）
#   - Cargo.toml 依赖示例 sqltool = "0.6.x"
#   - 测试统计表
#   - 许可证 Apache-2.0

# 1.3 同步更新 .trae/rules/project_rules.md 的「当前版本」
vim .trae/rules/project_rules.md

# 1.4 运行全量测试
cargo test
# 期望：全部 256 个测试用例通过（0 失败）

# 1.5 本地构建
cargo build --release

# 1.6 预览打包内容（检查 .gitignore 是否覆盖完整）
cargo package --list
# 确认：不包含 target/、*.db、SDK 编译产物等

# 1.7 校验打包（不发布，验证 manifest 合法）
cargo package
cargo package --allow-dirty

# 1.8 登录 crates.io（首次需要）
cargo login
# 提示输入 token，从 https://crates.io/settings/tokens 获取
# 凭据默认存放在 ~/.cargo/credentials.toml

# 1.9 推送到 GitHub
git add -A
git commit -m "chore: bump version to v0.6.x"
git tag v0.6.x
git push origin master
git push origin v0.6.x

# 1.10 发布到 crates.io
cargo publish
# 或使用 --allow-dirty 在未提交时强制发布
cargo publish --allow-dirty

# 1.11 验证发布
cargo search sqltool
# 或浏览器打开 https://crates.io/crates/sqltool
```

### 2. 常用发布命令速查

| 命令 | 用途 |
|------|------|
| `cargo login` | 登录 crates.io（首次） |
| `cargo login <token>` | 使用 token 登录 |
| `cargo package --list` | 预览包内容 |
| `cargo package` | 打包到 target/package/ |
| `cargo publish` | 推送到 crates.io |
| `cargo publish --dry-run` | 干跑（不上传） |
| `cargo publish --allow-dirty` | 允许未提交修改时发布 |
| `cargo yank --version 0.6.1` | 撤回已发布版本（不删但禁止新依赖） |
| `cargo owner --add github:user` | 添加包协作者 |

### 3. crates.io 准备事项

**首次发布前**:
1. 登录 https://crates.io/ 注册账号
2. 在 https://crates.io/settings/tokens 生成 API token
3. 在 https://crates.io/settings/profile 验证邮箱
4. 确认 `Cargo.toml` 中 `name` 唯一可用

**Cargo.toml 必填字段**（否则无法发布）:
- `name` - 包名（与 crates.io 上其他包不重复）
- `version` - 遵循 SemVer
- `edition` - Rust edition（推荐 2021）
- `license` - 许可证标识符（SPDX，如 `Apache-2.0`）
- `description` - 一句话描述
- `repository` - 仓库 URL
- `readme` - README 文件名

### 4. 验证时间戳

每次发布后更新 README 中的「验证时间」字段为发布当天的日期（YYYY-MM-DD）。

## 测试与质量门禁

```bash
# 单元测试
cargo test --lib

# 集成测试
cargo test --test '*'

# 全量测试
cargo test

# 基准测试
cargo bench

# 代码检查
cargo clippy --all-targets -- -D warnings

# 格式化
cargo fmt --check
```

## 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| `failed to authenticate` | token 无效 | 重新 `cargo login` |
| `crate name already taken` | 包名冲突 | 修改 `name` 字段 |
| `no targets to publish` | 仅 lib 模式 | 添加 `[[bin]]` 或 `examples` |
| `invalid version` | 违反 SemVer | 主版本号必须递增或保持兼容 |
| `missing license` | 缺 license 字段 | 设置 `license = "Apache-2.0"` |
| `missing description` | 缺 description | 添加一行功能描述 |
| `package size > 10MB` | 包含大文件 | 调整 `exclude` 或 `.gitignore` |
| `yanked` 状态 | 主动撤回 | 新版发布后用户可正常升级 |
