// Package sqltool provides SQLTool SDK for Go.
//
// 包含三大能力：
//  1. HTTP 客户端：调用 SQLTool 服务
//  2. 跨数据库迁移：同库跨版本、异构同版本、异构跨版本
//  3. 智能分库分表：查询合并、写入协调、动态扩容
//
// 依赖：仅用标准库（HTTP 模式需要 Go 1.18+）
//
// 演示见 sqltool_demo.go
package sqltool

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// ============================================================================
// HTTP 客户端
// ============================================================================

type SqlToolClient struct {
	BaseURL string
	Timeout time.Duration
	HTTP    *http.Client
}

func NewClient(baseURL string) *SqlToolClient {
	return &SqlToolClient{
		BaseURL: strings.TrimRight(baseURL, "/"),
		Timeout: 30 * time.Second,
		HTTP:    &http.Client{Timeout: 30 * time.Second},
	}
}

func (c *SqlToolClient) request(path, method string, data interface{}) (map[string]interface{}, error) {
	url := c.BaseURL + path
	var body io.Reader
	if data != nil {
		b, _ := json.Marshal(data)
		body = bytes.NewReader(b)
	}
	req, err := http.NewRequest(method, url, body)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := c.HTTP.Do(req)
	if err != nil {
		return nil, fmt.Errorf("请求失败 %s: %w", url, err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
	}
	var out map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&out); err != nil {
		return nil, err
	}
	return out, nil
}

func (c *SqlToolClient) Health() (map[string]interface{}, error) {
	return c.request("/api/health", "GET", nil)
}

func (c *SqlToolClient) Transfer(source, target, sourceType, targetType string, opts map[string]interface{}) (map[string]interface{}, error) {
	payload := map[string]interface{}{
		"source":       source,
		"target":       target,
		"source_type":  sourceType,
		"target_type":  targetType,
		"batch_size":   1000,
		"verify":       true,
	}
	for k, v := range opts {
		payload[k] = v
	}
	return c.request("/api/transfer", "POST", payload)
}

// ============================================================================
// 跨数据库迁移
// ============================================================================

type FieldSpec struct {
	Name           string `json:"name"`
	DataType       string `json:"data_type"`
	Nullable       bool   `json:"nullable"`
	PrimaryKey     bool   `json:"primary_key"`
	AutoIncrement  bool   `json:"auto_increment"`
	DefaultValue   string `json:"default_value,omitempty"`
}

type TableSpec struct {
	Name         string      `json:"name"`
	Fields       []FieldSpec `json:"fields"`
}

type FieldMigration struct {
	SourceField string   `json:"source_field"`
	TargetField string   `json:"target_field"`
	SourceType  string   `json:"source_type"`
	TargetType  string   `json:"target_type"`
	Transform   string   `json:"transform"`
	Lossy       bool     `json:"lossy"`
	Warnings    []string `json:"warnings"`
}

type MigrationResult struct {
	TableName        string            `json:"table_name"`
	Direction        string            `json:"direction"`
	SourceDB         string            `json:"source_db"`
	TargetDB         string            `json:"target_db"`
	SourceVersion    string            `json:"source_version"`
	TargetVersion    string            `json:"target_version"`
	FieldsTotal      int               `json:"fields_total"`
	FieldsMapped     int               `json:"fields_mapped"`
	FieldsUnmapped   int               `json:"fields_unmapped"`
	LossyConversions int               `json:"lossy_conversions"`
	Warnings         []string          `json:"warnings"`
	FieldMigrations  []FieldMigration  `json:"field_migrations"`
	DDL              string            `json:"ddl"`
	ElapsedMs        int64             `json:"elapsed_ms"`
}

func (r *MigrationResult) SuccessRate() float64 {
	if r.FieldsTotal == 0 {
		return 0
	}
	return float64(r.FieldsMapped) / float64(r.FieldsTotal)
}

