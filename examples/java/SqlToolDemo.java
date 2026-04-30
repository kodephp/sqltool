// SQLTool Java 调用示例
//
// 编译运行:
//   javac SqlToolDemo.java
//   java SqlToolDemo           # HTTP API 模式
//   java SqlToolDemo --cli     # CLI 模式

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

public class SqlToolDemo {

    static class SqlToolClient {
        private final String baseUrl;

        public SqlToolClient(String baseUrl) {
            this.baseUrl = baseUrl.replaceAll("/$", "");
        }

        public String healthCheck() throws Exception {
            return get("/api/health");
        }

        public String detectInjection(String input) throws Exception {
            return post("/api/security/detect-injection", "{\"input\":\"" + input + "\"}");
        }

        public String buildSafeSql(String table, String field, String operator, String value) throws Exception {
            return post("/api/security/build-safe-sql",
                "{\"table\":\"" + table + "\",\"field\":\"" + field + "\",\"operator\":\"" + operator + "\",\"value\":\"" + value + "\"}");
        }

        private String get(String path) throws Exception {
            URL url = new URL(baseUrl + path);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("GET");
            return readResponse(conn);
        }

        private String post(String path, String body) throws Exception {
            URL url = new URL(baseUrl + path);
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("POST");
            conn.setRequestProperty("Content-Type", "application/json");
            conn.setDoOutput(true);
            conn.getOutputStream().write(body.getBytes(StandardCharsets.UTF_8));
            return readResponse(conn);
        }

        private String readResponse(HttpURLConnection conn) throws Exception {
            int code = conn.getResponseCode();
            BufferedReader reader = new BufferedReader(
                new InputStreamReader(code == 200 ? conn.getInputStream() : conn.getErrorStream(), StandardCharsets.UTF_8));
            StringBuilder response = new StringBuilder();
            String line;
            while ((line = reader.readLine()) != null) {
                response.append(line).append("\n");
            }
            reader.close();
            return response.toString();
        }
    }

    static class SqlToolCLI {
        public String run(String... args) throws Exception {
            ProcessBuilder pb = new ProcessBuilder();
            pb.command("sqltool");
            for (String arg : args) pb.command().add(arg);
            pb.redirectErrorStream(true);
            Process process = pb.start();
            BufferedReader reader = new BufferedReader(
                new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
            StringBuilder output = new StringBuilder();
            String line;
            while ((line = reader.readLine()) != null) {
                output.append(line).append("\n");
            }
            process.waitFor();
            return output.toString();
        }

        public String detectInjection(String input) {
            try { return run("detect-sql-injection", "--input", input); }
            catch (Exception e) { return "Error: " + e.getMessage(); }
        }

        public String buildSafeSql(String table, String field, String operator, String value) {
            try { return run("build-safe-sql", "--table", table, "--field", field, "--operator", operator, "--value", value); }
            catch (Exception e) { return "Error: " + e.getMessage(); }
        }
    }

    static void printResult(String title, String result) {
        System.out.println("\n" + "=".repeat(50));
        System.out.println(title);
        System.out.println("=".repeat(50));
        System.out.println(result);
    }

    public static void main(String[] args) throws Exception {
        boolean useCLI = args.length > 0 && args[0].equals("--cli");

        System.out.println("""
            ╔══════════════════════════════════════════════════╗
            ║         SQLTool Java 调用示例                     ║
            ╚══════════════════════════════════════════════════╝
            """);

        if (useCLI) {
            System.out.println("模式: CLI (不需要启动 server)\n");
            SqlToolCLI cli = new SqlToolCLI();
            printResult("1. SQL注入检测", cli.detectInjection("' OR '1'='1"));
            printResult("2. 构建安全SQL", cli.buildSafeSql("users", "name", "=", "test'; DROP TABLE"));
        } else {
            System.out.println("模式: HTTP API (需要启动 sqltool server)\n");
            SqlToolClient client = new SqlToolClient("http://localhost:8080");

            try {
                printResult("0. 健康检查", client.healthCheck());
                printResult("1. SQL注入检测 - 恶意输入", client.detectInjection("' OR '1'='1"));
                printResult("2. SQL注入检测 - 正常输入", client.detectInjection("normal_input"));
                printResult("3. 构建安全SQL", client.buildSafeSql("users", "name", "=", "test'; DROP TABLE"));
            } catch (Exception e) {
                System.out.println("\n错误: 无法连接到 http://localhost:8080");
                System.out.println("请先启动 sqltool server:");
                System.out.println("  sqltool server -p 8080 -s mysql://localhost/mydb");
                System.exit(1);
            }
        }

        System.out.println("\n" + "=".repeat(50));
        System.out.println("示例执行完成!");
        System.out.println("=".repeat(50));
    }
}
