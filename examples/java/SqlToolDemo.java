/**
 * SQLTool Java 完整调用示例
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
 *   javac SqlToolDemo.java
 *   java SqlToolDemo                    # HTTP API 模式
 *   java SqlToolDemo cli               # CLI 模式
 */

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Map;
import java.util.stream.Collectors;

public class SqlToolDemo {
    private static final String BINARY_PATH = "/Users/Zhuanz/Desktop/website/composer/sqlmap/target/release/sqltool";

    // =========================================================================
    // HTTP API 客户端
    // =========================================================================

    static class SqlToolClient {
        private final String baseUrl;
        private final HttpClient client;

        public SqlToolClient(String baseUrl) {
            this.baseUrl = baseUrl.replaceAll("/$", "");
            this.client = HttpClient.newHttpClient();
        }

        private String post(String path, String json) throws Exception {
            HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(baseUrl + path))
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(json))
                .timeout(Duration.ofSeconds(60))
                .build();
            return client.send(request, HttpResponse.BodyHandlers.ofString()).body();
        }

        private String get(String path) throws Exception {
            HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(baseUrl + path))
                .header("Content-Type", "application/json")
                .GET()
                .timeout(Duration.ofSeconds(30))
                .build();
            return client.send(request, HttpResponse.BodyHandlers.ofString()).body();
        }

        public String healthCheck() throws Exception {
            return get("/api/health");
        }

        public String transfer(String source, String target, String sourceType, String targetType,
                              String tables, int batchSize, boolean verifyData) throws Exception {
            String json = String.format("""
                {"source":"%s","target":"%s","source_type":"%s","target_type":"%s","tables":"%s","batch_size":%d,"verify_data":%b,"skip_errors":true}
                """, source, target, sourceType, targetType, tables, batchSize, verifyData);
            return post("/api/transfer", json);
        }

        public String backup(String source, String dbType, String output, String backupType, boolean compress) throws Exception {
            String json = String.format("""
                {"source":"%s","db_type":"%s","output":"%s","backup_type":"%s","compress":%b}
                """, source, dbType, output, backupType, compress);
            return post("/api/backup", json);
        }

        public String compareData(String source, String target, String table, String primaryKey) throws Exception {
            String json = String.format("""
                {"source":"%s","target":"%s","table":"%s","primary_key":"%s"}
                """, source, target, table, primaryKey);
            return post("/api/compare", json);
        }

        public String createShard(String source, String table, String strategy, String threshold, String prefix) throws Exception {
            String json = String.format("""
                {"source":"%s","table":"%s","strategy":"%s","threshold":"%s","prefix":"%s"}
                """, source, table, strategy, threshold, prefix);
            return post("/api/shard/create", json);
        }

        public String detectSlowQuery(String source, String dbType, int thresholdMs, int limit) throws Exception {
            String json = String.format("""
                {"source":"%s","db_type":"%s","threshold_ms":%d,"limit":%d}
                """, source, dbType, thresholdMs, limit);
            return post("/api/detect-slow", json);
        }

        public String spanningQuery(String source, String table, String condition, String orderBy, String orderDir, int limit, int offset) throws Exception {
            String json = String.format("""
                {"source":"%s","table":"%s","condition":"%s","order_by":"%s","order_dir":"%s","limit":%d,"offset":%d}
                """, source, table, condition, orderBy, orderDir, limit, offset);
            return post("/api/spanning-query", json);
        }

        public String insertLog(String source, String table, String level, String message, String sourceName) throws Exception {
            String json = String.format("""
                {"source":"%s","table":"%s","level":"%s","message":"%s","source_name":"%s"}
                """, source, table, level, message, sourceName);
            return post("/api/log/insert", json);
        }

        public String queryLogs(String source, String table, String levels, String keyword, int limit) throws Exception {
            String json = String.format("""
                {"source":"%s","table":"%s","levels":"%s","keyword":"%s","limit":%d}
                """, source, table, levels, keyword, limit);
            return post("/api/log/query", json);
        }

        public String detectInjection(String input) throws Exception {
            String json = String.format("{\"input\":\"%s\"}", input.replace("\"", "\\\""));
            return post("/api/security/detect-injection", json);
        }

        public String buildSafeSql(String table, String field, String operator, String value) throws Exception {
            String json = String.format("""
                {"table":"%s","field":"%s","operator":"%s","value":"%s"}
                """, table, field, operator, value.replace("\"", "\\\""));
            return post("/api/security/build-safe-sql", json);
        }
    }

    // =========================================================================
    // CLI 客户端
    // =========================================================================

    static class SqlToolCLI {
        private final String binaryPath;

        public SqlToolCLI(String binaryPath) {
            this.binaryPath = binaryPath;
        }

        public String run(String... args) {
            try {
                var cmd = java.util.Arrays.asList(binaryPath, args);
                var process = new ProcessBuilder(cmd).start();
                var output = new String(process.getInputStream().readAllBytes());
                process.waitFor();
                return output;
            } catch (Exception e) {
                return "错误: " + e.getMessage();
            }
        }

        public String transfer(String source, String target, String sourceType, String targetType, String tables, int batchSize) {
            return run("transfer", "-s", source, "-t", target, "-S", sourceType, "-T", targetType,
                       "-B", String.valueOf(batchSize), "--tables", tables);
        }

        public String backup(String source, String output, String dbType, String backupType, boolean compress) {
            var args = new java.util.ArrayList<String>();
            args.add("backup");
            args.add("-s");
            args.add(source);
            args.add("-o");
            args.add(output);
            args.add("-T");
            args.add(dbType);
            args.add("-t");
            args.add(backupType);
            if (compress) args.add("-c");
            return run(args.toArray(new String[0]));
        }

        public String compareData(String source, String target, String table, String primaryKey) {
            return run("compare-data", "-s", source, "-t", target, "--table", table, "--primary-key", primaryKey);
        }

        public String createShard(String source, String table, String strategy, String threshold, String prefix) {
            return run("create-shard", "-s", source, "--table", table, "--strategy", strategy,
                       "--threshold", threshold, "--prefix", prefix);
        }

        public String detectSlowQuery(String source, String dbType, int thresholdMs) {
            return run("detect-slow-query", "-s", source, "-T", dbType, "--threshold-ms", String.valueOf(thresholdMs));
        }

        public String spanningQuery(String source, String table, String condition, String orderBy, int limit, int offset) {
            return run("spanning-query", "-s", source, "--table", table, "--condition", condition,
                       "--order-by", orderBy, "-L", String.valueOf(limit), "--offset", String.valueOf(offset));
        }

        public String insertLog(String source, String table, String level, String message, String sourceName) {
            return run("insert-log", "-s", source, "--table", table, "--level", level, "--message", message,
                       "--source-name", sourceName);
        }

        public String queryLogs(String source, String table, String levels, String keyword, int limit) {
            return run("query-logs", "-s", source, "--table", table, "--levels", levels,
                       "--keyword", keyword, "-L", String.valueOf(limit));
        }

        public String detectInjection(String input) {
            return run("detect-sql-injection", "-i", input);
        }

        public String buildSafeSql(String table, String field, String operator, String value) {
            return run("build-safe-sql", "--table", table, "--field", field, "--operator", operator, "--value", value);
        }
    }

    // =========================================================================
    // 主函数
    // =========================================================================

    public static void printResult(String title, String result) {
        System.out.println("\n" + "=".repeat(60));
        System.out.println(title);
        System.out.println("=".repeat(60));
        System.out.println(result);
    }

    public static void main(String[] args) {
        boolean useCLI = args.length > 0 && args[0].equals("cli");

        System.out.println("""
            ╔════════════════════════════════════════════════════════════╗
            ║         SQLTool Java 完整调用示例 v0.4.1              ║
            ╚════════════════════════════════════════════════════════════╝
            """);

        if (useCLI) {
            System.out.println("模式: CLI");
            System.out.println("二进制: " + BINARY_PATH + "\n");

            SqlToolCLI cli = new SqlToolCLI(BINARY_PATH);

            System.out.println("1. SQL注入检测...");
            printResult("检测结果", cli.detectInjection("' OR '1'='1"));

            System.out.println("2. 安全SQL构建...");
            printResult("构建结果", cli.buildSafeSql("users", "name", "=", "test'; DROP TABLE"));

            System.out.println("3. 数据迁移...");
            printResult("迁移结果", cli.transfer(
                "mysql://root:pass@localhost:3306/source",
                "postgresql://postgres:pass@localhost:5432/target",
                "mysql", "postgresql", "users,orders", 5000
            ));

            System.out.println("4. 数据库备份...");
            printResult("备份结果", cli.backup(
                "mysql://root:pass@localhost:3306/mydb",
                "/tmp/backup.sql", "mysql", "full", true
            ));

            System.out.println("5. 数据对比...");
            printResult("对比结果", cli.compareData(
                "mysql://root@localhost/db1",
                "mysql://root@localhost/db2",
                "users", "id"
            ));
        } else {
            System.out.println("模式: HTTP API");
            System.out.println("URL: http://localhost:8080\n");

            SqlToolClient client = new SqlToolClient("http://localhost:8080");

            try {
                System.out.println("0. 健康检查...");
                printResult("健康状态", client.healthCheck());

                System.out.println("1. SQL注入检测...");
                String result = client.detectInjection("' OR '1'='1");
                printResult("检测结果", result);
                if (result.contains("\"risk_level\":\"High\"") || result.contains("\"risk_level\":\"Critical\"")) {
                    System.out.println("⚠️ 警告: 检测到高风险SQL注入攻击!");
                }

                System.out.println("2. 安全SQL构建...");
                printResult("构建结果", client.buildSafeSql("users", "email", "LIKE", "%@example.com"));

                System.out.println("3. 数据迁移 (需要真实数据库连接)...");
                printResult("迁移结果", client.transfer(
                    "mysql://root:password@localhost:3306/source_db",
                    "postgresql://postgres:password@localhost:5432/target_db",
                    "mysql", "postgresql", "users,orders,products", 5000, true
                ));

                System.out.println("4. 数据库备份 (需要真实数据库连接)...");
                printResult("备份结果", client.backup(
                    "mysql://root:password@localhost:3306/mydb",
                    "mysql", "/tmp/backup_20240101.sql", "full", true
                ));

                System.out.println("5. 数据对比 (需要真实数据库连接)...");
                printResult("对比结果", client.compareData(
                    "mysql://root:password@localhost:3306/db1",
                    "mysql://root:password@localhost:3306/db2",
                    "users", "id"
                ));

                System.out.println("6. 分库分表 (需要真实数据库连接)...");
                printResult("分片结果", client.createShard(
                    "mysql://root:password@localhost:3306/mydb",
                    "orders", "row_count", "1000000", "orders_shard"
                ));

                System.out.println("7. 慢查询检测 (需要真实数据库连接)...");
                printResult("检测结果", client.detectSlowQuery(
                    "mysql://root:password@localhost:3306/mydb", "mysql", 1000, 10
                ));

                System.out.println("8. 跨分片查询 (需要真实数据库连接)...");
                printResult("查询结果", client.spanningQuery(
                    "mysql://root:password@localhost:3306/mydb",
                    "orders", "status='pending'", "created_at", "DESC", 100, 0
                ));

                System.out.println("9. 插入日志 (需要真实数据库连接)...");
                printResult("插入结果", client.insertLog(
                    "mysql://root:password@localhost:3306/mydb",
                    "app_logs", "INFO", "用户登录成功", "auth-service"
                ));

                System.out.println("10. 查询日志 (需要真实数据库连接)...");
                printResult("查询结果", client.queryLogs(
                    "mysql://root:password@localhost:3306/mydb",
                    "app_logs", "ERROR,WARN", "login", 50
                ));
            } catch (Exception e) {
                System.err.println("\n错误: " + e.getMessage());
                System.err.println("\n请先启动 sqltool server:");
                System.err.println("  sqltool server -p 8080 -s mysql://localhost/mydb");
                System.exit(1);
            }
        }

        System.out.println("\n" + "=".repeat(60));
        System.out.println("示例执行完成!");
        System.out.println("=".repeat(60));
    }
}
