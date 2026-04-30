// SQLTool Go 调用完整示例
//
// 功能覆盖：
//   - 数据迁移 (transfer)
//   - 数据备份 (backup)
//   - 数据对比 (compare-data)
//   - 分库分表 (create-shard)
//   - 慢查询检测 (detect-slow-query)
//   - 跨分片查询 (spanning-query)
//   - 日志管理 (insert-log/query-logs)
//   - SQL注入检测 (detect-sql-injection)
//   - 安全SQL构建 (build-safe-sql)
//
// 安装依赖:
//   go mod init sqltool-demo
//   go get github.com/kodephp/sqltool
//
// 使用方法:
//   go run sqltool_demo.go --cli                    # CLI 模式 (无需启动server)
//   go run sqltool_demo.go                           # HTTP API 模式 (需启动server)

package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"strings"
	"time"
)

// =============================================================================
// HTTP API 客户端
// =============================================================================

type SqlToolClient struct {
	BaseURL string
	Client  *http.Client
	APIKey  string
}

func NewSqlToolClient(baseURL string) *SqlToolClient {
	return &SqlToolClient{
		BaseURL: strings.TrimSuffix(baseURL, "/"),
		Client:  &http.Client{Timeout: 60 * time.Second},
	}
}

func (c *SqlToolClient) SetAPIKey(key string) {
	c.APIKey = key
}

func (c *SqlToolClient) doRequest(method, path string, body interface{}) (map[string]interface{}, error) {
	var reqBody *bytes.Buffer
	if body != nil {
		jsonBytes, _ := json.Marshal(body)
		reqBody = bytes.NewBuffer(jsonBytes)
	} else {
		reqBody = bytes.NewBuffer(nil)
	}

	req, err := http.NewRequest(method, c.BaseURL+path, reqBody)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	if c.APIKey != "" {
		req.Header.Set("Authorization", "Bearer "+c.APIKey)
	}

	resp, err := c.Client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var result map[string]interface{}
	json.NewDecoder(resp.Body).Decode(&result)
	return result, nil
}

// -----------------------------------------------------------------------------
// 健康检查
// -----------------------------------------------------------------------------

func (c *SqlToolClient) HealthCheck() (map[string]interface{}, error) {
	return c.doRequest("GET", "/api/health", nil)
}

// -----------------------------------------------------------------------------
// 数据迁移
// -----------------------------------------------------------------------------

type TransferRequest struct {
	Source      string `json:"source"`       // 源数据库连接字符串
	Target      string `json:"target"`       // 目标数据库连接字符串
	SourceType  string `json:"source_type"`  // mysql/postgresql/sqlite/oracle
	TargetType  string `json:"target_type"`  // mysql/postgresql/sqlite/oracle
	Tables      string `json:"tables"`       // 表名列表，逗号分隔，空表示所有表
	BatchSize   int    `json:"batch_size"`   // 批量大小，默认1000
	VerifyData  bool   `json:"verify_data"`  // 是否验证数据
	SkipErrors  bool   `json:"skip_errors"` // 是否跳过错误
}

type TransferResponse struct {
	Success         bool    `json:"success"`
	RowsTransferred int64   `json:"rows_transferred"`
	Duration        float64 `json:"duration_seconds"`
	Errors          []string `json:"errors,omitempty"`
}

// 数据迁移 - MySQL -> PostgreSQL
func (c *SqlToolClient) Transfer(req TransferRequest) (*TransferResponse, error) {
	result, err := c.doRequest("POST", "/api/transfer", req)
	if err != nil {
		return nil, err
	}

	resp := &TransferResponse{}
	jsonBytes, _ := json.Marshal(result)
	json.Unmarshal(jsonBytes, resp)
	return resp, nil
}

// 迁移示例
func Example_Transfer() {
	client := NewSqlToolClient("http://localhost:8080")

	req := TransferRequest{
		Source:     "mysql://root:password@localhost:3306/source_db",
		Target:     "postgresql://postgres:password@localhost:5432/target_db",
		SourceType: "mysql",
		TargetType: "postgresql",
		Tables:     "users,orders,products",
		BatchSize:  5000,
		VerifyData: true,
		SkipErrors: true,
	}

	resp, err := client.Transfer(req)
	if err != nil {
		fmt.Printf("迁移失败: %v\n", err)
		return
	}
	fmt.Printf("迁移成功: %d 行, 耗时: %.2f秒\n", resp.RowsTransferred, resp.Duration)
}

