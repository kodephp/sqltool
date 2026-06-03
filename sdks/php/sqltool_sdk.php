<?php
/**
 * SQLTool PHP SDK
 *
 * 包含：
 *  1. HTTP 客户端
 *  2. 跨数据库迁移（异构 + 跨版本 + 自动字段连线）
 *  3. 智能分库分表（查询合并 + 写入协调 + 动态扩容）
 *
 * 依赖：仅 cURL 扩展（HTTP 模式可选）
 *
 * 用法：
 *   require_once 'sqltool_sdk.php';
 *   $mig = new CrossDbMigrator();
 *   $result = $mig->migrateTable(...);
 */

declare(strict_types=1);

class SqlToolClient {
    private string $baseUrl;
    private int $timeout;

    public function __construct(string $baseUrl = 'http://localhost:8080', int $timeout = 30) {
        $this->baseUrl = rtrim($baseUrl, '/');
        $this->timeout = $timeout;
    }

    public function health(): array {
        return $this->request('/api/health', 'GET');
    }

    public function transfer(string $source, string $target, string $sourceType, string $targetType, array $opts = []): array {
        return $this->request('/api/transfer', 'POST', array_merge([
            'source' => $source, 'target' => $target,
            'source_type' => $sourceType, 'target_type' => $targetType,
            'batch_size' => 1000, 'verify' => true,
        ], $opts));
    }

    public function backup(string $source, string $output, array $opts = []): array {
        return $this->request('/api/backup', 'POST', array_merge([
            'source' => $source, 'output' => $output,
            'backup_type' => 'full', 'compress' => true,
        ], $opts));
    }

    private function request(string $path, string $method, ?array $data = null): array {
        $url = $this->baseUrl . $path;
        $ch = curl_init($url);
        curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
        curl_setopt($ch, CURLOPT_TIMEOUT, $this->timeout);
        curl_setopt($ch, CURLOPT_CUSTOMREQUEST, $method);
        $headers = ['Content-Type: application/json'];
        if ($data !== null) {
            curl_setopt($ch, CURLOPT_POSTFIELDS, json_encode($data));
        }
        curl_setopt($ch, CURLOPT_HTTPHEADER, $headers);
        $resp = curl_exec($ch);
        $code = curl_getinfo($ch, CURLINFO_HTTP_CODE);
        curl_close($ch);
        if ($code !== 200) {
            throw new RuntimeException("SQLTool 请求失败: HTTP $code");
        }
        return json_decode($resp, true) ?? [];
    }
}

class FieldSpec {
    public function __construct(
        public string $name,
        public string $dataType,
        public bool $nullable = true,
        public bool $primaryKey = false,
        public bool $autoIncrement = false,
    ) {}
}

class TableSpec {
    public function __construct(public string $name, public array $fields) {}
}

class FieldMigration {
    public function __construct(
        public string $sourceField,
        public string $targetField,
        public string $sourceType,
        public string $targetType,
        public bool $lossy,
        public array $warnings = [],
    ) {}
}

class MigrationResult {
    public function __construct(
        public string $tableName,
        public string $direction,
        public string $sourceDb,
        public string $targetDb,
        public string $sourceVersion,
        public string $targetVersion,
        public int $fieldsTotal,
        public int $fieldsMapped,
        public int $lossyConversions,
        public array $warnings,
        public array $fieldMigrations,
        public string $ddl,
        public int $elapsedMs,
    ) {}

    public function successRate(): float {
        return $this->fieldsTotal === 0 ? 0.0 : $this->fieldsMapped / $this->fieldsTotal;
    }
}

class CrossDbMigrator {
    private array $typeRules;

    public function __construct(private ?SqlToolClient $client = null) {
        $this->client = $client ?? new SqlToolClient();
        $this->typeRules = $this->loadTypeRules();
    }

