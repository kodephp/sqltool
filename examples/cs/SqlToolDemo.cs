// SQLTool C# 调用示例
//
// 编译运行:
//   dotnet new console -o SqlToolDemo
//   cp SqlToolDemo.cs SqlToolDemo/Program.cs
//   dotnet run                # HTTP API 模式
//   dotnet run -- --cli       # CLI 模式

using System;
using System.Diagnostics;
using System.Net.Http;
using System.Text;
using System.Threading.Tasks;
using System.Text.Json;

class SqlToolDemo
{
    class SqlToolClient
    {
        private readonly string _baseUrl;
        private readonly HttpClient _client = new HttpClient();

        public SqlToolClient(string baseUrl)
        {
            _baseUrl = baseUrl.TrimEnd('/');
        }

        public async Task<string> HealthCheckAsync()
        {
            return await _client.GetStringAsync($"{_baseUrl}/api/health");
        }

        public async Task<string> DetectInjectionAsync(string input)
        {
            var content = new StringContent(
                JsonSerializer.Serialize(new { input }),
                Encoding.UTF8, "application/json");
            var resp = await _client.PostAsync($"{_baseUrl}/api/security/detect-injection", content);
            return await resp.Content.ReadAsStringAsync();
        }

        public async Task<string> BuildSafeSqlAsync(string table, string field, string op, string value)
        {
            var content = new StringContent(
                JsonSerializer.Serialize(new { table, field, operator = op, value }),
                Encoding.UTF8, "application/json");
            var resp = await _client.PostAsync($"{_baseUrl}/api/security/build-safe-sql", content);
            return await resp.Content.ReadAsStringAsync();
        }
    }

    class SqlToolCLI
    {
        public string Run(params string[] args)
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
            process.WaitForExit();
            return output;
        }

        public string DetectInjection(string input) => Run("detect-injection", "--input", input);
        public string BuildSafeSql(string table, string field, string op, string value)
            => Run("build-safe-sql", "--table", table, "--field", field, "--operator", op, "--value", value);
    }

    static void PrintResult(string title, string result)
    {
        Console.WriteLine($"\n{new string('=', 50)}");
        Console.WriteLine(title);
        Console.WriteLine(new string('=', 50));
        Console.WriteLine(result);
    }

    static async Task Main(string[] args)
    {
        bool useCLI = args.Length > 0 && args[0] == "--cli";

        Console.WriteLine(@"
╔══════════════════════════════════════════════════╗
║         SQLTool C# 调用示例                        ║
╚══════════════════════════════════════════════════╝
        ");

        if (useCLI)
        {
            Console.WriteLine("模式: CLI (不需要启动 server)\n");
            var cli = new SqlToolCLI();
            PrintResult("1. SQL注入检测", cli.DetectInjection("' OR '1'='1"));
            PrintResult("2. 构建安全SQL", cli.BuildSafeSql("users", "name", "=", "test'; DROP TABLE"));
        }
        else
        {
            Console.WriteLine("模式: HTTP API (需要启动 sqltool server)\n");
            var client = new SqlToolClient("http://localhost:8080");

            try
            {
                PrintResult("0. 健康检查", await client.HealthCheckAsync());
                PrintResult("1. SQL注入检测 - 恶意输入", await client.DetectInjectionAsync("' OR '1'='1"));
                PrintResult("2. SQL注入检测 - 正常输入", await client.DetectInjectionAsync("normal_input"));
                PrintResult("3. 构建安全SQL", await client.BuildSafeSqlAsync("users", "name", "=", "test'; DROP TABLE"));
            }
            catch (Exception e)
            {
                Console.WriteLine($"\n错误: 无法连接到 http://localhost:8080");
                Console.WriteLine("请先启动 sqltool server:");
                Console.WriteLine("  sqltool server -p 8080 -s mysql://localhost/mydb");
                Environment.Exit(1);
            }
        }

        Console.WriteLine($"\n{new string('=', 50)}");
        Console.WriteLine("示例执行完成!");
        Console.WriteLine(new string('=', 50));
    }
}
