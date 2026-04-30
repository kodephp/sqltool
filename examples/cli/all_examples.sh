#!/bin/bash
# =============================================================================
# SQLTool Shell (Bash) 完整调用示例
#
# 功能覆盖：
#   - 数据迁移 (transfer)
#   - 数据库备份 (backup)
#   - 数据对比 (compare-data)
#   - 分库分表 (create-shard)
#   - 慢查询检测 (detect-slow-query)
#   - 跨分片查询 (spanning-query)
#   - 日志管理 (insert-log/query-logs)
#   - SQL注入检测 (detect-sql-injection)
#   - 安全SQL构建 (build-safe-sql)
#
# 使用方法:
#   ./all_examples.sh                    # HTTP API 模式
#   ./all_examples.sh --cli            # CLI 模式
# =============================================================================

set -e

BINARY_PATH="/Users/Zhuanz/Desktop/website/composer/sqlmap/target/release/sqltool"
API_URL="http://localhost:8080"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# =============================================================================
# 工具函数
# =============================================================================

print_header() {
    echo -e "\n${BLUE}$(printf '=%.0s' {1..60})${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}$(printf '=%.0s' {1..60})${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

# =============================================================================
# CLI 调用函数
# =============================================================================

cli_detect_injection() {
    local input="$1"
    "$BINARY_PATH" detect-sql-injection -i "$input"
}

cli_build_safe_sql() {
    local table="$1"
    local field="$2"
    local operator="$3"
    local value="$4"
    "$BINARY_PATH" build-safe-sql --table "$table" --field "$field" --operator "$operator" --value "$value"
}

cli_transfer() {
    local source="$1"
    local target="$2"
    local source_type="$3"
    local target_type="$4"
    local tables="$5"
    local batch_size="${6:-1000}"

    "$BINARY_PATH" transfer \
        -s "$source" \
        -t "$target" \
        -S "$source_type" \
        -T "$target_type" \
        --tables "$tables" \
        -B "$batch_size"
}

cli_backup() {
    local source="$1"
    local output="$2"
    local db_type="$3"
    local backup_type="${4:-full}"
    local compress="${5:-true}"

    local args=(
        "backup"
        "-s" "$source"
        "-o" "$output"
        "-T" "$db_type"
        "-t" "$backup_type"
    )

    if [ "$compress" = "true" ]; then
        args+=("-c")
    fi

    "$BINARY_PATH" "${args[@]}"
}

cli_compare_data() {
    local source="$1"
    local target="$2"
    local table="$3"
    local primary_key="${4:-id}"

    "$BINARY_PATH" compare-data \
        -s "$source" \
        -t "$target" \
        --table "$table" \
        --primary-key "$primary_key"
}

cli_create_shard() {
    local source="$1"
    local table="$2"
    local strategy="$3"
    local threshold="$4"
    local prefix="$5"

    "$BINARY_PATH" create-shard \
        -s "$source" \
        --table "$table" \
        --strategy "$strategy" \
        --threshold "$threshold" \
        --prefix "$prefix"
}

cli_detect_slow_query() {
    local source="$1"
    local db_type="$2"
    local threshold_ms="${3:-1000}"

    "$BINARY_PATH" detect-slow-query \
        -s "$source" \
        -T "$db_type" \
        --threshold-ms "$threshold_ms"
}

cli_spanning_query() {
    local source="$1"
    local table="$2"
    local condition="$3"
    local order_by="$4"
    local limit="${5:-100}"
    local offset="${6:-0}"

    "$BINARY_PATH" spanning-query \
        -s "$source" \
        --table "$table" \
        --condition "$condition" \
        --order-by "$order_by" \
        -L "$limit" \
        --offset "$offset"
}

cli_insert_log() {
    local source="$1"
    local table="$2"
    local level="$3"
    local message="$4"
    local source_name="$5"

    "$BINARY_PATH" insert-log \
        -s "$source" \
        --table "$table" \
        --level "$level" \
        --message "$message" \
        --source-name "$source_name"
}

cli_query_logs() {
    local source="$1"
    local table="$2"
    local levels="$3"
    local keyword="$4"
    local limit="${5:-100}"

    local args=(
        "query-logs"
        "-s" "$source"
        "--table" "$table"
        "-L" "$limit"
    )

    [ -n "$levels" ] && args+=("--levels" "$levels")
    [ -n "$keyword" ] && args+=("--keyword" "$keyword")

    "$BINARY_PATH" "${args[@]}"
}

# =============================================================================
# HTTP API 调用函数
# =============================================================================

api_request() {
    local method="$1"
    local path="$2"
    local data="$3"

    if [ -n "$data" ]; then
        curl -s -X "$method" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "${API_URL}${path}"
    else
        curl -s -X "$method" \
            -H "Content-Type: application/json" \
            "${API_URL}${path}"
    fi
}

api_health_check() {
    api_request "GET" "/api/health"
}

api_detect_injection() {
    local input="$1"
    api_request "POST" "/api/security/detect-injection" "{\"input\":\"$input\"}"
}

api_build_safe_sql() {
    local table="$1"
    local field="$2"
    local operator="$3"
    local value="$4"
    api_request "POST" "/api/security/build-safe-sql" \
        "{\"table\":\"$table\",\"field\":\"$field\",\"operator\":\"$operator\",\"value\":\"$value\"}"
}

api_transfer() {
    local source="$1"
    local target="$2"
    local source_type="$3"
    local target_type="$4"
    local tables="$5"
    local batch_size="${6:-1000}"

    api_request "POST" "/api/transfer" \
        "{\"source\":\"$source\",\"target\":\"$target\",\"source_type\":\"$source_type\",\"target_type\":\"$target_type\",\"tables\":\"$tables\",\"batch_size\":$batch_size,\"verify_data\":true,\"skip_errors\":true}"
}

api_backup() {
    local source="$1"
    local db_type="$2"
    local output="$3"
    local backup_type="${4:-full}"
    local compress="${5:-true}"

    api_request "POST" "/api/backup" \
        "{\"source\":\"$source\",\"db_type\":\"$db_type\",\"output\":\"$output\",\"backup_type\":\"$backup_type\",\"compress\":$compress}"
}

api_compare_data() {
    local source="$1"
    local target="$2"
    local table="$3"
    local primary_key="${4:-id}"

    api_request "POST" "/api/compare" \
        "{\"source\":\"$source\",\"target\":\"$target\",\"table\":\"$table\",\"primary_key\":\"$primary_key\"}"
}

api_create_shard() {
    local source="$1"
    local table="$2"
    local strategy="$3"
    local threshold="$4"
    local prefix="$5"

    api_request "POST" "/api/shard/create" \
        "{\"source\":\"$source\",\"table\":\"$table\",\"strategy\":\"$strategy\",\"threshold\":\"$threshold\",\"prefix\":\"$prefix\"}"
}

api_detect_slow_query() {
    local source="$1"
    local db_type="$2"
    local threshold_ms="${3:-1000}"
    local limit="${4:-10}"

    api_request "POST" "/api/detect-slow" \
        "{\"source\":\"$source\",\"db_type\":\"$db_type\",\"threshold_ms\":$threshold_ms,\"limit\":$limit}"
}

api_spanning_query() {
    local source="$1"
    local table="$2"
    local condition="$3"
    local order_by="$4"
    local order_dir="${5:-ASC}"
    local limit="${6:-100}"
    local offset="${7:-0}"

    api_request "POST" "/api/spanning-query" \
        "{\"source\":\"$source\",\"table\":\"$table\",\"condition\":\"$condition\",\"order_by\":\"$order_by\",\"order_dir\":\"$order_dir\",\"limit\":$limit,\"offset\":$offset}"
}

api_insert_log() {
    local source="$1"
    local table="$2"
    local level="$3"
    local message="$4"
    local source_name="$5"

    api_request "POST" "/api/log/insert" \
        "{\"source\":\"$source\",\"table\":\"$table\",\"level\":\"$level\",\"message\":\"$message\",\"source_name\":\"$source_name\"}"
}

api_query_logs() {
    local source="$1"
    local table="$2"
    local levels="$3"
    local keyword="$4"
    local limit="${5:-100}"

    api_request "POST" "/api/log/query" \
        "{\"source\":\"$source\",\"table\":\"$table\",\"levels\":\"$levels\",\"keyword\":\"$keyword\",\"limit\":$limit}"
}

# =============================================================================
# 主函数
# =============================================================================

USE_CLI=false
if [ "$1" = "--cli" ]; then
    USE_CLI=true
fi

echo "
╔════════════════════════════════════════════════════════════╗
║         SQLTool Shell 完整调用示例 v0.4.1            ║
╚════════════════════════════════════════════════════════════╝
"

if [ "$USE_CLI" = true ]; then
    echo -e "${YELLOW}模式: CLI${NC}"
    echo -e "${YELLOW}二进制: $BINARY_PATH${NC}\n"

    print_header "1. SQL注入检测"
    cli_detect_injection "' OR '1'='1"
    echo ""

    print_header "2. 安全SQL构建"
    cli_build_safe_sql "users" "name" "=" "test'; DROP TABLE"
    echo ""

    print_header "3. 数据迁移"
    cli_transfer \
        "mysql://root:pass@localhost:3306/source" \
        "postgresql://postgres:pass@localhost:5432/target" \
        "mysql" "postgresql" "users,orders" 5000
    echo ""

    print_header "4. 数据库备份"
    cli_backup \
        "mysql://root:pass@localhost:3306/mydb" \
        "/tmp/backup.sql" "mysql" "full" "true"
    echo ""

    print_header "5. 数据对比"
    cli_compare_data \
        "mysql://root@localhost/db1" \
        "mysql://root@localhost/db2" \
        "users" "id"
    echo ""

else
    echo -e "${YELLOW}模式: HTTP API${NC}"
    echo -e "${YELLOW}URL: $API_URL${NC}\n"

    print_header "0. 健康检查"
    api_health_check
    echo ""

    print_header "1. SQL注入检测 - 恶意输入"
    result=$(api_detect_injection "' OR '1'='1")
    echo "$result"
    if echo "$result" | grep -q '"risk_level":"High"\|"risk_level":"Critical"'; then
        print_warning "检测到高风险SQL注入攻击!"
    fi
    echo ""

    print_header "2. 安全SQL构建"
    api_build_safe_sql "users" "email" "LIKE" "%@example.com"
    echo ""

    print_header "3. 数据迁移 (需要真实数据库连接)"
    api_transfer \
        "mysql://root:password@localhost:3306/source_db" \
        "postgresql://postgres:password@localhost:5432/target_db" \
        "mysql" "postgresql" "users,orders,products" 5000
    echo ""

    print_header "4. 数据库备份 (需要真实数据库连接)"
    api_backup \
        "mysql://root:password@localhost:3306/mydb" \
        "mysql" "/tmp/backup_20240101.sql" "full" "true"
    echo ""

    print_header "5. 数据对比 (需要真实数据库连接)"
    api_compare_data \
        "mysql://root:password@localhost:3306/db1" \
        "mysql://root:password@localhost:3306/db2" \
        "users" "id"
    echo ""

    print_header "6. 分库分表 (需要真实数据库连接)"
    api_create_shard \
        "mysql://root:password@localhost:3306/mydb" \
        "orders" "row_count" "1000000" "orders_shard"
    echo ""

    print_header "7. 慢查询检测 (需要真实数据库连接)"
    api_detect_slow_query \
        "mysql://root:password@localhost:3306/mydb" \
        "mysql" 1000 10
    echo ""

    print_header "8. 跨分片查询 (需要真实数据库连接)"
    api_spanning_query \
        "mysql://root:password@localhost:3306/mydb" \
        "orders" "status='pending'" "created_at" "DESC" 100 0
    echo ""

    print_header "9. 插入日志 (需要真实数据库连接)"
    api_insert_log \
        "mysql://root:password@localhost:3306/mydb" \
        "app_logs" "INFO" "用户登录成功" "auth-service"
    echo ""

    print_header "10. 查询日志 (需要真实数据库连接)"
    api_query_logs \
        "mysql://root:password@localhost:3306/mydb" \
        "app_logs" "ERROR,WARN" "login" 50
    echo ""
fi

print_header "示例执行完成!"
echo ""
