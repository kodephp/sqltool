# SQLTool - 多语言调用指南

**核心**: Rust SQLTool 是唯一的二进制工具，其他语言通过 **命令行调用(subprocess)** 使用。

---

## 安装 SQLTool

```bash
# 方式1: cargo install（推荐）
cargo install sqltool

# 方式2: 下载二进制
# macOS
curl -L https://github.com/yourusername/sqltool/releases/latest/download/sqltool-macos.tar.gz | tar xz
# Linux
curl -L https://github.com/yourusername/sqltool/releases/latest/download/sqltool-linux.tar.gz | tar xz

# 方式3: 源码编译
git clone https://github.com/yourusername/sqltool.git
cd sqltool
cargo build --release
```

---

## Python 调用

### 基本调用

```python
import subprocess
import json
import shlex

def sqltool(*args):
    """调用 sqltool CLI 并返回结果"""
    cmd = ["sqltool"] + list(args)
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(result.stderr)
    return result.stdout

# 数据库备份
result = sqltool(
    "backup",
    "-s", "mysql://root:pass@localhost:3306/mydb",
    "--output", "./backup.sql",
    "--backup-type", "full"
)
print(result)

# 数据迁移
result = sqltool(
    "transfer",
    "-s", "mysql://root:pass@localhost:3306/source",
    "-t", "postgresql://postgres:pass@localhost:5432/target",
    "-B", "5000"
)
print(result)

# 数据对比
result = sqltool(
    "compare-data",
    "-s", "mysql://root:pass@localhost:3306/db1",
    "-t", "mysql://root:pass@localhost:3306/db2",
    "--table", "users",
    "--primary-key", "id",
    "--output", "json"
)
data = json.loads(result)
print(f"匹配率: {data.get('match_rate', 0)}%")

# SQL注入检测
result = sqltool(
    "detect-injection",
    "--input", "' OR '1'='1"
)
print(result)
```

### 使用 shell=True（需转义）

```python
import subprocess

def sqltool_shell(cmd_str):
    """安全使用 shell 方式（仅用于信任的输入）"""
    result = subprocess.run(cmd_str, shell=True, capture_output=True, text=True)
    return result.stdout

# 仅用于确定无用户输入的场景
result = sqltool_shell("sqltool backup -s 'mysql://root:pass@localhost:3306/mydb' --output ./backup.sql")
```

---

## Node.js 调用

### 基本调用

```javascript
const { execSync } = require('child_process');

function sqltool(...args) {
    const cmd = ['sqltool', ...args].join(' ');
    try {
        return execSync(cmd, { encoding: 'utf8' });
    } catch (error) {
        console.error('Error:', error.message);
        throw error;
    }
}

// 数据库备份
const backup = sqltool(
    'backup',
    '-s', 'mysql://root:pass@localhost:3306/mydb',
    '--output', './backup.sql',
    '--backup-type', 'full'
);
console.log(backup);

// 数据迁移
const transfer = sqltool(
    'transfer',
    '-s', 'mysql://root:pass@localhost:3306/source',
    '-t', 'postgresql://postgres:pass@localhost:5432/target',
    '-B', '5000'
);
console.log(transfer);

// 数据对比
const compare = sqltool(
    'compare-data',
    '-s', 'mysql://root:pass@localhost:3306/db1',
    '-t', 'mysql://root:pass@localhost:3306/db2',
    '--table', 'users',
    '--primary-key', 'id'
);
console.log(compare);
```

### 异步调用

```javascript
const { spawn } = require('child_process');

function sqltoolAsync(...args) {
    return new Promise((resolve, reject) => {
        const child = spawn('sqltool', args);
        let stdout = '';
        let stderr = '';

        child.stdout.on('data', (data) => { stdout += data; });
        child.stderr.on('data', (data) => { stderr += data; });
        child.on('close', (code) => {
            if (code === 0) resolve(stdout);
            else reject(new Error(stderr));
        });
    });
}

// 使用
async function main() {
    try {
        const result = await sqltoolAsync('health');
        console.log(result);
    } catch (e) {
        console.error(e.message);
    }
}
main();
```

