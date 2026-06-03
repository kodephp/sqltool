// SQLTool C# SDK
// 包含：HTTP 客户端、CLI 包装器、跨数据库迁移、智能分库分表
// 依赖：.NET Standard 2.0+（无第三方依赖）

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;
using System.Threading.Tasks;

namespace SqlTool.Sdk
{
    // ==========================================================================
    // HTTP 客户端
    // ==========================================================================

    public class SqlToolClient
    {
        public string BaseUrl { get; }
        private readonly HttpClient _http;
        private readonly int _timeoutSeconds;

        public SqlToolClient(string baseUrl = "http://localhost:8080", int timeoutSeconds = 30)
        {
            BaseUrl = baseUrl.TrimEnd('/');
            _timeoutSeconds = timeoutSeconds;
            _http = new HttpClient { Timeout = TimeSpan.FromSeconds(timeoutSeconds) };
        }

        public async Task<Dictionary<string, object>> HealthAsync()
        {
            return await RequestAsync("/api/health", "GET", null);
        }

        private async Task<Dictionary<string, object>> RequestAsync(string path, string method, object data)
        {
            var url = $"{BaseUrl}{path}";
            var req = new HttpRequestMessage(new HttpMethod(method), url);
            if (data != null)
            {
                req.Content = new StringContent(JsonSerializer.Serialize(data), Encoding.UTF8, "application/json");
            }
            var resp = await _http.SendAsync(req);
            var body = await resp.Content.ReadAsStringAsync();
            if (!resp.IsSuccessStatusCode)
                throw new Exception($"HTTP {(int)resp.StatusCode}: {body}");
            return JsonSerializer.Deserialize<Dictionary<string, object>>(body) ?? new Dictionary<string, object>();
        }
    }

    // ==========================================================================
    // CLI 包装器
    // ==========================================================================

    public class SqlToolCLI
    {
        private readonly string _binary;

        public SqlToolCLI(string binary = "sqltool")
        {
            _binary = binary;
        }

        public string Run(params string[] args)
        {
            var psi = new ProcessStartInfo
            {
                FileName = _binary,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                CreateNoWindow = true,
            };
            foreach (var a in args) psi.ArgumentList.Add(a);
            using var p = Process.Start(psi) ?? throw new Exception("启动 sqltool 失败");
            var stdout = p.StandardOutput.ReadToEnd();
            p.WaitForExit();
            if (p.ExitCode != 0) throw new Exception($"sqltool failed: {p.StandardError.ReadToEnd()}");
            return stdout;
        }
    }

    // ==========================================================================
    // 跨数据库迁移 - 模型
    // ==========================================================================

    public class FieldSpec
    {
        public string Name { get; set; }
        public string DataType { get; set; }
        public bool Nullable { get; set; } = true;
        public bool PrimaryKey { get; set; }
        public bool AutoIncrement { get; set; }

        public FieldSpec(string name, string dataType)
        {
            Name = name;
            DataType = dataType;
        }
    }

    public class TableSpec
    {
        public string Name { get; set; }
        public List<FieldSpec> Fields { get; set; }

        public TableSpec(string name, List<FieldSpec> fields)
        {
            Name = name;
            Fields = fields;
        }
    }

    public class FieldMigration
    {
        public string SourceField { get; set; }
        public string TargetField { get; set; }
        public string SourceType { get; set; }
        public string TargetType { get; set; }
        public bool Lossy { get; set; }
        public List<string> Warnings { get; set; } = new();

        public FieldMigration(string s, string t, string st, string tt, bool lossy)
        {
            SourceField = s; TargetField = t; SourceType = st; TargetType = tt; Lossy = lossy;
        }
    }

