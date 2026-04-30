using System;
using System.Diagnostics;
using System.Net.Http;
using System.Text;
using System.Threading.Tasks;

/**
 * SQLTool C# 完整调用示例
 *
 * 功能覆盖：
 *   - 数据迁移 (transfer)
 *   - 数据备份 (backup)
 *   - 数据对比 (compare-data)
 *   - 分库分表 (create-shard)
 *   - 慢查询检测 (detect-slow-query)
 *   - 跨分片查询 (spanning-query)
 *   - 日志管理 (insert-log/query-logs)
 *   - SQL注入检测 (detect-sql-injection)
 *   - 安全SQL构建 (build-safe-sql)
 *
 * 编译运行:
 *   dotnet new console -o SqlToolDemo
 *   cp SqlToolDemo.cs SqlToolDemo/Program.cs
 *   cd SqlToolDemo && dotnet run                    # HTTP API 模式
 *   cd SqlToolDemo && dotnet run -- cli            # CLI 模式
 */

class SqlToolDemo
{
    private const string BINARY_PATH = "/Users/Zhuanz/Desktop/website/composer/sqlmap/target/release/sqltool";

    // =========================================================================
    // HTTP API 客户端
    // =========================================================================

    class SqlToolClient
    {
        private readonly string _baseUrl;
        private readonly HttpClient _client;

        public SqlToolClient(string baseUrl = "http://localhost:8080")
        {
            _baseUrl = baseUrl.TrimEnd('/');
            _client = new HttpClient { Timeout = TimeSpan.FromSeconds(60) };
        }

        private async Task<string> PostAsync(string path, string json)
        {
            var content = new StringContent(json, Encoding.UTF8, "application/json");
            var response = await _client.PostAsync($"{_baseUrl}{path}", content);
            return await response.Content.ReadAsStringAsync();
        }

        private async Task<string> GetAsync(string path)
        {
            var response = await _client.GetAsync($"{_baseUrl}{path}");
            return await response.Content.ReadAsStringAsync();
        }

        public async Task<string> HealthCheckAsync() => await GetAsync("/api/health");

        public async Task<string> TransferAsync(string source, string target, string sourceType,
            string targetType, string tables, int batchSize, bool verifyData)
        {
            string json = $"{{\"source\":\"{source}\",\"target\":\"{target}\"," +
                $"\"source_type\":\"{sourceType}\",\"target_type\":\"{targetType}\"," +
                $"\"tables\":\"{tables}\",\"batch_size\":{batchSize}," +
                $"\"verify_data\":{verifyData.ToString().ToLower()},\"skip_errors\":true}}";
            return await PostAsync("/api/transfer", json);
        }

        public async Task<string> BackupAsync(string source, string dbType, string output,
            string backupType, bool compress)
        {
            string json = $"{{\"source\":\"{source}\",\"db_type\":\"{dbType}\"," +
                $"\"output\":\"{output}\",\"backup_type\":\"{backupType}\"," +
                $"\"compress\":{compress.ToString().ToLower()}}}";
            return await PostAsync("/api/backup", json);
        }

        public async Task<string> CompareDataAsync(string source, string target, string table,
            string primaryKey)
        {
            string json = $"{{\"source\":\"{source}\",\"target\":\"{target}\"," +
                $"\"table\":\"{table}\",\"primary_key\":\"{primaryKey}\"}}";
            return await PostAsync("/api/compare", json);
        }

        public async Task<string> CreateShardAsync(string source, string table,
            string strategy, string threshold, string prefix)
        {
            string json = $"{{\"source\":\"{source}\",\"table\":\"{table}\"," +
                $"\"strategy\":\"{strategy}\",\"threshold\":\"{threshold}\"," +
                $"\"prefix\":\"{prefix}\"}}";
            return await PostAsync("/api/shard/create", json);
        }

        public async Task<string> DetectSlowQueryAsync(string source, string dbType,
            int thresholdMs, int limit)
        {
            string json = $"{{\"source\":\"{source}\",\"db_type\":\"{dbType}\"," +
                $"\"threshold_ms\":{thresholdMs},\"limit\":{limit}}}";
            return await PostAsync("/api/detect-slow", json);
        }

        public async Task<string> SpanningQueryAsync(string source, string table,
            string condition, string orderBy, string orderDir, int limit, int offset)
        {
            string json = $"{{\"source\":\"{source}\",\"table\":\"{table}\"," +
                $"\"condition\":\"{condition}\",\"order_by\":\"{orderBy}\"," +
                $"\"order_dir\":\"{orderDir}\",\"limit\":{limit},\"offset\":{offset}}}";
            return await PostAsync("/api/spanning-query", json);
        }

