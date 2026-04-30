<?php
/**
 * SQLTool PHP 完整调用示例
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
 * 安装依赖: 无 (使用curl)
 *
 * 使用方法:
 *   php sqltool_demo.php                    # HTTP API 模式
 *   php sqltool_demo.php --cli             # CLI 模式
 */

// =============================================================================
// HTTP API 客户端
// =============================================================================

class SqlToolClient {
    private string $baseUrl;
    private array $headers;

    public function __construct(string $baseUrl = 'http://localhost:8080', ?string $apiKey = null) {
        $this->baseUrl = rtrim($baseUrl, '/');
        $this->headers = ['Content-Type: application/json'];
        if ($apiKey) {
            $this->headers[] = "Authorization: Bearer {$apiKey}";
        }
    }

    private function request(string $method, string $path, ?array $data = null): array {
        $ch = curl_init($this->baseUrl . $path);
        curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
        curl_setopt($ch, CURLOPT_HTTPHEADER, $this->headers);
        curl_setopt($ch, CURLOPT_TIMEOUT, 60);

        if ($method === 'POST') {
            curl_setopt($ch, CURLOPT_CUSTOMREQUEST, 'POST');
            if ($data) {
                curl_setopt($ch, CURLOPT_POSTFIELDS, json_encode($data));
            }
        }

        $response = curl_exec($ch);
        $httpCode = curl_getinfo($ch, CURLINFO_HTTP_CODE);
        curl_close($ch);

        if ($httpCode >= 400) {
            throw new Exception("HTTP Error: {$httpCode}");
        }

        return json_decode($response, true) ?? [];
    }

    // -------------------------------------------------------------------------
    // 健康检查
    // -------------------------------------------------------------------------

    public function healthCheck(): array {
        return $this->request('GET', '/api/health');
    }

    // -------------------------------------------------------------------------
    // 数据迁移
    // -------------------------------------------------------------------------

    /**
     * 数据迁移
     *
     * @param string $source 源数据库连接字符串
     * @param string $target 目标数据库连接字符串
     * @param string $sourceType 源数据库类型 (mysql/postgresql/sqlite/oracle)
     * @param string $targetType 目标数据库类型
     * @param string $tables 表名列表，逗号分隔，空表示所有表
     * @param int $batchSize 批量大小
     * @param bool $verifyData 是否验证数据
     * @param bool $skipErrors 是否跳过错误
     */
    public function transfer(
        string $source,
        string $target,
        string $sourceType = 'mysql',
        string $targetType = 'postgresql',
        string $tables = '',
        int $batchSize = 1000,
        bool $verifyData = true,
        bool $skipErrors = true
    ): array {
        return $this->request('POST', '/api/transfer', [
            'source' => $source,
            'target' => $target,
            'source_type' => $sourceType,
            'target_type' => $targetType,
            'tables' => $tables,
            'batch_size' => $batchSize,
            'verify_data' => $verifyData,
            'skip_errors' => $skipErrors
        ]);
    }

    // -------------------------------------------------------------------------
    // 数据库备份
    // -------------------------------------------------------------------------

    /**
     * 数据库备份
     *
     * @param string $source 数据库连接字符串
     * @param string $dbType 数据库类型
     * @param string $output 备份文件路径
     * @param string $backupType 备份类型 (full/incremental/differential)
     * @param bool $compress 是否压缩
     */
    public function backup(
        string $source,
        string $dbType = 'mysql',
        string $output = '/tmp/backup.sql',
        string $backupType = 'full',
        bool $compress = true
    ): array {
        return $this->request('POST', '/api/backup', [
            'source' => $source,
            'db_type' => $dbType,
            'output' => $output,
            'backup_type' => $backupType,
            'compress' => $compress,
            'include_procedures' => true,
            'include_functions' => true,
            'include_triggers' => true,
            'parallel_tables' => 4
        ]);
    }

    // -------------------------------------------------------------------------
    // 数据对比
    // -------------------------------------------------------------------------

    /**
     * 数据对比
     *
     * @param string $source 源数据库
     * @param string $target 目标数据库
     * @param string $table 表名
     * @param string $primaryKey 主键字段
     * @param string $sourceType 源类型
     * @param string $targetType 目标类型
     */
    public function compareData(
        string $source,
        string $target,
        string $table,
        string $primaryKey = 'id',
        string $sourceType = 'mysql',
        string $targetType = 'mysql'
    ): array {
        return $this->request('POST', '/api/compare', [
            'source' => $source,
            'target' => $target,
            'source_type' => $sourceType,
            'target_type' => $targetType,
            'table' => $table,
            'primary_key' => $primaryKey,
            'ignore_fields' => '',
            'compare_mode' => 'full'
        ]);
    }