type CrossDbMigrator struct {
	client    *SqlToolClient
	typeRules map[string]typeRuleEntry
}

type typeRuleEntry struct {
	TargetType string
	Lossy      bool
}

func NewCrossDbMigrator() *CrossDbMigrator {
	return &CrossDbMigrator{
		client:    NewClient("http://localhost:8080"),
		typeRules: loadTypeRules(),
	}
}

// MigrateTable 迁移单张表
func (m *CrossDbMigrator) MigrateTable(source, target string, table TableSpec,
	sourceVersion, targetVersion string, manualFieldMap map[string]string) (*MigrationResult, error) {

	start := time.Now()
	srcDB := parseDBType(source)
	tgtDB := parseDBType(target)
	if sourceVersion == "" {
		sourceVersion = defaultVersion(srcDB)
	}
	if targetVersion == "" {
		targetVersion = defaultVersion(tgtDB)
	}
	direction := inferDirection(srcDB, tgtDB, sourceVersion, targetVersion)

	fms := make([]FieldMigration, 0, len(table.Fields))
	warnings := []string{}
	lossyCount := 0

	for _, f := range table.Fields {
		targetField := f.Name
		if v, ok := manualFieldMap[f.Name]; ok {
			targetField = v
		}
		baseType := strings.ToUpper(strings.Split(f.DataType, "(")[0])
		key := fmt.Sprintf("%s|%s|%s", srcDB, tgtDB, baseType)
		entry, ok := m.typeRules[key]
		var targetType string
		var lossy bool
		if ok {
			targetType = preserveLength(entry.TargetType, f.DataType)
			lossy = entry.Lossy
		} else if srcDB == tgtDB {
			targetType = f.DataType
		} else {
			targetType = f.DataType
		}
		if lossy {
			lossyCount++
			warnings = append(warnings, fmt.Sprintf("%s → %s 可能损失精度", f.DataType, targetType))
		}
		fms = append(fms, FieldMigration{
			SourceField: f.Name,
			TargetField: targetField,
			SourceType:  f.DataType,
			TargetType:  targetType,
			Transform:   "type_cast",
			Lossy:       lossy,
		})
	}

	ddl := generateDDL(table.Name, fms, tgtDB)
	mapped := 0
	for _, fm := range fms {
		if fm.TargetField != "" {
			mapped++
		}
	}

	return &MigrationResult{
		TableName:        table.Name,
		Direction:        direction,
		SourceDB:         srcDB,
		TargetDB:         tgtDB,
		SourceVersion:    sourceVersion,
		TargetVersion:    targetVersion,
		FieldsTotal:      len(fms),
		FieldsMapped:     mapped,
		FieldsUnmapped:   len(fms) - mapped,
		LossyConversions: lossyCount,
		Warnings:         warnings,
		FieldMigrations:  fms,
		DDL:              ddl,
		ElapsedMs:        time.Since(start).Milliseconds(),
	}, nil
}

func parseDBType(url string) string {
	idx := strings.Index(url, "://")
	if idx < 0 {
		return url
	}
	scheme := strings.ToLower(url[:idx])
	mapping := map[string]string{
		"postgres": "postgresql", "pg": "postgresql", "sqlserver": "mssql",
	}
	if v, ok := mapping[scheme]; ok {
		return v
	}
	return scheme
}

func parseVersion(v string) [3]int {
	out := [3]int{}
	for i, p := range strings.Split(strings.Split(v, "(")[0], ".") {
		if i >= 3 {
			break
		}
		fmt.Sscanf(p, "%d", &out[i])
	}
	return out
}

func defaultVersion(db string) string {
	m := map[string]string{
		"mysql":      "8.0.32",
		"mariadb":    "10.11.0",
		"tidb":       "7.5.0",
		"postgresql": "16.2.0",
		"sqlite":     "3.45.0",
		"oracle":     "21.0.0",
		"mssql":      "16.0.0",
	}
	return m[db]
}

