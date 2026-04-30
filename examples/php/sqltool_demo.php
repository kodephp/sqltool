<?php
/**
 * SQLTool PHP 调用示例
 *
 * 安装依赖:
 *   composer require guzzlehttp/guzzle
 *
 * 或使用 curl 扩展 (内置)
 *
 * 使用方法:
 *   php sqltool_demo.php           # HTTP API 模式
 *   php sqltool_demo.php --cli     # CLI 模式
 */

class SqlToolClient {
    private string $baseUrl;
    private array $headers;

    public function __construct(string $baseUrl = 'http://localhost:8080') {
        $this->baseUrl = rtrim($baseUrl, '/');
        $this->headers = ['Content-Type: application/json'];
    }

    private function request(string $method, string $path, ?array $data = null): array {
        $ch = curl_init($this->baseUrl . $path);
        curl_setopt($ch, CURLOPT_CUSTOMREQUEST, $method);
        curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
        curl_setopt($ch, CURLOPT_HTTPHEADER, $this->headers);
        if ($data !== null) {
            curl_setopt($ch, CURLOPT_POSTFIELDS, json_encode($data));
        }
        $response = curl_exec($ch);
        curl_close($ch);
        return json_decode($response, true) ?? [];
    }

    public function healthCheck(): array {
        return $this->request('GET', '/api/health');
    }

    public function detectInjection(string $input): array {
        return $this->request('POST', '/api/security/detect-injection', ['input' => $input]);
    }

    public function buildSafeSql(string $table, string $field, string $operator, string $value): array {
        return $this->request('POST', '/api/security/build-safe-sql', [
            'table' => $table, 'field' => $field, 'operator' => $operator, 'value' => $value
        ]);
    }
}

class SqlToolCLI {
    public function run(...$args): string {
        $command = 'sqltool ' . implode(' ', array_map('escapeshellarg', $args));
        $output = [];
        exec($command, $output, $returnCode);
        return implode("\n", $output);
    }

    public function detectInjection(string $input): string {
        return $this->run('detect-injection', '--input', $input);
    }

    public function buildSafeSql(string $table, string $field, string $operator, string $value): string {
        return $this->run('build-safe-sql', '--table', $table, '--field', $field,
                          '--operator', $operator, '--value', $value);
    }
}

function printResult(string $title, $result): void {
    echo "\n" . str_repeat("=", 50) . "\n";
    echo "$title\n";
    echo str_repeat("=", 50) . "\n";
    if (is_array($result)) {
        echo json_encode($result, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE) . "\n";
    } else {
        echo "$result\n";
    }
}

$useCLI = isset($argv[1]) && $argv[1] === '--cli';

echo "
╔══════════════════════════════════════════════════╗
║         SQLTool PHP 调用示例                      ║
╚══════════════════════════════════════════════════╝
    ";

if ($useCLI) {
    echo "模式: CLI (不需要启动 server)\n\n";
    $cli = new SqlToolCLI();
    printResult("1. SQL注入检测", $cli->detectInjection("' OR '1'='1"));
    printResult("2. 构建安全SQL", $cli->buildSafeSql('users', 'name', '=', "test'; DROP TABLE"));
} else {
    echo "模式: HTTP API (需要启动 sqltool server)\n\n";
    $client = new SqlToolClient();

    $result = $client->healthCheck();
    if (empty($result)) {
        echo "\n错误: 无法连接到 http://localhost:8080\n";
        echo "请先启动 sqltool server:\n";
        echo "  sqltool server -p 8080 -s mysql://localhost/mydb\n";
        exit(1);
    }
    printResult("0. 健康检查", $result);

    printResult("1. SQL注入检测 - 恶意输入", $client->detectInjection("' OR '1'='1"));
    printResult("2. SQL注入检测 - 正常输入", $client->detectInjection("normal_input"));
    printResult("3. 构建安全SQL", $client->buildSafeSql('users', 'name', '=', "test'; DROP TABLE"));
}

echo "\n" . str_repeat("=", 50) . "\n";
echo "示例执行完成!\n";
echo str_repeat("=", 50) . "\n";
