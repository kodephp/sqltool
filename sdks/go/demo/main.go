// 演示主程序
package main

import (
	"fmt"
	"strings"

	"sqlmap.local/sdks/go/sqltool"
)

func main() {
	fmt.Println(strings.Repeat("=", 70))
	fmt.Println("SQLTool Go SDK 演示")
	fmt.Println(strings.Repeat("=", 70))

	// 演示 1: 跨数据库迁移
	fmt.Println("\n[1] 跨数据库迁移 (MySQL 5.7 → PostgreSQL 16)")
	mig := sqltool.NewCrossDbMigrator()
	result, err := mig.MigrateTable(
		"mysql://root:pass@localhost:3306/mydb",
		"postgresql://postgres:pass@localhost:5432/mydb",
		sqltool.TableSpec{
			Name: "orders",
			Fields: []sqltool.FieldSpec{
				{Name: "id", DataType: "INT", PrimaryKey: true},
				{Name: "user_id", DataType: "BIGINT"},
				{Name: "amount", DataType: "DECIMAL(10,2)"},
				{Name: "created_at", DataType: "DATETIME"},
			},
		},
		"5.7.40", "16.2.0",
		nil,
	)
	if err != nil {
		fmt.Println("错误:", err)
		return
	}
	fmt.Printf("  方向: %s\n", result.Direction)
	fmt.Printf("  映射: %d/%d (%.1f%%)\n", result.FieldsMapped, result.FieldsTotal, result.SuccessRate()*100)
	fmt.Printf("  有损: %d\n", result.LossyConversions)
	fmt.Println("  DDL:")
	fmt.Println(result.DDL)

	// 演示 2: 智能分库分表
	fmt.Println("\n[2] 智能分库分表 (4 分片哈希)")
	sharding := sqltool.NewSmartSharding("orders", "user_id", sqltool.StrategyHash)
	sharding.AddShard("s0", "mysql://n1/orders_0", "orders_0")
	sharding.AddShard("s1", "mysql://n1/orders_1", "orders_1")
	sharding.AddShard("s2", "mysql://n2/orders_2", "orders_2")
	sharding.AddShard("s3", "mysql://n2/orders_3", "orders_3")

	fmt.Println("  路由演示:")
	for _, uid := range []string{"user_001", "user_042", "user_001"} {
		node, _ := sharding.Route(uid)
		fmt.Printf("    %s → %s (%s)\n", uid, node.ID, node.Table)
	}

	qResult, _ := sharding.Query()
	fmt.Printf("  跨分片查询: 涉及 %d 分片\n", qResult.TotalShards)

	wResult, _ := sharding.WriteBatch([]string{"u1", "u2", "u3"})
	fmt.Printf("  批量写入: %d/%d 成功\n", wResult["success"], wResult["total"])

	plan := sharding.RebalancePlan(10_000_000)
	moves := plan["moves"].([]map[string]interface{})
	fmt.Printf("  Rebalance: %d 步, ~%vs\n", len(moves), plan["estimated_seconds"])

	fmt.Println("\n✓ 演示完成")
}