---

## Go 调用

### 基本调用

```go
package main

import (
    "fmt"
    "os/exec"
    "strings"
)

func sqltool(args ...string) (string, error) {
    cmd := exec.Command("sqltool", args...)
    output, err := cmd.CombinedOutput()
    if err != nil {
        return "", fmt.Errorf("sqltool error: %v, output: %s", err, string(output))
    }
    return string(output), nil
}

func main() {
    // 数据库备份
    result, err := sqltool(
        "backup",
        "-s", "mysql://root:pass@localhost:3306/mydb",
        "--output", "./backup.sql",
        "--backup-type", "full",
    )
    if err != nil {
        fmt.Println("Error:", err)
        return
    }
    fmt.Println(result)

    // 数据迁移
    result, err = sqltool(
        "transfer",
        "-s", "mysql://root:pass@localhost:3306/source",
        "-t", "postgresql://postgres:pass@localhost:5432/target",
        "-B", "5000",
    )
    fmt.Println(result)

    // 数据对比
    result, err = sqltool(
        "compare-data",
        "-s", "mysql://root:pass@localhost:3306/db1",
        "-t", "mysql://root:pass@localhost:3306/db2",
        "--table", "users",
        "--primary-key", "id",
    )
    fmt.Println(result)
}
```

---

## PHP 调用

### 基本调用

```php
<?php

function sqltool(...$args) {
    $args = array_merge(['sqltool'], $args);
    $command = implode(' ', array_map('escapeshellarg', $args));
    $output = [];
    $returnCode = 0;
    exec($command, $output, $returnCode);

    if ($returnCode !== 0) {
        throw new RuntimeException("sqltool error: " . implode("\n", $output));
    }

    return implode("\n", $output);
}

// 数据库备份
try {
    $result = sqltool(
        'backup',
        '-s', 'mysql://root:pass@localhost:3306/mydb',
        '--output', './backup.sql',
        '--backup-type', 'full'
    );
    echo $result;
} catch (Exception $e) {
    echo "Error: " . $e->getMessage();
}

// 数据迁移
$result = sqltool(
    'transfer',
    '-s', 'mysql://root:pass@localhost:3306/source',
    '-t', 'postgresql://postgres:pass@localhost:5432/target',
    '-B', '5000'
);
echo $result;

// 数据对比
$result = sqltool(
    'compare-data',
    '-s', 'mysql://root:pass@localhost:3306/db1',
    '-t', 'mysql://root:pass@localhost:3306/db2',
    '--table', 'users',
    '--primary-key', 'id'
);
echo $result;
```

### 使用 shell_exec

```php
<?php
// 注意：仅用于确定无用户输入的场景
$result = shell_exec('sqltool backup -s "mysql://root:pass@localhost:3306/mydb" --output ./backup.sql');
echo $result;
```

---

## Java 调用

```java
import java.io.BufferedReader;
import java.io.InputStreamReader;

public class SqlTool {

    public static String sqltool(String... args) throws Exception {
        ProcessBuilder pb = new ProcessBuilder();
        pb.command("sqltool");
        for (String arg : args) {
            pb.command().add(arg);
        }
        pb.redirectErrorStream(true);

        Process process = pb.start();
        StringBuilder output = new StringBuilder();
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(process.getInputStream()))) {
            String line;
            while ((line = reader.readLine()) != null) {
                output.append(line).append("\n");
            }
        }

        int exitCode = process.waitFor();
        if (exitCode != 0) {
            throw new RuntimeException("sqltool failed with exit code: " + exitCode);
        }

        return output.toString();
    }

    public static void main(String[] args) throws Exception {
        // 数据库备份
        String result = sqltool(
            "backup",
            "-s", "mysql://root:pass@localhost:3306/mydb",
            "--output", "./backup.sql"
        );
        System.out.println(result);

        // 数据迁移
        result = sqltool(
            "transfer",
            "-s", "mysql://root:pass@localhost:3306/source",
            "-t", "postgresql://postgres:pass@localhost:5432/target"
        );
        System.out.println(result);
    }
}
```