    // -------------------------------------------------------------------------
    // 分库分表
    // -------------------------------------------------------------------------

    /**
     * 创建分片
     *
     * @param string $source 数据库连接
     * @param string $table 表名
     * @param string $strategy 分片策略 (row_count/time/size/hash)
     * @param string $threshold 阈值
     * @param string $prefix 分片前缀
     */
    public function createShard(
        string $source,
        string $table,
        string $strategy = 'row_count',
        string $threshold = '1000000',
        string $prefix = 'shard'
    ): array {
        return $this->request('POST', '/api/shard/create', [
            'source' => $source,
            'table' => $table,
            'strategy' => $strategy,
            'threshold' => $threshold,
            'prefix' => $prefix
        ]);
    }

    // -------------------------------------------------------------------------
    // 慢查询检测
    // -------------------------------------------------------------------------

    /**
     * 慢查询检测
     *
     * @param string $source 数据库连接
     * @param string $dbType 数据库类型
     * @param int $thresholdMs 阈值（毫秒）
     * @param int $limit 返回数量
     */
    public function detectSlowQuery(
        string $source,
        string $dbType = 'mysql',
        int $thresholdMs = 1000,
        int $limit = 10
    ): array {
        return $this->request('POST', '/api/detect-slow', [
            'source' => $source,
            'db_type' => $dbType,
            'threshold_ms' => $thresholdMs,
            'limit' => $limit
        ]);
    }

    // -------------------------------------------------------------------------
    // 跨分片查询
    // -------------------------------------------------------------------------

    /**
     * 跨分片查询
     *
     * @param string $source 数据库连接
     * @param string $table 表名
     * @param string $condition WHERE条件
     * @param string $orderBy 排序字段
     * @param string $orderDir 排序方向 (ASC/DESC)
     * @param int $limit 返回数量
     * @param int $offset 偏移量
     */
    public function spanningQuery(
        string $source,
        string $table,
        string $condition = '1=1',
        string $orderBy = '',
        string $orderDir = 'ASC',
        int $limit = 100,
        int $offset = 0
    ): array {
        return $this->request('POST', '/api/spanning-query', [
            'source' => $source,
            'table' => $table,
            'condition' => $condition,
            'order_by' => $orderBy,
            'order_dir' => $orderDir,
            'limit' => $limit,
            'offset' => $offset
        ]);
    }

    // -------------------------------------------------------------------------
    // 日志管理 - 插入
    // -------------------------------------------------------------------------

    /**
     * 插入日志
     *
     * @param string $source 数据库连接
     * @param string $level 日志级别 (DEBUG/INFO/WARN/ERROR)
     * @param string $message 日志消息
     * @param string $table 日志表名
     * @param string $sourceName 来源名称
     */
    public function insertLog(
        string $source,
        string $level = 'INFO',
        string $message = '',
        string $table = 'app_logs',
        string $sourceName = ''
    ): array {
        return $this->request('POST', '/api/log/insert', [
            'source' => $source,
            'table' => $table,
            'level' => $level,
            'message' => $message,
            'source_name' => $sourceName
        ]);
    }

    // -------------------------------------------------------------------------
    // 日志管理 - 查询
    // -------------------------------------------------------------------------

    /**
     * 查询日志
     *
     * @param string $source 数据库连接
     * @param string $levels 级别过滤（逗号分隔）
     * @param string $keyword 关键字过滤
     * @param string $table 日志表名
     * @param int $limit 返回数量
     */
    public function queryLogs(
        string $source,
        string $levels = '',
        string $keyword = '',
        string $table = 'app_logs',
        int $limit = 100
    ): array {
        $result = $this->request('POST', '/api/log/query', [
            'source' => $source,
            'table' => $table,
            'levels' => $levels,
            'keyword' => $keyword,
            'start_time' => 0,
            'end_time' => 0,
            'limit' => $limit
        ]);
        return $result['rows'] ?? [];
    }

    // -------------------------------------------------------------------------
    // SQL注入检测
    // -------------------------------------------------------------------------

    /**
     * SQL注入检测
     *
     * @param string $input 要检测的输入
     */
    public function detectInjection(string $input): array {
        return $this->request('POST', '/api/security/detect-injection', [
            'input' => $input
        ]);
    }

    // -------------------------------------------------------------------------
    // 安全SQL构建
    // -------------------------------------------------------------------------