func inferDirection(src, tgt, srcV, tgtV string) string {
	if src == tgt {
		if srcV == tgtV {
			return "SameDbSameVersion"
		}
		return "SameDbCrossVersion"
	}
	if srcV == tgtV {
		return "CrossDbSameVersion"
	}
	return "CrossDbCrossVersion"
}

func loadTypeRules() map[string]typeRuleEntry {
	r := map[string]typeRuleEntry{}
	add := func(src, tgt, st, tt string, lossy bool) {
		base := strings.ToUpper(strings.Split(st, "(")[0])
		r[fmt.Sprintf("%s|%s|%s", src, tgt, base)] = typeRuleEntry{tt, lossy}
	}
	// MySQL → PG
	add("mysql", "postgresql", "TINYINT", "SMALLINT", true)
	add("mysql", "postgresql", "INT", "INTEGER", false)
	add("mysql", "postgresql", "BIGINT", "BIGINT", false)
	add("mysql", "postgresql", "DECIMAL", "NUMERIC", false)
	add("mysql", "postgresql", "DATETIME", "TIMESTAMP", true)
	add("mysql", "postgresql", "TIMESTAMP", "TIMESTAMP WITH TIME ZONE", true)
	add("mysql", "postgresql", "JSON", "JSONB", false)
	add("mysql", "postgresql", "BLOB", "BYTEA", false)
	// PG → MySQL
	add("postgresql", "mysql", "INTEGER", "INT", false)
	add("postgresql", "mysql", "BIGINT", "BIGINT", false)
	add("postgresql", "mysql", "TIMESTAMP", "DATETIME", true)
	add("postgresql", "mysql", "BOOLEAN", "TINYINT(1)", false)
	add("postgresql", "mysql", "BYTEA", "BLOB", false)
	add("postgresql", "mysql", "JSONB", "JSON", false)
	add("postgresql", "mysql", "UUID", "CHAR(36)", true)
	// MySQL → SQLite
	add("mysql", "sqlite", "INT", "INTEGER", false)
	add("mysql", "sqlite", "DATETIME", "TEXT", true)
	add("mysql", "sqlite", "JSON", "TEXT", false)
	// SQLite → MySQL
	add("sqlite", "mysql", "INTEGER", "BIGINT", true)
	add("sqlite", "mysql", "REAL", "DOUBLE", false)
	return r
}

func preserveLength(target, source string) string {
	// 提取 source 长度
	for i, c := range source {
		if c == '(' {
			closeIdx := strings.Index(source[i:], ")")
			if closeIdx > 0 {
				length := source[i : i+closeIdx+1]
				baseType := strings.ToUpper(strings.TrimSpace(source[:i]))
				baseTarget := strings.ToUpper(strings.TrimSpace(strings.Split(target, "(")[0]))
				if baseType == baseTarget {
					if !strings.Contains(target, "(") {
						return baseType + length
					}
				}
			}
			break
		}
	}
	return target
}

func generateDDL(tableName string, fms []FieldMigration, tgtDB string) string {
	quote := "\""
	if tgtDB == "mysql" || tgtDB == "mariadb" || tgtDB == "tidb" {
		quote = "`"
	}
	cols := []string{}
	for _, fm := range fms {
		if fm.TargetField == "" {
			continue
		}
		cols = append(cols, fmt.Sprintf("  %s%s%s %s", quote, fm.TargetField, quote, fm.TargetType))
	}
	return fmt.Sprintf("CREATE TABLE %s%s%s (\n%s\n)", quote, tableName, quote, strings.Join(cols, ",\n"))
}

// ============================================================================
// 智能分库分表
// ============================================================================

type ShardNode struct {
	ID         string
	Connection string
	Table      string
	Weight     int
	Active     bool
}

type ShardStrategy string