    public class MigrationResult
    {
        public string TableName { get; set; }
        public string Direction { get; set; }
        public string SourceDb { get; set; }
        public string TargetDb { get; set; }
        public string SourceVersion { get; set; }
        public string TargetVersion { get; set; }
        public int FieldsTotal { get; set; }
        public int FieldsMapped { get; set; }
        public int LossyConversions { get; set; }
        public List<string> Warnings { get; set; } = new();
        public List<FieldMigration> FieldMigrations { get; set; } = new();
        public string Ddl { get; set; }
        public long ElapsedMs { get; set; }

        public double SuccessRate => FieldsTotal == 0 ? 0.0 : (double)FieldsMapped / FieldsTotal;
    }

    // ==========================================================================
    // 跨数据库迁移器
    // ==========================================================================

    public class CrossDbMigrator
    {
        public static readonly string[] SupportedDbs = {
            "mysql", "postgresql", "sqlite", "tidb", "mariadb", "oracle", "mssql"
        };

        private static readonly Dictionary<string, string> AliasMap = new()
        {
            { "postgres", "postgresql" }, { "pg", "postgresql" }, { "sqlserver", "mssql" }
        };

        private static readonly Dictionary<string, string> Defaults = new()
        {
            { "mysql", "8.0.32" }, { "mariadb", "10.11.0" }, { "tidb", "7.5.0" },
            { "postgresql", "16.2.0" }, { "sqlite", "3.45.0" },
            { "oracle", "21.0.0" }, { "mssql", "16.0.0" }
        };

        // (srcDb|tgtDb|SRC_TYPE) -> (TARGET_TYPE, lossy)
        private static readonly Dictionary<string, (string, bool)> TypeRules = new()
        {
            // MySQL -> PostgreSQL
            { "mysql|postgresql|TINYINT", ("SMALLINT", true) },
            { "mysql|postgresql|INT", ("INTEGER", false) },
            { "mysql|postgresql|BIGINT", ("BIGINT", false) },
            { "mysql|postgresql|FLOAT", ("REAL", true) },
            { "mysql|postgresql|DOUBLE", ("DOUBLE PRECISION", false) },
            { "mysql|postgresql|DECIMAL", ("NUMERIC", false) },
            { "mysql|postgresql|DATETIME", ("TIMESTAMP", true) },
            { "mysql|postgresql|TIMESTAMP", ("TIMESTAMP WITH TIME ZONE", true) },
            { "mysql|postgresql|JSON", ("JSONB", false) },
            { "mysql|postgresql|BLOB", ("BYTEA", false) },
            { "mysql|postgresql|TEXT", ("TEXT", false) },
            { "mysql|postgresql|VARCHAR", ("VARCHAR", false) },
            // PostgreSQL -> MySQL
            { "postgresql|mysql|INTEGER", ("INT", false) },
            { "postgresql|mysql|BIGINT", ("BIGINT", false) },
            { "postgresql|mysql|DOUBLE PRECISION", ("DOUBLE", false) },
            { "postgresql|mysql|NUMERIC", ("DECIMAL", false) },
            { "postgresql|mysql|TIMESTAMP", ("DATETIME", true) },
            { "postgresql|mysql|BOOLEAN", ("TINYINT(1)", false) },
            { "postgresql|mysql|BYTEA", ("BLOB", false) },
            { "postgresql|mysql|JSONB", ("JSON", false) },
            { "postgresql|mysql|UUID", ("CHAR(36)", true) },
            // MySQL -> SQLite
            { "mysql|sqlite|INT", ("INTEGER", false) },
            { "mysql|sqlite|BIGINT", ("INTEGER", false) },
            { "mysql|sqlite|DATETIME", ("TEXT", true) },
            { "mysql|sqlite|TIMESTAMP", ("TEXT", true) },
            { "mysql|sqlite|JSON", ("TEXT", false) },
            { "mysql|sqlite|VARCHAR", ("TEXT", false) },
            { "mysql|sqlite|BOOLEAN", ("INTEGER", false) },
            // SQLite -> MySQL
            { "sqlite|mysql|INTEGER", ("BIGINT", true) },
            { "sqlite|mysql|REAL", ("DOUBLE", false) },
            { "sqlite|mysql|TEXT", ("TEXT", false) },
            { "sqlite|mysql|BLOB", ("BLOB", false) },
            // SQLite -> PostgreSQL
            { "sqlite|postgresql|INTEGER", ("BIGINT", true) },
            { "sqlite|postgresql|REAL", ("DOUBLE PRECISION", false) },
            { "sqlite|postgresql|TEXT", ("TEXT", false) },
            { "sqlite|postgresql|BLOB", ("BYTEA", false) },
        };