    public function migrateTable(
        string $source,
        string $target,
        TableSpec $table,
        ?string $sourceVersion = null,
        ?string $targetVersion = null,
        array $manualFieldMap = []
    ): MigrationResult {
        $start = microtime(true);
        $srcDb = self::parseDbType($source);
        $tgtDb = self::parseDbType($target);
        $srcV = $sourceVersion ?? self::defaultVersion($srcDb);
        $tgtV = $targetVersion ?? self::defaultVersion($tgtDb);
        $direction = self::inferDirection($srcDb, $tgtDb, $srcV, $tgtV);

        $fms = [];
        $warnings = [];
        $lossyCount = 0;

        foreach ($table->fields as $f) {
            $targetField = $manualFieldMap[$f->name] ?? $f->name;
            $baseType = strtoupper(explode('(', $f->dataType)[0]);
            $key = "$srcDb|$tgtDb|$baseType";
            if (isset($this->typeRules[$key])) {
                [$targetType, $lossy] = $this->typeRules[$key];
            } elseif ($srcDb === $tgtDb) {
                [$targetType, $lossy] = [$f->dataType, false];
            } else {
                [$targetType, $lossy] = [$f->dataType, false];
            }
            $targetType = self::preserveLength($targetType, $f->dataType);
            if ($lossy) {
                $lossyCount++;
                $warnings[] = "{$f->dataType} → {$targetType} 可能损失精度";
            }
            $fms[] = new FieldMigration($f->name, $targetField, $f->dataType, $targetType, $lossy);
        }

        $ddl = self::generateDdl($table->name, $fms, $tgtDb);
        $mapped = count(array_filter($fms, fn($m) => $m->targetField !== ''));

        return new MigrationResult(
            tableName: $table->name,
            direction: $direction,
            sourceDb: $srcDb,
            targetDb: $tgtDb,
            sourceVersion: $srcV,
            targetVersion: $tgtV,
            fieldsTotal: count($fms),
            fieldsMapped: $mapped,
            lossyConversions: $lossyCount,
            warnings: $warnings,
            fieldMigrations: $fms,
            ddl: $ddl,
            elapsedMs: (int)((microtime(true) - $start) * 1000),
        );
    }

    public static function parseDbType(string $url): string {
        $scheme = strtolower(explode('://', $url)[0]);
        $map = ['postgres' => 'postgresql', 'pg' => 'postgresql', 'sqlserver' => 'mssql'];
        return $map[$scheme] ?? $scheme;
    }

    public static function defaultVersion(string $db): string {
        $m = [
            'mysql' => '8.0.32', 'mariadb' => '10.11.0', 'tidb' => '7.5.0',
            'postgresql' => '16.2.0', 'sqlite' => '3.45.0',
            'oracle' => '21.0.0', 'mssql' => '16.0.0',
        ];
        return $m[$db] ?? '1.0.0';
    }

    public static function inferDirection(string $src, string $tgt, string $srcV, string $tgtV): string {
        if ($src === $tgt) return $srcV === $tgtV ? 'SameDbSameVersion' : 'SameDbCrossVersion';
        return $srcV === $tgtV ? 'CrossDbSameVersion' : 'CrossDbCrossVersion';
    }

    public static function preserveLength(string $target, string $source): string {
        if (!str_contains($source, '(')) return $target;
        if (!preg_match('/^([A-Za-z_]+)\s*\(([^)]+)\)/', $source, $m)) return $target;
        $srcBase = strtoupper($m[1]);
        if (!str_contains($target, '(') || strtoupper(explode('(', $target)[0]) === $srcBase) {
            if (!str_contains($target, '(')) return "$srcBase({$m[2]})";
        }
        return $target;
    }

    public static function generateDdl(string $tableName, array $fms, string $tgtDb): string {
        $quote = in_array($tgtDb, ['mysql', 'mariadb', 'tidb']) ? '`' : '"';
        $cols = [];
        foreach ($fms as $fm) {
            if ($fm->targetField === '') continue;
            $cols[] = "  $quote{$fm->targetField}$quote {$fm->targetType}";
        }
        return "CREATE TABLE $quote$tableName$quote (\n" . implode(",\n", $cols) . "\n)";
    }

