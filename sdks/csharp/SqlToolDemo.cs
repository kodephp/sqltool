// SQLTool C# SDK 演示
// 编译：dotnet new console && 把此文件作为 Program.cs
// 依赖：.NET 6.0+

using System;
using System.Collections.Generic;
using SqlTool.Sdk;

class SqlToolDemo
{
    static void DemoCrossDbMigration()
    {
        Console.WriteLine(new string('=', 70));
        Console.WriteLine("演示 1: 跨数据库迁移 (MySQL 5.7 → PostgreSQL 16)");
        Console.WriteLine(new string('=', 70));

        var mig = new CrossDbMigrator();
        var fields = new List<FieldSpec>
        {
            new("id", "INT") { PrimaryKey = true, AutoIncrement = true },
            new("user_id", "BIGINT"),
            new("amount", "DECIMAL(10,2)"),
            new("status", "VARCHAR(32)"),
            new("created_at", "DATETIME"),
            new("updated_at", "TIMESTAMP"),
            new("remark", "TEXT") { Nullable = true }
        };
        var table = new TableSpec("orders", fields);

        // 字段重命名映射：remark → comment
        var manualFieldMap = new Dictionary<string, string> { { "remark", "comment" } };

        var result = mig.MigrateTable(
            "mysql://root:pass@localhost:3306/mydb",
            "postgresql://postgres:pass@localhost:5432/mydb",
            table, "5.7.40", "16.2.0", manualFieldMap);

        Console.WriteLine($"\n迁移方向: {result.Direction}");
        Console.WriteLine($"源库: {result.SourceDb} ({result.SourceVersion})");
        Console.WriteLine($"目标库: {result.TargetDb} ({result.TargetVersion})");
        Console.WriteLine($"成功率: {result.SuccessRate * 100:F1}%");
        Console.WriteLine($"有损转换: {result.LossyConversions} 个");
        Console.WriteLine($"耗时: {result.ElapsedMs}ms");
        Console.WriteLine("\n生成的 DDL:");
        Console.WriteLine(result.Ddl);

        Console.WriteLine("\n字段映射详情:");
        foreach (var fm in result.FieldMigrations)
        {
            string flag = fm.Lossy ? "⚠️" : "  ";
            Console.WriteLine($"  {flag} {fm.SourceField} ({fm.SourceType}) → {fm.TargetField} ({fm.TargetType})");
        }
    }

    static async void DemoSmartSharding()
    {
        Console.WriteLine("\n" + new string('=', 70));
        Console.WriteLine("演示 2: 智能分库分表 (4 分片哈希)");
        Console.WriteLine(new string('=', 70));

        var sharding = new SmartSharding("orders", "user_id", "hash");
        sharding.AddShard("s0", "mysql://node1/orders_0", "orders_0");
        sharding.AddShard("s1", "mysql://node1/orders_1", "orders_1");
        sharding.AddShard("s2", "mysql://node2/orders_2", "orders_2");
        sharding.AddShard("s3", "mysql://node2/orders_3", "orders_3");

        Console.WriteLine("\n路由演示（相同 key 路由到固定分片）:");
        foreach (var uid in new[] { "user_001", "user_042", "user_999", "user_001" })
        {
            var n = sharding.Route(uid);
            Console.WriteLine($"  {uid} → 分片 {n.Id} (表 {n.Table})");
        }

        Console.WriteLine("\n跨分片查询演示:");
        var q = sharding.Query("amount > 100");
        Console.WriteLine($"  涉及分片数: {q["total_shards"]}");
        Console.WriteLine($"  总行数: {q["total_rows"]}");

        Console.WriteLine("\n跨分片批量写入演示:");
        var keys = new List<string> { "user_001", "user_042", "user_999" };
        var w = sharding.WriteBatch(keys);
        Console.WriteLine($"  总数: {w["total"]}, 成功: {w["success"]}");

        Console.WriteLine("\nRebalance 计划（扩容演示）:");
        var plan = sharding.RebalancePlan(10_000_000);
        Console.WriteLine($"  预计迁移行数: {plan["estimated_total_rows"]}");
        Console.WriteLine($"  预计耗时: {plan["estimated_seconds"]}s");

        await Task.CompletedTask;
    }

    static async void DemoClient()
    {
        Console.WriteLine("\n" + new string('=', 70));
        Console.WriteLine("演示 3: HTTP API 客户端");
        Console.WriteLine(new string('=', 70));

        var client = new SqlToolClient("http://localhost:8080", 30);
        try
        {
            var health = await client.HealthAsync();
            Console.WriteLine($"✓ 服务健康: {health.Count} 个字段");
        }
        catch (Exception e)
        {
            Console.WriteLine($"✗ 无法连接 SQLTool 服务: {e.Message}");
            Console.WriteLine("  启动方式: sqltool server -p 8080");
        }
    }

    static void DemoCLI()
    {
        Console.WriteLine("\n" + new string('=', 70));
        Console.WriteLine("演示 4: CLI 包装器");
        Console.WriteLine(new string('=', 70));

        var cli = new SqlToolCLI();
        try
        {
            var version = cli.Run("--version");
            Console.WriteLine($"✓ sqltool CLI 可用: {version.Trim()}");
        }
        catch (Exception e)
        {
            Console.WriteLine($"✗ sqltool CLI 不在 PATH: {e.Message}");
            Console.WriteLine("  安装: cargo install sqltool");
        }
    }

    static void Main(string[] args)
    {
        DemoCrossDbMigration();
        DemoSmartSharding();
        DemoClient();
        DemoCLI();

        Console.WriteLine("\n" + new string('=', 70));
        Console.WriteLine("✓ 演示完成");
        Console.WriteLine(new string('=', 70));
    }
}