---

## Ruby 调用

```ruby
require 'open3'

def sqltool(*args)
  cmd = ['sqltool'] + args
  stdout, stderr, status = Open3.capture3(*cmd)

  unless status.success?
    raise "sqltool error: #{stderr}"
  end

  stdout
end

# 数据库备份
result = sqltool(
  'backup',
  '-s', 'mysql://root:pass@localhost:3306/mydb',
  '--output', './backup.sql',
  '--backup-type', 'full'
)
puts result

# 数据迁移
result = sqltool(
  'transfer',
  '-s', 'mysql://root:pass@localhost:3306/source',
  '-t', 'postgresql://postgres:pass@localhost:5432/target'
)
puts result
```

---

## .NET/C# 调用

```csharp
using System.Diagnostics;

public class SqlTool
{
    public static string Sqltool(params string[] args)
    {
        var process = new Process
        {
            StartInfo = new ProcessStartInfo
            {
                FileName = "sqltool",
                Arguments = string.Join(" ", args),
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            }
        };

        process.Start();
        string output = process.StandardOutput.ReadToEnd();
        string error = process.StandardError.ReadToEnd();
        process.WaitForExit();

        if (process.ExitCode != 0)
        {
            throw new Exception($"sqltool error: {error}");
        }

        return output;
    }

    public static void Main()
    {
        // 数据库备份
        var result = Sqltool(
            "backup",
            "-s", "mysql://root:pass@localhost:3306/mydb",
            "--output", "./backup.sql"
        );
        Console.WriteLine(result);
    }
}
```

---

## CLI 命令参考

```bash
# 帮助
sqltool --help

# 数据库备份
sqltool backup -s mysql://user:pass@host:port/db --output ./backup.sql --backup-type full

# 备份恢复
sqltool restore --backup ./backup.sql -t mysql://user:pass@host:port/db

# 数据迁移
sqltool transfer -s mysql://source -t postgresql://target -B 5000

# 数据对比
sqltool compare-data -s db1 -t db2 --table users --primary-key id

# 创建分片
sqltool create-shard -s mysql://db --table orders --strategy row_count --threshold 1000000

# 跨分片查询
sqltool spanning-query -s mysql://db --table orders --condition "created_at > '2024-01-01'"

# 慢查询检测
sqltool detect-slow -s mysql://db --threshold-ms 1000

# 插入日志
sqltool insert-log -s mysql://db --table app_logs --level ERROR --message "test error"

# 查询日志
sqltool query-logs -s mysql://db --table app_logs --levels ERROR,WARN --limit 50

# SQL注入检测
sqltool detect-injection --input "' OR '1'='1"

# 构建安全SQL
sqltool build-safe-sql --table users --field name --operator = --value "test"

# 启动HTTP服务
sqltool server -p 8080 -s mysql://db --cors
```

---

## 连接字符串格式

| 数据库 | 格式 | 示例 |
|--------|------|------|
| MySQL | `mysql://user:pass@host:port/db` | `mysql://root:password@localhost:3306/mydb` |
| PostgreSQL | `postgresql://user:pass@host:port/db` | `postgresql://postgres:pass@localhost:5432/mydb` |
| SQLite | `sqlite:///path` 或 `sqlite:///:memory:` | `sqlite:///./mydb.sqlite` |
| Oracle | `oracle://user:pass@host:port/db` | `oracle://system:pass@localhost:1521/orcl` |
| Redis | `redis://host:port` | `redis://localhost:6379` |

---

## 注意事项

1. **安全性**: 连接字符串中的密码包含特殊字符时，使用各语言的数组形式传参而非 shell 字符串
2. **路径**: 确保 `sqltool` 在 PATH 中，或使用完整路径 `/path/to/sqltool`
3. **错误处理**: 始终检查返回码和 stderr 输出
4. **超时**: 对于长时间运行的任务，考虑设置超时