// -----------------------------------------------------------------------------
// 数据库备份
// -----------------------------------------------------------------------------

type BackupRequest struct {
	Source             string `json:"source"`              // 数据库连接字符串
	DbType             string `json:"db_type"`             // mysql/postgresql/sqlite
	Output             string `json:"output"`              // 备份文件路径
	BackupType         string `json:"backup_type"`         // full/incremental/differential
	Compress           bool   `json:"compress"`            // 是否压缩
	IncludeProcedures  bool   `json:"include_procedures"`  // 包含存储过程
	IncludeFunctions   bool   `json:"include_functions"`    // 包含函数
	IncludeTriggers    bool   `json:"include_triggers"`    // 包含触发器
	ParallelTables     int    `json:"parallel_tables"`      // 并行备份表数
}

type BackupResponse struct {
	Success    bool    `json:"success"`
	FilePath   string  `json:"file_path"`
	SizeBytes  int64   `json:"size_bytes"`
	Duration   float64 `json:"duration_seconds"`
}

// 数据库备份
func (c *SqlToolClient) Backup(req BackupRequest) (*BackupResponse, error) {
	result, err := c.doRequest("POST", "/api/backup", req)
	if err != nil {
		return nil, err
	}

	resp := &BackupResponse{}
	jsonBytes, _ := json.Marshal(result)
	json.Unmarshal(jsonBytes, resp)
	return resp, nil
}

// 备份示例
func Example_Backup() {
	client := NewSqlToolClient("http://localhost:8080")

	req := BackupRequest{
		Source:            "mysql://root:password@localhost:3306/mydb",
		DbType:            "mysql",
		Output:            "/tmp/backup_20240101.sql",
		BackupType:        "full",
		Compress:          true,
		IncludeProcedures: true,
		IncludeFunctions:  true,
		IncludeTriggers:   true,
		ParallelTables:    4,
	}

	resp, err := client.Backup(req)
	if err != nil {
		fmt.Printf("备份失败: %v\n", err)
		return
	}
	fmt.Printf("备份成功: %s (%.2fMB), 耗时: %.2f秒\n",
		resp.FilePath, float64(resp.SizeBytes)/1024/1024, resp.Duration)
}

// -----------------------------------------------------------------------------
// 数据对比
// -----------------------------------------------------------------------------

type CompareRequest struct {
	Source       string `json:"source"`        // 源数据库
	Target       string `json:"target"`        // 目标数据库
	SourceType   string `json:"source_type"`   // 源类型
	TargetType   string `json:"target_type"`   // 目标类型
	Table        string `json:"table"`         // 表名
	PrimaryKey   string `json:"primary_key"`   // 主键字段
	IgnoreFields string `json:"ignore_fields"` // 忽略字段
	CompareMode  string `json:"compare_mode"`  // full/sample
}

type CompareResponse struct {
	Success       bool    `json:"success"`
	TotalRows     int64   `json:"total_rows"`
	MatchedRows   int64   `json:"matched_rows"`
	DiffRows      int64   `json:"diff_rows"`
	MatchPercent  float64 `json:"match_percent"`
	DiffDetails   []DiffDetail `json:"diff_details,omitempty"`
}

type DiffDetail struct {
	RowKey   string      `json:"row_key"`
	Field    string      `json:"field"`
	SourceValue interface{} `json:"source_value"`
	TargetValue interface{} `json:"target_value"`
}

// 数据对比
func (c *SqlToolClient) CompareData(req CompareRequest) (*CompareResponse, error) {
	result, err := c.doRequest("POST", "/api/compare", req)
	if err != nil {
		return nil, err
	}

	resp := &CompareResponse{}
	jsonBytes, _ := json.Marshal(result)
	json.Unmarshal(jsonBytes, resp)
	return resp, nil
}

