#!/bin/bash
# SQLTool CLI 调用示例集合
#
# 使用前提: 已安装 sqltool (cargo install sqltool)
#
# 用法: bash examples/cli/all_examples.sh

set -e

echo "=============================================="
echo "SQLTool CLI 功能示例"
echo "=============================================="
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查 sqltool 是否安装
check_sqltool() {
    if ! command -v sqltool &> /dev/null; then
        echo -e "${RED}错误: sqltool 未安装${NC}"
        echo "请先安装: cargo install sqltool"
        exit 1
    fi
    echo -e "${GREEN}✓ sqltool 已安装${NC}"
    sqltool --version
    echo ""
}

# 示例1: 健康检查
echo -e "${YELLOW}=== 示例1: 健康检查 ===${NC}"
sqltool server --help 2>/dev/null | head -5 || echo "server 命令可用"
echo ""

# 示例2: SQL注入检测
echo -e "${YELLOW}=== 示例2: SQL注入检测 ===${NC}"
echo "检测: ' OR '1'='1"
sqltool detect-sql-injection --input "' OR '1'='1"
echo ""

echo "检测: normal_input"
sqltool detect-sql-injection --input "normal_input"
echo ""

# 示例3: 构建安全SQL
echo -e "${YELLOW}=== 示例3: 构建安全SQL ===${NC}"
echo "用户输入: test'; DROP TABLE users; --"
sqltool build-safe-sql --table users --field name --operator = --value "test'; DROP TABLE users; --"
echo ""

# 示例4: 数据迁移 (需要真实数据库)
echo -e "${YELLOW}=== 示例4: 数据迁移 (需要真实数据库) ===${NC}"
echo "命令格式:"
echo "  sqltool transfer \\"
echo "    -s mysql://user:pass@localhost:3306/source_db \\"
echo "    -t postgresql://user:pass@localhost:5432/target_db \\"
echo "    -B 1000 \\"
echo "    -v"
echo ""

# 示例5: 数据库备份 (需要真实数据库)
echo -e "${YELLOW}=== 示例5: 数据库备份 (需要真实数据库) ===${NC}"
echo "命令格式:"
echo "  sqltool backup \\"
echo "    -s mysql://user:pass@localhost:3306/mydb \\"
echo "    --output ./backup.sql \\"
echo "    --backup-type full"
echo ""

# 示例6: 数据对比 (需要真实数据库)
echo -e "${YELLOW}=== 示例6: 数据对比 (需要真实数据库) ===${NC}"
echo "命令格式:"
echo "  sqltool compare-data \\"
echo "    -s mysql://user:pass@localhost:3306/db1 \\"
echo "    -t mysql://user:pass@localhost:3306/db2 \\"
echo "    --table users \\"
echo "    --primary-key id"
echo ""

# 示例7: 创建分片 (需要真实数据库)
echo -e "${YELLOW}=== 示例7: 创建分片 (需要真实数据库) ===${NC}"
echo "命令格式:"
echo "  sqltool create-shard \\"
echo "    -s mysql://user:pass@localhost:3306/mydb \\"
echo "    --table orders \\"
echo "    --strategy row_count \\"
echo "    --threshold 1000000"
echo ""

# 示例8: 慢查询检测 (需要真实数据库)
echo -e "${YELLOW}=== 示例8: 慢查询检测 (需要真实数据库) ===${NC}"
echo "命令格式:"
echo "  sqltool detect-slow \\"
echo "    -s mysql://user:pass@localhost:3306/mydb \\"
echo "    --threshold-ms 1000"
echo ""

# 示例9: 日志管理 (需要真实数据库)
echo -e "${YELLOW}=== 示例9: 插入日志 (需要真实数据库) ===${NC}"
echo "命令格式:"
echo "  sqltool insert-log \\"
echo "    -s mysql://user:pass@localhost:3306/mydb \\"
echo "    --table app_logs \\"
echo "    --level INFO \\"
echo "    --message '用户登录成功'"
echo ""

echo "=============================================="
echo -e "${GREEN}CLI 示例完成${NC}"
echo ""
echo "完整文档: https://docs.rs/sqltool"
echo "GitHub: https://github.com/kodephp/sqltool"
