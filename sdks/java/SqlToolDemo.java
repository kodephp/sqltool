package com.sqltool.sdk.demo;

import com.sqltool.sdk.SqlTool;
import com.sqltool.sdk.SqlTool.CrossDbMigrator;
import com.sqltool.sdk.SqlTool.MigrationResult;
import com.sqltool.sdk.SqlTool.ShardNode;
import com.sqltool.sdk.SqlTool.SmartSharding;
import com.sqltool.sdk.SqlTool.TableSpec;
import com.sqltool.sdk.SqlTool.FieldSpec;

import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * SQLTool Java SDK 演示程序
 *
 * 演示内容：
 *  1. 跨数据库迁移（异构 + 跨版本 + 字段重命名）
 *  2. 智能分库分表（路由 + 跨分片查询 + 批量写入 + Rebalance）
 *  3. HTTP API 客户端
 *  4. CLI 包装器
 */
public class SqlToolDemo {

    public static void demoCrossDbMigration() {
        System.out.println("=".repeat(70));
        System.out.println("演示 1: 跨数据库迁移 (MySQL 5.7 → PostgreSQL 16)");
        System.out.println("=".repeat(70));

        CrossDbMigrator mig = new CrossDbMigrator();

        // 源表结构
        List<FieldSpec> fields = Arrays.asList(
            new FieldSpec("id", "INT"),
            new FieldSpec("user_id", "BIGINT"),
            new FieldSpec("amount", "DECIMAL(10,2)"),
            new FieldSpec("status", "VARCHAR(32)"),
            new FieldSpec("created_at", "DATETIME"),
            new FieldSpec("updated_at", "TIMESTAMP"),
            new FieldSpec("remark", "TEXT")
        );
        TableSpec table = new TableSpec("orders", fields);

        // 字段重命名映射：remark → comment
        Map<String, String> manualFieldMap = new HashMap<>();
        manualFieldMap.put("remark", "comment");

        MigrationResult result = mig.migrateTable(
            "mysql://root:pass@localhost:3306/mydb",
            "postgresql://postgres:pass@localhost:5432/mydb",
            table,
            "5.7.40", "16.2.0",
            manualFieldMap
        );

        System.out.println("\n迁移方向: " + result.direction);
        System.out.println("源库: " + result.sourceDb + " (" + result.sourceVersion + ")");
        System.out.println("目标库: " + result.targetDb + " (" + result.targetVersion + ")");
        System.out.println("成功率: " + String.format("%.1f", result.successRate() * 100) + "%");
        System.out.println("有损转换: " + result.lossyConversions + " 个");
        System.out.println("耗时: " + result.elapsedMs + "ms");
        System.out.println("\n生成的 DDL:");
        System.out.println(result.ddl);

        System.out.println("\n字段映射详情:");
        for (MigrationResult fieldMigration : result.fieldMigrations) {
            String flag = "⚠️";
            System.out.println("  " + flag + " " + fieldMigration.sourceField + " (" + fieldMigration.sourceType + ") → " + fieldMigration.targetField + " (" + fieldMigration.targetType + ")");
        }
    }

    public static void demoSmartSharding() {
        System.out.println("\n" + "=".repeat(70));
        System.out.println("演示 2: 智能分库分表 (4 分片哈希)");
        System.out.println("=".repeat(70));

        SmartSharding sharding = new SmartSharding("orders", "user_id", "hash");
        sharding.addShard("s0", "mysql://node1/orders_0", "orders_0");
        sharding.addShard("s1", "mysql://node1/orders_1", "orders_1");
        sharding.addShard("s2", "mysql://node2/orders_2", "orders_2");
        sharding.addShard("s3", "mysql://node2/orders_3", "orders_3");

        System.out.println("\n路由演示（相同 key 路由到固定分片）:");
        for (String uid : new String[]{"user_001", "user_042", "user_999", "user_001"}) {
            ShardNode node = sharding.route(uid);
            System.out.println("  " + uid + " → 分片 " + node.id + " (表 " + node.table + ")");
        }

        System.out.println("\n跨分片查询演示:");
        Map<String, Object> q = sharding.query("amount > 100");
        System.out.println("  涉及分片数: " + q.get("total_shards"));
        System.out.println("  总行数: " + q.get("total_rows"));

        System.out.println("\n跨分片批量写入演示:");
        List<String> keys = Arrays.asList("user_001", "user_042", "user_999");
        Map<String, Object> w = sharding.writeBatch(keys);
        System.out.println("  总数: " + w.get("total") + ", 成功: " + w.get("success"));

        System.out.println("\nRebalance 计划（扩容演示）:");
        Map<String, Object> plan = sharding.rebalancePlan(10_000_000L);
        System.out.println("  预计迁移行数: " + plan.get("estimated_total_rows"));
        System.out.println("  预计耗时: " + plan.get("estimated_seconds") + "s");
    }

    public static void demoClient() {
        System.out.println("\n" + "=".repeat(70));
        System.out.println("演示 3: HTTP API 客户端");
        System.out.println("=".repeat(70));

        SqlTool.Client client = new SqlTool.Client("http://localhost:8080", 30);
        try {
            Map<String, Object> health = client.health();
            System.out.println("✓ 服务健康: " + health);
        } catch (Exception e) {
            System.out.println("✗ 无法连接 SQLTool 服务: " + e.getMessage());
            System.out.println("  启动方式: sqltool server -p 8080");
        }
    }

    public static void demoCLI() {
        System.out.println("\n" + "=".repeat(70));
        System.out.println("演示 4: CLI 包装器");
        System.out.println("=".repeat(70));

        SqlTool.CLI cli = new SqlTool.CLI();
        try {
            String version = cli.run("--version");
            System.out.println("✓ sqltool CLI 可用: " + version.trim());
        } catch (Exception e) {
            System.out.println("✗ sqltool CLI 不在 PATH: " + e.getMessage());
            System.out.println("  安装: cargo install sqltool");
        }
    }

    public static void main(String[] args) {
        demoCrossDbMigration();
        demoSmartSharding();
        demoClient();
        demoCLI();

        System.out.println("\n" + "=".repeat(70));
        System.out.println("✓ 演示完成");
        System.out.println("=".repeat(70));
    }
}