// 对比示例
func Example_Compare() {
	client := NewSqlToolClient("http://localhost:8080")

	req := CompareRequest{
		Source:       "mysql://root:password@localhost:3306/db1",
		Target:       "mysql://root:password@localhost:3306/db2",
		SourceType:   "mysql",
		TargetType:   "mysql",
		Table:        "users",
		PrimaryKey:   "id",
		IgnoreFields: "updated_at",
		CompareMode:  "full",
	}

	resp, err := client.CompareData(req)
	if err != nil {
		fmt.Printf("对比失败: %v\n", err)
		return
	}
	fmt.Printf("对比结果: %d/%d 匹配 (%.2f%%)\n",
		resp.MatchedRows, resp.TotalRows, resp.MatchPercent)
}

// -----------------------------------------------------------------------------
// 分库分表
// -----------------------------------------------------------------------------

type ShardRequest struct {
	Source    string `json:"source"`     // 数据库连接
	Table     string `json:"table"`      // 表名
	Strategy  string `json:"strategy"`   // row_count/time/size/hash
	Threshold string `json:"threshold"`  // 阈值
	Prefix    string `json:"prefix"`     // 分片前缀
}

type ShardResponse struct {
	Success     bool     `json:"success"`
	ShardCount  int      `json:"shard_count"`
	ShardNames  []string `json:"shard_names"`
	Distribution []int64  `json:"distribution"`
}

// 创建分片
func (c *SqlToolClient) CreateShard(req ShardRequest) (*ShardResponse, error) {
	result, err := c.doRequest("POST", "/api/shard/create", req)
	if err != nil {
		return nil, err
	}

	resp := &ShardResponse{}
	jsonBytes, _ := json.Marshal(result)
	json.Unmarshal(jsonBytes, resp)
	return resp, nil
}

// 创建分片示例
func Example_CreateShard() {
	client := NewSqlToolClient("http://localhost:8080")

	req := ShardRequest{
		Source:    "mysql://root:password@localhost:3306/mydb",
		Table:     "orders",
		Strategy:  "row_count",
		Threshold: "1000000",
		Prefix:    "orders_shard",
	}

	resp, err := client.CreateShard(req)
	if err != nil {
		fmt.Printf("分片失败: %v\n", err)
		return
	}
	fmt.Printf("创建分片成功: %d 个分片\n", resp.ShardCount)
	for i, name := range resp.ShardNames {
		fmt.Printf("  %s: %d 行\n", name, resp.Distribution[i])
	}
}

// -----------------------------------------------------------------------------
// 慢查询检测
// -----------------------------------------------------------------------------

type SlowQueryRequest struct {
	Source      string `json:"source"`       // 数据库连接
	DbType      string `json:"db_type"`     // 数据库类型
	ThresholdMs int64  `json:"threshold_ms"` // 阈值(毫秒)
	Limit       int    `json:"limit"`       // 返回数量
}

type SlowQueryResponse struct {
	Success     bool           `json:"success"`
	QueryCount int            `json:"query_count"`
	Queries     []SlowQueryInfo `json:"queries"`
}

type SlowQueryInfo struct {
	SQL          string  `json:"sql"`
	ExecutionMs  int64   `json:"execution_ms"`
	RowsExamined int64   `json:"rows_examined"`
	Suggestions  []string `json:"suggestions"`
}

// 慢查询检测
func (c *SqlToolClient) DetectSlowQuery(req SlowQueryRequest) (*SlowQueryResponse, error) {
	result, err := c.doRequest("POST", "/api/detect-slow", req)
	if err != nil {
		return nil, err
	}

	resp := &SlowQueryResponse{}
	jsonBytes, _ := json.Marshal(result)
	json.Unmarshal(jsonBytes, resp)
	return resp, nil
}