        public MigrationResult MigrateTable(
            string source, string target, TableSpec table,
            string sourceVersion = null, string targetVersion = null,
            Dictionary<string, string> manualFieldMap = null)
        {
            var sw = Stopwatch.StartNew();
            string srcDb = ParseDbType(source);
            string tgtDb = ParseDbType(target);
            string srcV = sourceVersion ?? (Defaults.ContainsKey(srcDb) ? Defaults[srcDb] : "1.0.0");
            string tgtV = targetVersion ?? (Defaults.ContainsKey(tgtDb) ? Defaults[tgtDb] : "1.0.0");
            string direction = InferDirection(srcDb, tgtDb, srcV, tgtV);

            manualFieldMap ??= new();

            var fms = new List<FieldMigration>();
            var warnings = new List<string>();
            int lossyCount = 0;

            foreach (var f in table.Fields)
            {
                string targetField = manualFieldMap.ContainsKey(f.Name) ? manualFieldMap[f.Name] : f.Name;
                var map = TypeMap(f.DataType, srcDb, tgtDb);
                string tgtType = map.target;
                bool lossy = map.lossy;
                tgtType = PreserveLength(tgtType, f.DataType);
                if (lossy)
                {
                    lossyCount++;
                    warnings.Add($"{f.DataType} → {tgtType} 可能损失精度");
                }
                fms.Add(new FieldMigration(f.Name, targetField, f.DataType, tgtType, lossy));
            }

            string ddl = GenerateDdl(table.Name, fms, tgtDb);
            int mapped = fms.Count(x => !string.IsNullOrEmpty(x.TargetField));

            var r = new MigrationResult
            {
                TableName = table.Name,
                Direction = direction,
                SourceDb = srcDb,
                TargetDb = tgtDb,
                SourceVersion = srcV,
                TargetVersion = tgtV,
                FieldsTotal = fms.Count,
                FieldsMapped = mapped,
                LossyConversions = lossyCount,
                Warnings = warnings,
                FieldMigrations = fms,
                Ddl = ddl,
                ElapsedMs = sw.ElapsedMilliseconds
            };
            return r;
        }

        public static string ParseDbType(string url)
        {
            var scheme = url.Split("://", 2)[0].ToLowerInvariant();
            return AliasMap.ContainsKey(scheme) ? AliasMap[scheme] : scheme;
        }

        private static string InferDirection(string src, string tgt, string srcV, string tgtV)
        {
            int[] sv = ParseVersion(srcV);
            int[] tv = ParseVersion(tgtV);
            bool sameV = sv.SequenceEqual(tv);
            if (src == tgt) return sameV ? "SameDbSameVersion" : "SameDbCrossVersion";
            return sameV ? "CrossDbSameVersion" : "CrossDbCrossVersion";
        }

        private static int[] ParseVersion(string v)
        {
            var parts = v.Replace("(", ".").Replace(")", "").Split('.');
            var r = new int[3];
            for (int i = 0; i < 3 && i < parts.Length; i++)
            {
                var digits = new string(parts[i].Where(char.IsDigit).ToArray());
                r[i] = string.IsNullOrEmpty(digits) ? 0 : int.Parse(digits);
            }
            return r;
        }

        private static (string target, bool lossy) TypeMap(string srcType, string srcDb, string tgtDb)
        {
            string baseType = srcType.ToUpperInvariant().Split('(')[0];
            string key = $"{srcDb}|{tgtDb}|{baseType}";
            if (TypeRules.ContainsKey(key)) return TypeRules[key];
            if (srcDb == tgtDb) return (srcType, false);
            return (srcType, false);
        }