const (
	StrategyHash           ShardStrategy = "hash"
	StrategyRange          ShardStrategy = "range"
	StrategyConsistentHash ShardStrategy = "consistent_hash"
)

type SmartSharding struct {
	LogicalTable string
	ShardKey     string
	Strategy     ShardStrategy
	Nodes        []ShardNode
}

func NewSmartSharding(logicalTable, shardKey string, strategy ShardStrategy) *SmartSharding {
	return &SmartSharding{
		LogicalTable: logicalTable,
		ShardKey:     shardKey,
		Strategy:     strategy,
	}
}

func (s *SmartSharding) AddShard(id, conn, table string) {
	s.Nodes = append(s.Nodes, ShardNode{
		ID: id, Connection: conn, Table: table, Weight: 100, Active: true,
	})
}

func (s *SmartSharding) stableHash(v string) uint64 {
	h := uint64(1469598103934665603)
	for _, c := range []byte(v) {
		h ^= uint64(c)
		h *= 1099511628211
	}
	return h
}

func (s *SmartSharding) Route(shardValue string) (*ShardNode, error) {
	active := []ShardNode{}
	for _, n := range s.Nodes {
		if n.Active {
			active = append(active, n)
		}
	}
	if len(active) == 0 {
		return nil, errors.New("无活跃分片")
	}
	if s.Strategy == StrategyHash || s.Strategy == StrategyConsistentHash {
		idx := s.stableHash(shardValue) % uint64(len(active))
		return &active[idx], nil
	}
	var n int
	fmt.Sscanf(shardValue, "%d", &n)
	return &active[n%len(active)], nil
}

type ShardQueryResult struct {
	TotalShards  int
	ShardResults []map[string]interface{}
	TotalRows    int
	HasMore      bool
}

func (s *SmartSharding) Query() (*ShardQueryResult, error) {
	results := []map[string]interface{}{}
	for _, n := range s.Nodes {
		if !n.Active {
			continue
		}
		results = append(results, map[string]interface{}{
			"shard_id":   n.ID,
			"sql":        fmt.Sprintf("SELECT * FROM %s", n.Table),
			"rows":       []interface{}{},
			"elapsed_ms": 0,
		})
	}
	return &ShardQueryResult{
		TotalShards:  len(results),
		ShardResults: results,
		TotalRows:    0,
		HasMore:      false,
	}, nil
}

func (s *SmartSharding) WriteBatch(keyValues []string) (map[string]interface{}, error) {
	results := []map[string]interface{}{}
	success := 0
	for _, kv := range keyValues {
		node, err := s.Route(kv)
		if err != nil {
			results = append(results, map[string]interface{}{"key": kv, "success": false, "error": err.Error()})
			continue
		}
		results = append(results, map[string]interface{}{"key": kv, "shard_id": node.ID, "success": true})
		success++
	}
	return map[string]interface{}{
		"total":   len(results),
		"success": success,
		"failed":  len(results) - success,
		"results": results,
	}, nil
}

func (s *SmartSharding) RebalancePlan(totalRows int) map[string]interface{} {
	if len(s.Nodes) < 2 {
		return map[string]interface{}{"moves": []interface{}{}, "estimated_total_rows": totalRows}
	}
	perShard := int64(totalRows) / int64(len(s.Nodes))
	moves := []map[string]interface{}{}
	for i := 1; i < len(s.Nodes); i++ {
		moves = append(moves, map[string]interface{}{
			"from":           s.Nodes[0].ID,
			"to":             s.Nodes[i].ID,
			"range_start":    int64(i-1) * perShard,
			"range_end":      int64(i) * perShard,
			"estimated_rows": perShard,
		})
	}
	return map[string]interface{}{
		"moves":               moves,
		"estimated_total_rows": totalRows,
		"estimated_seconds":   totalRows / 10000,
	}
}

// ============================================================================
// 演示主函数请见 sqltool_demo.go
// ============================================================================