// 慢查询检测示例
func Example_DetectSlowQuery() {
	client := NewSqlToolClient("http://localhost:8080")

	req := SlowQueryRequest{
		Source:      "mysql://root:password@localhost:3306/mydb",
		DbType:      "mysql",
		ThresholdMs: 1000,
		Limit:       10,
	}

	resp, err := client.DetectSlowQuery(req)
	if err != nil {
		fmt.Printf("检测失败: %v\n", err)
		return
	}
	fmt.Printf("发现 %d 条慢查询\n", resp.QueryCount)
	for _, q := range resp.Queries {
		fmt.Printf("  [%dms] %s\n", q.ExecutionMs, q.SQL)
		for _, s := range q.Suggestions {
			fmt.Printf("    建议: %s\n", s)
		}
	}
}

// -----------------------------------------------------------------------------
// 跨分片查询
// -----------------------------------------------------------------------------

type SpanningQueryRequest struct {
	Source    string `json:"source"`    // 数据库连接
	Table     string `json:"table"`     // 表名
	Condition string `json:"condition"`  // WHERE条件
	OrderBy   string `json:"order_by"`  // 排序字段
	OrderDir  string `json:"order_dir"` // ASC/DESC
	Limit     int    `json:"limit"`     // 返回数量
	Offset    int    `json:"offset"`    // 偏移量
}

type SpanningQueryResponse struct {
	Success    bool        `json:"success"`
	TotalRows  int64       `json:"total_rows"`
	Rows       []map[string]interface{} `json:"rows"`
	DurationMs int64       `json:"duration_ms"`
}

// 跨分片查询
func (c *SqlToolClient) SpanningQuery(req SpanningQueryRequest) (*SpanningQueryResponse, error) {
	result, err := c.doRequest("POST", "/api/spanning-query", req)
	if err != nil {
		return nil, err
	}

	resp := &SpanningQueryResponse{}
	jsonBytes, _ := json.Marshal(result)
	json.Unmarshal(jsonBytes, resp)
	return resp, nil
}

// 跨分片查询示例
func Example_SpanningQuery() {
	client := NewSqlToolClient("http://localhost:8080")

	req := SpanningQueryRequest{
		Source:    "mysql://root:password@localhost:3306/mydb",
		Table:    "orders",
		Condition: "status = 'pending'",
		OrderBy:   "created_at",
		OrderDir:  "DESC",
		Limit:     100,
		Offset:    0,
	}

	resp, err := client.SpanningQuery(req)
	if err != nil {
		fmt.Printf("查询失败: %v\n", err)
		return
	}
	fmt.Printf("查询成功: %d 行 (耗时: %dms)\n", resp.TotalRows, resp.DurationMs)
}

// -----------------------------------------------------------------------------
// 日志管理
// -----------------------------------------------------------------------------

type InsertLogRequest struct {
	Source     string `json:"source"`      // 数据库连接
	Table      string `json:"table"`       // 日志表名
	Level      string `json:"level"`       // DEBUG/INFO/WARN/ERROR
	Message    string `json:"message"`     // 日志消息
	SourceName string `json:"source_name"` // 来源名称
}

type QueryLogsRequest struct {
	Source     string `json:"source"`     // 数据库连接
	Table      string `json:"table"`      // 日志表名
	Levels     string `json:"levels"`      // 级别过滤
	Keyword    string `json:"keyword"`     // 关键字过滤
	StartTime  int64  `json:"start_time"` // 开始时间
	EndTime    int64  `json:"end_time"`   // 结束时间
	Limit      int    `json:"limit"`       // 返回数量
}

// 插入日志
func (c *SqlToolClient) InsertLog(req InsertLogRequest) (map[string]interface{}, error) {
	return c.doRequest("POST", "/api/log/insert", req)
}

// 插入日志示例
func Example_InsertLog() {
	client := NewSqlToolClient("http://localhost:8080")

	req := InsertLogRequest{
		Source:     "mysql://root:password@localhost:3306/mydb",
		Table:      "app_logs",
		Level:      "INFO",
		Message:    "用户登录成功",
		SourceName: "auth-service",
	}

	result, err := client.InsertLog(req)
	if err != nil {
		fmt.Printf("插入失败: %v\n", err)
		return
	}
	fmt.Printf("日志插入成功: %v\n", result)
}