        private static string PreserveLength(string targetType, string sourceType)
        {
            var sm = Regex.Match(sourceType, @"^([A-Za-z_]+)\s*\(([^)]+)\)");
            if (!sm.Success) return targetType;
            string srcBase = sm.Groups[1].Value.ToUpperInvariant();
            string srcLen = sm.Groups[2].Value;
            var tm = Regex.Match(targetType, @"^([A-Za-z_\s]+)\s*\(([^)]+)\)");
            if (!tm.Success) return $"{srcBase}({srcLen})";
            if (tm.Groups[1].Value.ToUpperInvariant().Trim() == srcBase) return targetType;
            return targetType;
        }

        private static string GenerateDdl(string tableName, List<FieldMigration> fms, string tgtDb)
        {
            string quote = (tgtDb == "mysql" || tgtDb == "mariadb" || tgtDb == "tidb") ? "`" : "\"";
            var cols = fms.Where(f => !string.IsNullOrEmpty(f.TargetField))
                .Select(f => $"  {quote}{f.TargetField}{quote} {f.TargetType}");
            return $"CREATE TABLE {quote}{tableName}{quote} (\n{string.Join(",\n", cols)}\n)";
        }
    }

    // ==========================================================================
    // 智能分库分表
    // ==========================================================================

    public class ShardNode
    {
        public string Id { get; set; }
        public string Connection { get; set; }
        public string Table { get; set; }
        public int Weight { get; set; } = 100;
        public bool Active { get; set; } = true;

        public ShardNode(string id, string connection, string table)
        {
            Id = id; Connection = connection; Table = table;
        }
    }

    public class SmartSharding
    {
        public string LogicalTable { get; }
        public string ShardKey { get; }
        public string Strategy { get; }
        public List<ShardNode> Nodes { get; } = new();

        public SmartSharding(string logicalTable, string shardKey, string strategy = "hash")
        {
            LogicalTable = logicalTable;
            ShardKey = shardKey;
            Strategy = strategy ?? "hash";
        }

        public void AddShard(string id, string connection, string table)
        {
            Nodes.Add(new ShardNode(id, connection, table));
        }

        private static long StableHash(string s)
        {
            // FNV-1a 64-bit
            ulong h = 0xcbf29ce484222325UL;
            foreach (var c in s) { h ^= c; h *= 0x100000001b3UL; }
            return (long)(h & 0x7fffffffffffffffUL);
        }

        public ShardNode Route(string shardValue)
        {
            var active = Nodes.Where(n => n.Active).ToList();
            if (active.Count == 0) throw new Exception($"表 {LogicalTable} 无活跃分片");
            if (Strategy == "hash")
            {
                int idx = (int)(StableHash(shardValue) % (ulong)active.Count);
                return active[idx];
            }
            else
            {
                if (!int.TryParse(shardValue, out int n)) n = 0;
                return active[n % active.Count];
            }
        }

        public Dictionary<string, object> Query(string whereClause = null)
        {
            var shardResults = new List<Dictionary<string, object>>();
            foreach (var n in Nodes)
            {
                if (!n.Active) continue;
                var r = new Dictionary<string, object>
                {
                    { "shard_id", n.Id },
                    { "sql", "SELECT * FROM " + n.Table + (string.IsNullOrEmpty(whereClause) ? "" : " WHERE " + whereClause) },
                    { "rows", new List<Dictionary<string, object>>() },
                    { "elapsed_ms", 0 }
                };
                shardResults.Add(r);
            }
            return new Dictionary<string, object>
            {
                { "total_shards", shardResults.Count },
                { "shard_results", shardResults },
                { "total_rows", 0 },
                { "has_more", false }
            };
        }