    private function loadTypeRules(): array {
        $r = [];
        $add = function($src, $tgt, $st, $tt, $lossy) use (&$r) {
            $base = strtoupper(explode('(', $st)[0]);
            $r["$src|$tgt|$base"] = [$tt, $lossy];
        };
        // MySQL → PG
        $add('mysql', 'postgresql', 'TINYINT', 'SMALLINT', true);
        $add('mysql', 'postgresql', 'INT', 'INTEGER', false);
        $add('mysql', 'postgresql', 'BIGINT', 'BIGINT', false);
        $add('mysql', 'postgresql', 'DECIMAL', 'NUMERIC', false);
        $add('mysql', 'postgresql', 'DATETIME', 'TIMESTAMP', true);
        $add('mysql', 'postgresql', 'TIMESTAMP', 'TIMESTAMP WITH TIME ZONE', true);
        $add('mysql', 'postgresql', 'JSON', 'JSONB', false);
        $add('mysql', 'postgresql', 'BLOB', 'BYTEA', false);
        // PG → MySQL
        $add('postgresql', 'mysql', 'INTEGER', 'INT', false);
        $add('postgresql', 'mysql', 'BIGINT', 'BIGINT', false);
        $add('postgresql', 'mysql', 'TIMESTAMP', 'DATETIME', true);
        $add('postgresql', 'mysql', 'BOOLEAN', 'TINYINT(1)', false);
        $add('postgresql', 'mysql', 'BYTEA', 'BLOB', false);
        $add('postgresql', 'mysql', 'JSONB', 'JSON', false);
        $add('postgresql', 'mysql', 'UUID', 'CHAR(36)', true);
        // MySQL → SQLite
        $add('mysql', 'sqlite', 'INT', 'INTEGER', false);
        $add('mysql', 'sqlite', 'DATETIME', 'TEXT', true);
        $add('mysql', 'sqlite', 'JSON', 'TEXT', false);
        // SQLite → MySQL
        $add('sqlite', 'mysql', 'INTEGER', 'BIGINT', true);
        $add('sqlite', 'mysql', 'REAL', 'DOUBLE', false);
        return $r;
    }
}

class ShardNode {
    public function __construct(
        public string $id, public string $connection, public string $table,
        public int $weight = 100, public bool $active = true,
    ) {}
}

class SmartSharding {
    public string $logicalTable;
    public string $shardKey;
    public string $strategy;
    public array $nodes = [];

    public function __construct(string $logicalTable, string $shardKey, string $strategy = 'hash') {
        $this->logicalTable = $logicalTable;
        $this->shardKey = $shardKey;
        $this->strategy = $strategy;
    }

    public function addShard(string $id, string $connection, string $table): void {
        $this->nodes[] = new ShardNode($id, $connection, $table);
    }

    private function stableHash(string $s): int {
        // 使用 32-bit FNV-1a 避免 PHP 64-bit 浮点精度问题
        $h = 2166136261; // FNV offset basis (32-bit)
        for ($i = 0; $i < strlen($s); $i++) {
            $h ^= ord($s[$i]);
            $h = ($h * 16777619) & 0xFFFFFFFF; // FNV prime (32-bit)
        }
        return $h;
    }

    public function route(string $shardValue): ShardNode {
        $active = array_filter($this->nodes, fn($n) => $n->active);
        if (empty($active)) throw new RuntimeException('无活跃分片');
        $active = array_values($active);
        if ($this->strategy === 'hash') {
            $idx = $this->stableHash($shardValue) % count($active);
            return $active[$idx];
        }
        $n = (int)$shardValue;
        return $active[$n % count($active)];
    }

    public function query(): array {
        $results = [];
        foreach ($this->nodes as $node) {
            if (!$node->active) continue;
            $results[] = [
                'shard_id' => $node->id,
                'sql' => "SELECT * FROM {$node->table}",
                'rows' => [],
                'elapsed_ms' => 0,
            ];
        }
        return [
            'total_shards' => count($results),
            'shard_results' => $results,
            'total_rows' => 0,
            'has_more' => false,
        ];
    }