// 查询日志
func (c *SqlToolClient) QueryLogs(req QueryLogsRequest) ([]map[string]interface{}, error) {
	result, err := c.doRequest("POST", "/api/log/query", req)
	if err != nil {
		return nil, err
	}

	if rows, ok := result["rows"].([]map[string]interface{}); ok {
		return rows, nil
	}
	return nil, nil
}

// 查询日志示例
func Example_QueryLogs() {
	client := NewSqlToolClient("http://localhost:8080")

	req := QueryLogsRequest{
		Source:    "mysql://root:password@localhost:3306/mydb",
		Table:     "app_logs",
		Levels:    "ERROR,WARN",
		Keyword:   "login",
		Limit:     100,
	}

	rows, err := client.QueryLogs(req)
	if err != nil {
		fmt.Printf("查询失败: %v\n", err)
		return
	}
	fmt.Printf("查询到 %d 条日志\n", len(rows))
	for _, row := range rows {
		fmt.Printf("  [%s] %s\n", row["level"], row["message"])
	}
}

// -----------------------------------------------------------------------------
// SQL注入检测
// -----------------------------------------------------------------------------

type InjectionRequest struct {
	Input string `json:"input"`
}

type InjectionResponse struct {
	Success   bool     `json:"success"`
	RiskLevel string   `json:"risk_level"` // Low/Medium/High/Critical
	Findings  []Finding `json:"findings"`
}

type Finding struct {
	Pattern     string `json:"pattern"`
	Description string `json:"description"`
}

// SQL注入检测
func (c *SqlToolClient) DetectInjection(input string) (*InjectionResponse, error) {
	result, err := c.doRequest("POST", "/api/security/detect-injection", InjectionRequest{Input: input})
	if err != nil {
		return nil, err
	}

	resp := &InjectionResponse{}
	jsonBytes, _ := json.Marshal(result)
	json.Unmarshal(jsonBytes, resp)
	return resp, nil
}

// SQL注入检测示例
func Example_DetectInjection() {
	client := NewSqlToolClient("http://localhost:8080")

	// 检测恶意输入
	resp, err := client.DetectInjection("' OR '1'='1")
	if err != nil {
		fmt.Printf("检测失败: %v\n", err)
		return
	}
	fmt.Printf("风险等级: %s\n", resp.RiskLevel)
	for _, f := range resp.Findings {
		fmt.Printf("  - %s: %s\n", f.Pattern, f.Description)
	}
}

// -----------------------------------------------------------------------------
// 安全SQL构建
// -----------------------------------------------------------------------------

type SafeSqlRequest struct {
	Table    string `json:"table"`
	Field    string `json:"field"`
	Operator string `json:"operator"` // =, !=, <, >, LIKE, IN
	Value    string `json:"value"`
}

type SafeSqlResponse struct {
	Success bool   `json:"success"`
	SQL     string `json:"sql"`
	Error   string `json:"error,omitempty"`
}

// 安全SQL构建
func (c *SqlToolClient) BuildSafeSql(req SafeSqlRequest) (*SafeSqlResponse, error) {
	result, err := c.doRequest("POST", "/api/security/build-safe-sql", req)
	if err != nil {
		return nil, err
	}

	resp := &SafeSqlResponse{}
	jsonBytes, _ := json.Marshal(result)
	json.Unmarshal(jsonBytes, resp)
	return resp, nil
}

// 安全SQL构建示例
func Example_BuildSafeSql() {
	client := NewSqlToolClient("http://localhost:8080")

	// 安全构建查询
	resp, err := client.BuildSafeSql(SafeSqlRequest{
		Table:    "users",
		Field:    "name",
		Operator: "=",
		Value:    "John O'Brien",
	})
	if err != nil {
		fmt.Printf("构建失败: %v\n", err)
		return
	}
	if resp.Success {
		fmt.Printf("安全SQL: %s\n", resp.SQL)
	} else {
		fmt.Printf("构建失败: %s\n", resp.Error)
	}
}

// =============================================================================
// CLI 客户端
// =============================================================================

type SqlToolCLI struct {
	binaryPath string
}

func NewSqlToolCLI(binaryPath string) *SqlToolCLI {
	if binaryPath == "" {
		binaryPath = "sqltool"
	}
	return &SqlToolCLI{binaryPath: binaryPath}
}