    /**
     * 安全SQL构建
     *
     * @param string $table 表名
     * @param string $field 字段名
     * @param string $operator 操作符 (=, !=, <, >, LIKE, IN)
     * @param string $value 值
     */
    public function buildSafeSql(
        string $table,
        string $field,
        string $operator = '=',
        string $value = ''
    ): array {
        return $this->request('POST', '/api/security/build-safe-sql', [
            'table' => $table,
            'field' => $field,
            'operator' => $operator,
            'value' => $value
        ]);
    }
}

// =============================================================================
// CLI 客户端
// =============================================================================

class SqlToolCLI {
    private string $binaryPath;

    public function __construct(string $binaryPath = 'sqltool') {
        $this->binaryPath = $binaryPath;
    }

    private function run(...$args): string {
        $cmd = "{$this->binaryPath} " . implode(' ', array_map('escapeshellarg', $args));
        $output = [];
        $returnCode = 0;
        exec($cmd, $output, $returnCode);
        return implode("\n", $output);
    }

    // 数据迁移
    public function transfer(string $source, string $target, string $sourceType, string $targetType, string $tables = '', int $batchSize = 1000): string {
        $args = ['transfer', '-s', $source, '-t', $target, '-S', $sourceType, '-T', $targetType, '-B', $batchSize];
        if ($tables) {
            $args[] = '--tables';
            $args[] = $tables;
        }
        return $this->run(...$args);
    }

    // 数据库备份
    public function backup(string $source, string $output, string $dbType = 'mysql', string $backupType = 'full', bool $compress = true): string {
        $args = ['backup', '-s', $source, '-T', $dbType, '-o', $output, '-t', $backupType];
        if ($compress) {
            $args[] = '-c';
        }
        return $this->run(...$args);
    }

    // 数据对比
    public function compareData(string $source, string $target, string $table, string $primaryKey = 'id'): string {
        return $this->run('compare-data', '-s', $source, '-t', $target, '--table', $table, '--primary-key', $primaryKey);
    }

    // 创建分片
    public function createShard(string $source, string $table, string $strategy = 'row_count', string $threshold = '1000000', string $prefix = 'shard'): string {
        return $this->run('create-shard', '-s', $source, '--table', $table, '--strategy', $strategy, '--threshold', $threshold, '--prefix', $prefix);
    }

    // 慢查询检测
    public function detectSlowQuery(string $source, string $dbType = 'mysql', int $thresholdMs = 1000): string {
        return $this->run('detect-slow-query', '-s', $source, '-T', $dbType, '--threshold-ms', $thresholdMs);
    }

    // 跨分片查询
    public function spanningQuery(string $source, string $table, string $condition = '1=1', string $orderBy = '', int $limit = 100, int $offset = 0): string {
        $args = ['spanning-query', '-s', $source, '--table', $table, '--condition', $condition, '-L', $limit, '--offset', $offset];
        if ($orderBy) {
            $args[] = '--order-by';
            $args[] = $orderBy;
        }
        return $this->run(...$args);
    }

    // 插入日志
    public function insertLog(string $source, string $message, string $table = 'app_logs', string $level = 'INFO', string $sourceName = ''): string {
        $args = ['insert-log', '-s', $source, '--table', $table, '--level', $level, '--message', $message];
        if ($sourceName) {
            $args[] = '--source-name';
            $args[] = $sourceName;
        }
        return $this->run(...$args);
    }

    // 查询日志
    public function queryLogs(string $source, string $table = 'app_logs', string $levels = '', string $keyword = '', int $limit = 100): string {
        $args = ['query-logs', '-s', $source, '--table', $table, '-L', $limit];
        if ($levels) {
            $args[] = '--levels';
            $args[] = $levels;
        }
        if ($keyword) {
            $args[] = '--keyword';
            $args[] = $keyword;
        }
        return $this->run(...$args);
    }

    // SQL注入检测
    public function detectInjection(string $input): string {
        return $this->run('detect-sql-injection', '-i', $input);
    }

    // 安全SQL构建
    public function buildSafeSql(string $table, string $field, string $operator = '=', string $value = ''): string {
        return $this->run('build-safe-sql', '--table', $table, '--field', $field, '--operator', $operator, '--value', $value);
    }
}

// =============================================================================
// 主函数
// =============================================================================

function printResult(string $title, $result): void {
    echo "\n" . str_repeat('=', 60) . "\n";
    echo "{$title}\n";
    echo str_repeat('=', 60) . "\n";
    if (is_array($result)) {
        echo json_encode($result, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE) . "\n";
    } else {
        echo "{$result}\n";
    }
}

$useCLI = in_array('--cli', $argv);
$binaryPath = '/Users/Zhuanz/Desktop/website/composer/sqlmap/target/release/sqltool';