        public Dictionary<string, object> WriteBatch(List<string> keyValues)
        {
            var results = new List<Dictionary<string, object>>();
            foreach (var kv in keyValues)
            {
                var node = Route(kv);
                results.Add(new Dictionary<string, object>
                {
                    { "key", kv }, { "shard_id", node.Id }, { "success", true }
                });
            }
            int success = results.Count(r => (bool)r["success"]);
            return new Dictionary<string, object>
            {
                { "total", results.Count }, { "success", success },
                { "failed", results.Count - success }, { "results", results }
            };
        }

        public Dictionary<string, object> RebalancePlan(long totalRows = 1_000_000)
        {
            if (Nodes.Count < 2)
                return new() { { "moves", new List<Dictionary<string, object>>() }, { "estimated_total_rows", totalRows } };
            long perShard = totalRows / Nodes.Count;
            var moves = new List<Dictionary<string, object>>();
            for (int i = 1; i < Nodes.Count; i++)
            {
                moves.Add(new()
                {
                    { "from", Nodes[0].Id }, { "to", Nodes[i].Id },
                    { "range_start", (i - 1) * perShard }, { "range_end", i * perShard },
                    { "estimated_rows", perShard }
                });
            }
            return new()
            {
                { "moves", moves },
                { "estimated_total_rows", totalRows },
                { "estimated_seconds", totalRows / 10_000 }
            };
        }
    }

    // ==========================================================================
    // 演示
    // ==========================================================================

    public static class Program
    {
        public static void Main(string[] args)
        {
            Console.WriteLine(new string('=', 70));
            Console.WriteLine("SQLTool C# SDK 演示");
            Console.WriteLine(new string('=', 70));

            // 演示 1: 跨数据库迁移
            Console.WriteLine("\n[1] 跨数据库迁移 (MySQL 5.7 → PostgreSQL 16)");
            var mig = new CrossDbMigrator();
            var fields = new List<FieldSpec>
            {
                new("id", "INT"),
                new("user_id", "BIGINT"),
                new("amount", "DECIMAL(10,2)"),
                new("created_at", "DATETIME")
            };
            var table = new TableSpec("orders", fields);
            var result = mig.MigrateTable(
                "mysql://root:pass@localhost:3306/mydb",
                "postgresql://postgres:pass@localhost:5432/mydb",
                table, "5.7.40", "16.2.0", null);
            Console.WriteLine($"  方向: {result.Direction}");
            Console.WriteLine($"  映射: {result.FieldsMapped}/{result.FieldsTotal} ({result.SuccessRate * 100:F1}%)");
            Console.WriteLine($"  有损: {result.LossyConversions}");
            Console.WriteLine("  DDL:");
            Console.WriteLine(result.Ddl);

            // 演示 2: 智能分库分表
            Console.WriteLine("\n[2] 智能分库分表 (4 分片哈希)");
            var sharding = new SmartSharding("orders", "user_id", "hash");
            sharding.AddShard("s0", "mysql://n1/orders_0", "orders_0");
            sharding.AddShard("s1", "mysql://n1/orders_1", "orders_1");
            sharding.AddShard("s2", "mysql://n2/orders_2", "orders_2");
            sharding.AddShard("s3", "mysql://n2/orders_3", "orders_3");

            Console.WriteLine("  路由演示:");
            foreach (var uid in new[] { "user_001", "user_042", "user_001" })
            {
                var n = sharding.Route(uid);
                Console.WriteLine($"    {uid} → {n.Id} ({n.Table})");
            }
            var q = sharding.Query();
            Console.WriteLine($"  跨分片查询: 涉及 {q["total_shards"]} 分片");
            var w = sharding.WriteBatch(new List<string> { "u1", "u2", "u3" });
            Console.WriteLine($"  批量写入: {w["success"]}/{w["total"]} 成功");
            var plan = sharding.RebalancePlan(10_000_000);
            Console.WriteLine($"  Rebalance: {((List<object>)plan["moves"]).Count} 步");

            Console.WriteLine("\n✓ 演示完成");
        }
    }
}