func (cli *SqlToolCLI) run(timeout time.Duration, args ...string) (string, error) {
	ctx := context.Background()
	if timeout > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, timeout)
		defer cancel()
	}

	cmd := exec.CommandContext(ctx, cli.binaryPath, args...)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("命令执行失败: %v, 输出: %s", err, string(output))
	}
	return string(output), nil
}

// CLI 命令封装

func (cli *SqlToolCLI) Transfer(source, target, sourceType, targetType string, tables string, batchSize int) string {
	args := []string{"transfer", "-s", source, "-t", target, "-S", sourceType, "-T", targetType}
	if tables != "" {
		args = append(args, "--tables", tables)
	}
	if batchSize > 0 {
		args = append(args, "-B", fmt.Sprintf("%d", batchSize))
	}
	result, _ := cli.run(0, args...)
	return result
}

func (cli *SqlToolCLI) Backup(source, dbType, output, backupType string, compress bool) string {
	args := []string{"backup", "-s", source, "-T", dbType, "-o", output, "-t", backupType}
	if compress {
		args = append(args, "-c")
	}
	result, _ := cli.run(0, args...)
	return result
}

func (cli *SqlToolCLI) CompareData(source, target, sourceType, targetType, table, primaryKey string) string {
	args := []string{"compare-data", "-s", source, "-t", target, "-S", sourceType, "-T", targetType, "--table", table, "--primary-key", primaryKey}
	result, _ := cli.run(0, args...)
	return result
}

func (cli *SqlToolCLI) CreateShard(source, table, strategy, threshold, prefix string) string {
	args := []string{"create-shard", "-s", source, "--table", table, "--strategy", strategy}
	if threshold != "" {
		args = append(args, "--threshold", threshold)
	}
	if prefix != "" {
		args = append(args, "--prefix", prefix)
	}
	result, _ := cli.run(0, args...)
	return result
}

func (cli *SqlToolCLI) DetectSlowQuery(source, dbType string, thresholdMs int64) string {
	args := []string{"detect-slow-query", "-s", source, "-T", dbType, "--threshold-ms", fmt.Sprintf("%d", thresholdMs)}
	result, _ := cli.run(0, args...)
	return result
}

func (cli *SqlToolCLI) SpanningQuery(source, table, condition string, orderBy string, limit, offset int) string {
	args := []string{"spanning-query", "-s", source, "--table", table, "--condition", condition}
	if orderBy != "" {
		args = append(args, "--order-by", orderBy)
	}
	args = append(args, "-L", fmt.Sprintf("%d", limit), "--offset", fmt.Sprintf("%d", offset))
	result, _ := cli.run(0, args...)
	return result
}

func (cli *SqlToolCLI) InsertLog(source, table, level, message string) string {
	args := []string{"insert-log", "-s", source, "--table", table, "--level", level, "--message", message}
	result, _ := cli.run(0, args...)
	return result
}

func (cli *SqlToolCLI) QueryLogs(source, table, levels, keyword string, limit int) string {
	args := []string{"query-logs", "-s", source, "--table", table, "-L", fmt.Sprintf("%d", limit)}
	if levels != "" {
		args = append(args, "--levels", levels)
	}
	if keyword != "" {
		args = append(args, "--keyword", keyword)
	}
	result, _ := cli.run(0, args...)
	return result
}

func (cli *SqlToolCLI) DetectInjection(input string) string {
	result, _ := cli.run(0, "detect-sql-injection", "-i", input)
	return result
}

func (cli *SqlToolCLI) BuildSafeSql(table, field, operator, value string) string {
	result, _ := cli.run(0, "build-safe-sql", "--table", table, "--field", field, "--operator", operator, "--value", value)
	return result
}

// =============================================================================
// 主函数
// =============================================================================

import (
	"context"
)

func printResult(title string, result interface{}) {
	fmt.Println("\n" + strings.Repeat("=", 60))
	fmt.Println(title)
	fmt.Println(strings.Repeat("=", 60))
	if jsonBytes, err := json.MarshalIndent(result, "", "  "); err == nil {
		fmt.Println(string(jsonBytes))
	} else {
		fmt.Println(result)
	}
}