echo "
╔════════════════════════════════════════════════════════════╗
║         SQLTool PHP 完整调用示例 v0.4.1                ║
╚════════════════════════════════════════════════════════════╝
";

if ($useCLI) {
    echo "模式: CLI\n";
    echo "二进制: {$binaryPath}\n\n";

    $cli = new SqlToolCLI($binaryPath);

    echo "1. SQL注入检测...\n";
    printResult("检测结果", $cli->detectInjection("' OR '1'='1"));

    echo "2. 安全SQL构建...\n";
    printResult("构建结果", $cli->buildSafeSql('users', 'name', '=', "test'; DROP TABLE"));

    echo "3. 数据迁移...\n";
    printResult("迁移结果", $cli->transfer(
        'mysql://root:pass@localhost:3306/source',
        'postgresql://postgres:pass@localhost:5432/target',
        'mysql', 'postgresql', 'users,orders', 5000
    ));

    echo "4. 数据库备份...\n";
    printResult("备份结果", $cli->backup(
        'mysql://root:pass@localhost:3306/mydb',
        '/tmp/backup.sql', 'mysql', 'full', true
    ));

    echo "5. 数据对比...\n";
    printResult("对比结果", $cli->compareData(
        'mysql://root@localhost/db1',
        'mysql://root@localhost/db2',
        'users', 'id'
    ));
} else {
    echo "模式: HTTP API\n";
    echo "URL: http://localhost:8080\n\n";

    $client = new SqlToolClient('http://localhost:8080');

    try {
        echo "0. 健康检查...\n";
        printResult("健康状态", $client->healthCheck());

        echo "1. SQL注入检测...\n";
        $result = $client->detectInjection("' OR '1'='1");
        printResult("检测结果", $result);
        if (in_array($result['risk_level'] ?? '', ['High', 'Critical'])) {
            echo "⚠️ 警告: 检测到高风险SQL注入攻击!\n";
        }

        echo "2. 安全SQL构建...\n";
        printResult("构建结果", $client->buildSafeSql('users', 'email', 'LIKE', '%@example.com'));

        echo "3. 数据迁移 (需要真实数据库连接)...\n";
        printResult("迁移结果", $client->transfer(
            'mysql://root:password@localhost:3306/source_db',
            'postgresql://postgres:password@localhost:5432/target_db',
            'mysql', 'postgresql', 'users,orders,products', 5000
        ));

        echo "4. 数据库备份 (需要真实数据库连接)...\n";
        printResult("备份结果", $client->backup(
            'mysql://root:password@localhost:3306/mydb',
            'mysql', '/tmp/backup_20240101.sql', 'full', true
        ));

        echo "5. 数据对比 (需要真实数据库连接)...\n";
        printResult("对比结果", $client->compareData(
            'mysql://root:password@localhost:3306/db1',
            'mysql://root:password@localhost:3306/db2',
            'users', 'id'
        ));

        echo "6. 分库分表 (需要真实数据库连接)...\n";
        printResult("分片结果", $client->createShard(
            'mysql://root:password@localhost:3306/mydb',
            'orders', 'row_count', '1000000', 'orders_shard'
        ));

        echo "7. 慢查询检测 (需要真实数据库连接)...\n";
        printResult("检测结果", $client->detectSlowQuery(
            'mysql://root:password@localhost:3306/mydb', 'mysql', 1000, 10
        ));

        echo "8. 跨分片查询 (需要真实数据库连接)...\n";
        printResult("查询结果", $client->spanningQuery(
            'mysql://root:password@localhost:3306/mydb',
            'orders', "status='pending'", 'created_at', 'DESC', 100, 0
        ));

        echo "9. 插入日志 (需要真实数据库连接)...\n";
        printResult("插入结果", $client->insertLog(
            'mysql://root:password@localhost:3306/mydb',
            'INFO', '用户登录成功', 'app_logs', 'auth-service'
        ));

        echo "10. 查询日志 (需要真实数据库连接)...\n";
        printResult("查询结果", $client->queryLogs(
            'mysql://root:password@localhost:3306/mydb',
            'ERROR,WARN', 'login', 'app_logs', 50
        ));
    } catch (Exception $e) {
        echo "\n错误: {$e->getMessage()}\n";
        echo "\n请先启动 sqltool server:\n";
        echo "  sqltool server -p 8080 -s mysql://localhost/mydb\n";
        exit(1);
    }
}

echo "\n" . str_repeat('=', 60) . "\n";
echo "示例执行完成!\n";
echo str_repeat('=', 60) . "\n";