        public async Task<string> InsertLogAsync(string source, string table,
            string level, string message, string sourceName)
        {
            string json = $"{{\"source\":\"{source}\",\"table\":\"{table}\"," +
                $"\"level\":\"{level}\",\"message\":\"{message}\"," +
                $"\"source_name\":\"{sourceName}\"}}";
            return await PostAsync("/api/log/insert", json);
        }

        public async Task<string> QueryLogsAsync(string source, string table,
            string levels, string keyword, int limit)
        {
            string json = $"{{\"source\":\"{source}\",\"table\":\"{table}\"," +
                $"\"levels\":\"{levels}\",\"keyword\":\"{keyword}\",\"limit\":{limit}}}";
            return await PostAsync("/api/log/query", json);
        }

        public async Task<string> DetectInjectionAsync(string input)
        {
            string json = $"{{\"input\":\"{input}\"}}";
            return await PostAsync("/api/security/detect-injection", json);
        }

        public async Task<string> BuildSafeSqlAsync(string table, string field,
            string op, string value)
        {
            string json = $"{{\"table\":\"{table}\",\"field\":\"{field}\"," +
                $"\"operator\":\"{op}\",\"value\":\"{value}\"}}";
            return await PostAsync("/api/security/build-safe-sql", json);
        }
    }

    // =========================================================================
    // CLI 客户端
    // =========================================================================

    class SqlToolCLI
    {
        private readonly string _binaryPath;

        public SqlToolCLI(string binaryPath = "sqltool")
        {
            _binaryPath = binaryPath;
        }

        public string Run(params string[] args)
        {
            try
            {
                var psi = new ProcessStartInfo
                {
                    FileName = _binaryPath,
                    Arguments = string.Join(" ", args),
                    RedirectStandardOutput = true,
                    UseShellExecute = false
                };
                var process = Process.Start(psi);
                return process!.StandardOutput.ReadToEnd();
            }
            catch (Exception ex)
            {
                return $"错误: {ex.Message}";
            }
        }

        public string Transfer(string source, string target, string sourceType,
            string targetType, string tables, int batchSize)
            => Run("transfer", "-s", source, "-t", target, "-S", sourceType,
                "-T", targetType, "-B", batchSize.ToString(), "--tables", tables);

        public string Backup(string source, string output, string dbType,
            string backupType, bool compress)
        {
            var args = new List<string> { "backup", "-s", source, "-o", output,
                "-T", dbType, "-t", backupType };
            if (compress) args.Add("-c");
            return Run(args.ToArray());
        }

        public string CompareData(string source, string target, string table, string primaryKey)
            => Run("compare-data", "-s", source, "-t", target, "--table", table,
                "--primary-key", primaryKey);

        public string CreateShard(string source, string table, string strategy,
            string threshold, string prefix)
            => Run("create-shard", "-s", source, "--table", table, "--strategy", strategy,
                "--threshold", threshold, "--prefix", prefix);

        public string DetectSlowQuery(string source, string dbType, int thresholdMs)
            => Run("detect-slow-query", "-s", source, "-T", dbType,
                "--threshold-ms", thresholdMs.ToString());

        public string SpanningQuery(string source, string table, string condition,
            string orderBy, int limit, int offset)
            => Run("spanning-query", "-s", source, "--table", table, "--condition", condition,
                "--order-by", orderBy, "-L", limit.ToString(), "--offset", offset.ToString());

        public string InsertLog(string source, string table, string level,
            string message, string sourceName)
            => Run("insert-log", "-s", source, "--table", table, "--level", level,
                "--message", message, "--source-name", sourceName);

        public string QueryLogs(string source, string table, string levels,
            string keyword, int limit)
            => Run("query-logs", "-s", source, "--table", table, "--levels", levels,
                "--keyword", keyword, "-L", limit.ToString());

        public string DetectInjection(string input)
            => Run("detect-sql-injection", "-i", input);

        public string BuildSafeSql(string table, string field, string op, string value)
            => Run("build-safe-sql", "--table", table, "--field", field,
                "--operator", op, "--value", value);
    }

    // =========================================================================
    // 主函数
    // =========================================================================

    static void PrintResult(string title, string result)
    {
        Console.WriteLine($"\n{new string('=', 60)}");
        Console.WriteLine(title);
        Console.WriteLine(new string('=', 60));
        Console.WriteLine(result);
    }