func main() {
	useCLI := len(os.Args) > 1 && os.Args[1] == "--cli"

	fmt.Println(`
╔══════════════════════════════════════════════════════════════╗
║          SQLTool Go 完整调用示例 v0.4.1                      ║
╚══════════════════════════════════════════════════════════════╝
`)

	if useCLI {
		fmt.Println("模式: CLI (不需要启动 server)")
		fmt.Println("二进制路径: /Users/Zhuanz/Desktop/website/composer/sqlmap/target/release/sqltool\n")

		cli := NewSqlToolCLI("/Users/Zhuanz/Desktop/website/composer/sqlmap/target/release/sqltool")

		// 1. SQL注入检测
		fmt.Println("1. SQL注入检测...")
		result := cli.DetectInjection("' OR '1'='1")
		printResult("SQL注入检测结果", result)

		// 2. 安全SQL构建
		fmt.Println("2. 安全SQL构建...")
		result = cli.BuildSafeSql("users", "name", "=", "test'; DROP TABLE")
		printResult("安全SQL构建结果", result)

		// 3. 数据迁移 (示例命令)
		fmt.Println("3. 数据迁移 (CLI)...")
		result = cli.Transfer(
			"mysql://root:pass@localhost:3306/source",
			"postgresql://postgres:pass@localhost:5432/target",
			"mysql", "postgresql", "users,orders", 5000)
		printResult("数据迁移结果", result)

		// 4. 数据库备份 (示例命令)
		fmt.Println("4. 数据库备份 (CLI)...")
		result = cli.Backup(
			"mysql://root:pass@localhost:3306/mydb",
			"mysql", "/tmp/backup.sql", "full", true)
		printResult("备份结果", result)

	} else {
		fmt.Println("模式: HTTP API (需要启动 sqltool server)")
		fmt.Println("服务地址: http://localhost:8080\n")

		client := NewSqlToolClient("http://localhost:8080")

		// 健康检查
		fmt.Println("0. 健康检查...")
		result, err := client.HealthCheck()
		if err != nil {
			fmt.Printf("\n错误: 无法连接到 http://localhost:8080\n")
			fmt.Println("请先启动 sqltool server:")
			fmt.Println("  sqltool server -p 8080 -s mysql://localhost/mydb")
			return
		}
		printResult("健康检查", result)

		// 1. SQL注入检测
		fmt.Println("1. SQL注入检测 - 恶意输入...")
		resp, _ := client.DetectInjection("' OR '1'='1")
		printResult("检测结果", resp)

		// 2. 安全SQL构建
		fmt.Println("2. 安全SQL构建...")
		sqlResp, _ := client.BuildSafeSql(SafeSqlRequest{
			Table:    "users",
			Field:    "email",
			Operator: "LIKE",
			Value:    "%@example.com",
		})
		printResult("构建结果", sqlResp)

		// 3. 数据迁移示例
		fmt.Println("3. 数据迁移 (HTTP API)...")
		Example_Transfer()

		// 4. 数据库备份示例
		fmt.Println("4. 数据库备份 (HTTP API)...")
		Example_Backup()

		// 5. 数据对比示例
		fmt.Println("5. 数据对比 (HTTP API)...")
		Example_Compare()

		// 6. 分库分表示例
		fmt.Println("6. 分库分表 (HTTP API)...")
		Example_CreateShard()

		// 7. 慢查询检测示例
		fmt.Println("7. 慢查询检测 (HTTP API)...")
		Example_DetectSlowQuery()

		// 8. 跨分片查询示例
		fmt.Println("8. 跨分片查询 (HTTP API)...")
		Example_SpanningQuery()

		// 9. 插入日志示例
		fmt.Println("9. 插入日志 (HTTP API)...")
		Example_InsertLog()

		// 10. 查询日志示例
		fmt.Println("10. 查询日志 (HTTP API)...")
		Example_QueryLogs()
	}

	fmt.Println("\n" + strings.Repeat("=", 60))
	fmt.Println("示例执行完成!")
	fmt.Println(strings.Repeat("=", 60))
}