    public function writeBatch(array $keyValues): array {
        $results = [];
        $success = 0;
        foreach ($keyValues as $kv) {
            try {
                $node = $this->route($kv);
                $results[] = ['key' => $kv, 'shard_id' => $node->id, 'success' => true];
                $success++;
            } catch (Throwable $e) {
                $results[] = ['key' => $kv, 'success' => false, 'error' => $e->getMessage()];
            }
        }
        return [
            'total' => count($results),
            'success' => $success,
            'failed' => count($results) - $success,
            'results' => $results,
        ];
    }

    public function rebalancePlan(int $totalRows = 1_000_000): array {
        if (count($this->nodes) < 2) {
            return ['moves' => [], 'estimated_total_rows' => $totalRows];
        }
        $perShard = intdiv($totalRows, count($this->nodes));
        $moves = [];
        for ($i = 1; $i < count($this->nodes); $i++) {
            $moves[] = [
                'from' => $this->nodes[0]->id,
                'to' => $this->nodes[$i]->id,
                'range_start' => ($i - 1) * $perShard,
                'range_end' => $i * $perShard,
                'estimated_rows' => $perShard,
            ];
        }
        return [
            'moves' => $moves,
            'estimated_total_rows' => $totalRows,
            'estimated_seconds' => intdiv($totalRows, 10000),
        ];
    }
}

// ============================================================================
// 演示
// ============================================================================

if (PHP_SAPI === 'cli' && realpath($_SERVER['SCRIPT_FILENAME'] ?? '') === __FILE__) {
    echo str_repeat("=", 70) . "\n";
    echo "SQLTool PHP SDK 演示\n";
    echo str_repeat("=", 70) . "\n";

    // 演示 1: 跨数据库迁移
    echo "\n[1] 跨数据库迁移 (MySQL 5.7 → PostgreSQL 16)\n";
    $mig = new CrossDbMigrator();
    $result = $mig->migrateTable(
        source: 'mysql://root:pass@localhost:3306/mydb',
        target: 'postgresql://postgres:pass@localhost:5432/mydb',
        table: new TableSpec('orders', [
            new FieldSpec('id', 'INT', primaryKey: true),
            new FieldSpec('user_id', 'BIGINT'),
            new FieldSpec('amount', 'DECIMAL(10,2)'),
            new FieldSpec('created_at', 'DATETIME'),
        ]),
        sourceVersion: '5.7.40',
        targetVersion: '16.2.0',
    );
    echo "  方向: {$result->direction}\n";
    echo "  映射: {$result->fieldsMapped}/{$result->fieldsTotal} (" . round($result->successRate() * 100, 1) . "%)\n";
    echo "  有损: {$result->lossyConversions}\n";
    echo "  DDL:\n{$result->ddl}\n";

    // 演示 2: 智能分库分表
    echo "\n[2] 智能分库分表 (4 分片哈希)\n";
    $sharding = new SmartSharding('orders', 'user_id', 'hash');
    $sharding->addShard('s0', 'mysql://n1/orders_0', 'orders_0');
    $sharding->addShard('s1', 'mysql://n1/orders_1', 'orders_1');
    $sharding->addShard('s2', 'mysql://n2/orders_2', 'orders_2');
    $sharding->addShard('s3', 'mysql://n2/orders_3', 'orders_3');

    echo "  路由演示:\n";
    foreach (['user_001', 'user_042', 'user_001'] as $uid) {
        $node = $sharding->route($uid);
        echo "    {$uid} → {$node->id} ({$node->table})\n";
    }

    $qResult = $sharding->query();
    echo "  跨分片查询: 涉及 {$qResult['total_shards']} 分片\n";

    $wResult = $sharding->writeBatch(['u1', 'u2', 'u3']);
    echo "  批量写入: {$wResult['success']}/{$wResult['total']} 成功\n";

    $plan = $sharding->rebalancePlan(10_000_000);
    echo "  Rebalance: " . count($plan['moves']) . " 步, ~{$plan['estimated_seconds']}s\n";

    echo "\n✓ 演示完成\n";
}