    static async Task Main(string[] args)
    {
        bool useCLI = args.Length > 0 && args[0] == "cli";

        Console.WriteLine(@"
╔════════════════════════════════════════════════════════════╗
║         SQLTool C# 完整调用示例 v0.4.1                ║
╚════════════════════════════════════════════════════════════╝
        ");

        if (useCLI)
        {
            Console.WriteLine("模式: CLI");
            Console.WriteLine($"二进制: {BINARY_PATH}\n");

            var cli = new SqlToolCLI(BINARY_PATH);

            Console.WriteLine("1. SQL注入检测...");
            PrintResult("检测结果", cli.DetectInjection("' OR '1'='1"));

            Console.WriteLine("2. 安全SQL构建...");
            PrintResult("构建结果", cli.BuildSafeSql("users", "name", "=", "test'; DROP TABLE"));

            Console.WriteLine("3. 数据迁移...");
            PrintResult("迁移结果", cli.Transfer(
                "mysql://root:pass@localhost:3306/source",
                "postgresql://postgres:pass@localhost:5432/target",
                "mysql", "postgresql", "users,orders", 5000));

            Console.WriteLine("4. 数据库备份...");
            PrintResult("备份结果", cli.Backup(
                "mysql://root:pass@localhost:3306/mydb",
                "/tmp/backup.sql", "mysql", "full", true));

            Console.WriteLine("5. 数据对比...");
            PrintResult("对比结果", cli.CompareData(
                "mysql://root@localhost/db1",
                "mysql://root@localhost/db2",
                "users", "id"));
        }
        else
        {
            Console.WriteLine("模式: HTTP API");
            Console.WriteLine("URL: http://localhost:8080\n");

            var client = new SqlToolClient("http://localhost:8080");

            try
            {
                Console.WriteLine("0. 健康检查...");
                PrintResult("健康状态", await client.HealthCheckAsync());

                Console.WriteLine("1. SQL注入检测...");
                var result = await client.DetectInjectionAsync("' OR '1'='1");
                PrintResult("检测结果", result);
                if (result.Contains("\"risk_level\":\"High\"") ||
                    result.Contains("\"risk_level\":\"Critical\""))
                    Console.WriteLine("⚠️ 警告: 检测到高风险SQL注入攻击!");

                Console.WriteLine("2. 安全SQL构建...");
                PrintResult("构建结果", await client.BuildSafeSqlAsync(
                    "users", "email", "LIKE", "%@example.com"));

                Console.WriteLine("3. 数据迁移 (需要真实数据库连接)...");
                PrintResult("迁移结果", await client.TransferAsync(
                    "mysql://root:password@localhost:3306/source_db",
                    "postgresql://postgres:password@localhost:5432/target_db",
                    "mysql", "postgresql", "users,orders,products", 5000, true));

                Console.WriteLine("4. 数据库备份 (需要真实数据库连接)...");
                PrintResult("备份结果", await client.BackupAsync(
                    "mysql://root:password@localhost:3306/mydb",
                    "mysql", "/tmp/backup_20240101.sql", "full", true));

                Console.WriteLine("5. 数据对比 (需要真实数据库连接)...");
                PrintResult("对比结果", await client.CompareDataAsync(
                    "mysql://root:password@localhost:3306/db1",
                    "mysql://root:password@localhost:3306/db2",
                    "users", "id"));

                Console.WriteLine("6. 分库分表 (需要真实数据库连接)...");
                PrintResult("分片结果", await client.CreateShardAsync(
                    "mysql://root:password@localhost:3306/mydb",
                    "orders", "row_count", "1000000", "orders_shard"));

                Console.WriteLine("7. 慢查询检测 (需要真实数据库连接)...");
                PrintResult("检测结果", await client.DetectSlowQueryAsync(
                    "mysql://root:password@localhost:3306/mydb", "mysql", 1000, 10));

                Console.WriteLine("8. 跨分片查询 (需要真实数据库连接)...");
                PrintResult("查询结果", await client.SpanningQueryAsync(
                    "mysql://root:password@localhost:3306/mydb",
                    "orders", "status='pending'", "created_at", "DESC", 100, 0));

                Console.WriteLine("9. 插入日志 (需要真实数据库连接)...");
                PrintResult("插入结果", await client.InsertLogAsync(
                    "mysql://root:password@localhost:3306/mydb",
                    "app_logs", "INFO", "用户登录成功", "auth-service"));

                Console.WriteLine("10. 查询日志 (需要真实数据库连接)...");
                PrintResult("查询结果", await client.QueryLogsAsync(
                    "mysql://root:password@localhost:3306/mydb",
                    "app_logs", "ERROR,WARN", "login", 50));
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"\n错误: {ex.Message}");
                Console.Error.WriteLine("\n请先启动 sqltool server:");
                Console.Error.WriteLine("  sqltool server -p 8080 -s mysql://localhost/mydb");
                Environment.Exit(1);
            }
        }

        Console.WriteLine($"\n{new string('=', 60)}");
        Console.WriteLine("示例执行完成!");
        Console.WriteLine(new string('=', 60));
    }
}
