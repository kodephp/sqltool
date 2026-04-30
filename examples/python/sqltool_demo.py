#!/usr/bin/env python3
"""
SQLTool Python 完整调用示例

功能覆盖：
    - 数据迁移 (transfer)
    - 数据备份 (backup)
    - 数据对比 (compare-data)
    - 分库分表 (create-shard)
    - 慢查询检测 (detect-slow-query)
    - 跨分片查询 (spanning-query)
    - 日志管理 (insert-log/query-logs)
    - SQL注入检测 (detect-sql-injection)
    - 安全SQL构建 (build-safe-sql)

安装依赖:
    pip install requests

使用方法:
    python sqltool_demo.py                    # HTTP API 模式
    python sqltool_demo.py --cli              # CLI 模式
    python sqltool_demo.py --cli --binary /path/to/sqltool  # 自定义二进制路径
"""

import argparse
import json
import subprocess
import sys
from typing import Optional, Dict, Any, List
from dataclasses import dataclass
from datetime import datetime

try:
    import requests
    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False


# =============================================================================
# HTTP API 客户端
# =============================================================================

class SqlToolClient:
    """SQLTool HTTP API 客户端"""

    def __init__(self, base_url: str = "http://localhost:8080", api_key: str = None):
        self.base_url = base_url.rstrip('/')
        self.headers = {"Content-Type": "application/json"}
        if api_key:
            self.headers["Authorization"] = f"Bearer {api_key}"

    def _post(self, path: str, data: Dict[str, Any]) -> Dict[str, Any]:
        """发送 POST 请求"""
        resp = requests.post(
            f"{self.base_url}{path}",
            json=data,
            headers=self.headers,
            timeout=60
        )
        resp.raise_for_status()
        return resp.json()

    def _get(self, path: str) -> Dict[str, Any]:
        """发送 GET 请求"""
        resp = requests.get(
            f"{self.base_url}{path}",
            headers=self.headers,
            timeout=30
        )
        resp.raise_for_status()
        return resp.json()

    # -------------------------------------------------------------------------
    # 健康检查
    # -------------------------------------------------------------------------

    def health_check(self) -> Dict[str, Any]:
        """健康检查"""
        return self._get("/api/health")

    # -------------------------------------------------------------------------
    # 数据迁移
    # -------------------------------------------------------------------------

    def transfer(
        self,
        source: str,
        target: str,
        source_type: str = "mysql",
        target_type: str = "postgresql",
        tables: str = "",
        batch_size: int = 1000,
        verify_data: bool = True,
        skip_errors: bool = True
    ) -> Dict[str, Any]:
        """
        数据迁移

        参数:
            source: 源数据库连接字符串
            target: 目标数据库连接字符串
            source_type: 源数据库类型 (mysql/postgresql/sqlite/oracle)
            target_type: 目标数据库类型
            tables: 表名列表，逗号分隔，空表示所有表
            batch_size: 批量大小
            verify_data: 是否验证数据
            skip_errors: 是否跳过错误
        """
        return self._post("/api/transfer", {
            "source": source,
            "target": target,
            "source_type": source_type,
            "target_type": target_type,
            "tables": tables,
            "batch_size": batch_size,
            "verify_data": verify_data,
            "skip_errors": skip_errors
        })

    # -------------------------------------------------------------------------
    # 数据库备份
    # -------------------------------------------------------------------------

    def backup(
        self,
        source: str,
        db_type: str = "mysql",
        output: str = "/tmp/backup.sql",
        backup_type: str = "full",
        compress: bool = True,
        include_procedures: bool = True,
        include_functions: bool = True,
        include_triggers: bool = True,
        parallel_tables: int = 4
    ) -> Dict[str, Any]:
        """
        数据库备份

        参数:
            source: 数据库连接字符串
            db_type: 数据库类型
            output: 备份文件路径
            backup_type: 备份类型 (full/incremental/differential)
            compress: 是否压缩
            include_procedures: 包含存储过程
            include_functions: 包含函数
            include_triggers: 包含触发器
            parallel_tables: 并行备份表数
        """
        return self._post("/api/backup", {
            "source": source,
            "db_type": db_type,
            "output": output,
            "backup_type": backup_type,
            "compress": compress,
            "include_procedures": include_procedures,
            "include_functions": include_functions,
            "include_triggers": include_triggers,
            "parallel_tables": parallel_tables
        })

    # -------------------------------------------------------------------------
    # 数据对比
    # -------------------------------------------------------------------------

    def compare_data(
        self,
        source: str,
        target: str,
        table: str,
        source_type: str = "mysql",
        target_type: str = "mysql",
        primary_key: str = "id",
        ignore_fields: str = "",
        compare_mode: str = "full"
    ) -> Dict[str, Any]:
        """
        数据对比

        参数:
            source: 源数据库
            target: 目标数据库
            table: 表名
            source_type: 源类型
            target_type: 目标类型
            primary_key: 主键字段
            ignore_fields: 忽略字段（逗号分隔）
            compare_mode: 对比模式 (full/sample)
        """
        return self._post("/api/compare", {
            "source": source,
            "target": target,
            "source_type": source_type,
            "target_type": target_type,
            "table": table,
            "primary_key": primary_key,
            "ignore_fields": ignore_fields,
            "compare_mode": compare_mode
        })

    # -------------------------------------------------------------------------
    # 分库分表
    # -------------------------------------------------------------------------

    def create_shard(
        self,
        source: str,
        table: str,
        strategy: str = "row_count",
        threshold: str = "1000000",
        prefix: str = "shard"
    ) -> Dict[str, Any]:
        """
        创建分片

        参数:
            source: 数据库连接
            table: 表名
            strategy: 分片策略 (row_count/time/size/hash)
            threshold: 阈值
            prefix: 分片前缀
        """
        return self._post("/api/shard/create", {
            "source": source,
            "table": table,
            "strategy": strategy,
            "threshold": threshold,
            "prefix": prefix
        })

    # -------------------------------------------------------------------------
    # 慢查询检测
    # -------------------------------------------------------------------------

    def detect_slow_query(
        self,
        source: str,
        db_type: str = "mysql",
        threshold_ms: int = 1000,
        limit: int = 10
    ) -> Dict[str, Any]:
        """
        慢查询检测

        参数:
            source: 数据库连接
            db_type: 数据库类型
            threshold_ms: 阈值（毫秒）
            limit: 返回数量
        """
        return self._post("/api/detect-slow", {
            "source": source,
            "db_type": db_type,
            "threshold_ms": threshold_ms,
            "limit": limit
        })

    # -------------------------------------------------------------------------
    # 跨分片查询
    # -------------------------------------------------------------------------

    def spanning_query(
        self,
        source: str,
        table: str,
        condition: str = "1=1",
        order_by: str = "",
        order_dir: str = "ASC",
        limit: int = 100,
        offset: int = 0
    ) -> Dict[str, Any]:
        """
        跨分片查询

        参数:
            source: 数据库连接
            table: 表名
            condition: WHERE条件
            order_by: 排序字段
            order_dir: 排序方向 (ASC/DESC)
            limit: 返回数量
            offset: 偏移量
        """
        return self._post("/api/spanning-query", {
            "source": source,
            "table": table,
            "condition": condition,
            "order_by": order_by,
            "order_dir": order_dir,
            "limit": limit,
            "offset": offset
        })

    # -------------------------------------------------------------------------
    # 日志管理 - 插入
    # -------------------------------------------------------------------------

    def insert_log(
        self,
        source: str,
        table: str = "app_logs",
        level: str = "INFO",
        message: str = "",
        source_name: str = ""
    ) -> Dict[str, Any]:
        """
        插入日志

        参数:
            source: 数据库连接
            table: 日志表名
            level: 日志级别 (DEBUG/INFO/WARN/ERROR)
            message: 日志消息
            source_name: 来源名称
        """
        return self._post("/api/log/insert", {
            "source": source,
            "table": table,
            "level": level,
            "message": message,
            "source_name": source_name
        })

    # -------------------------------------------------------------------------
    # 日志管理 - 查询
    # -------------------------------------------------------------------------

    def query_logs(
        self,
        source: str,
        table: str = "app_logs",
        levels: str = "",
        keyword: str = "",
        start_time: int = 0,
        end_time: int = 0,
        limit: int = 100
    ) -> List[Dict[str, Any]]:
        """
        查询日志

        参数:
            source: 数据库连接
            table: 日志表名
            levels: 级别过滤（逗号分隔）
            keyword: 关键字过滤
            start_time: 开始时间（Unix时间戳）
            end_time: 结束时间（Unix时间戳）
            limit: 返回数量
        """
        result = self._post("/api/log/query", {
            "source": source,
            "table": table,
            "levels": levels,
            "keyword": keyword,
            "start_time": start_time,
            "end_time": end_time,
            "limit": limit
        })
        return result.get("rows", [])

    # -------------------------------------------------------------------------
    # SQL注入检测
    # -------------------------------------------------------------------------

    def detect_injection(self, input_text: str) -> Dict[str, Any]:
        """
        SQL注入检测

        参数:
            input_text: 要检测的输入
        """
        return self._post("/api/security/detect-injection", {
            "input": input_text
        })

    # -------------------------------------------------------------------------
    # 安全SQL构建
    # -------------------------------------------------------------------------

    def build_safe_sql(
        self,
        table: str,
        field: str,
        operator: str = "=",
        value: str = ""
    ) -> Dict[str, Any]:
        """
        安全SQL构建

        参数:
            table: 表名
            field: 字段名
            operator: 操作符 (=, !=, <, >, LIKE, IN)
            value: 值
        """
        return self._post("/api/security/build-safe-sql", {
            "table": table,
            "field": field,
            "operator": operator,
            "value": value
        })


# =============================================================================
# CLI 客户端
# =============================================================================

class SqlToolCLI:
    """SQLTool CLI 调用客户端"""

    def __init__(self, binary_path: str = "sqltool"):
        self.binary_path = binary_path

    def run(self, *args: str, timeout: int = 60) -> str:
        """执行 sqltool CLI 命令"""
        cmd = [self.binary_path] + list(args)
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False
            )
            return result.stdout if result.returncode == 0 else f"错误: {result.stderr}"
        except subprocess.TimeoutExpired:
            return "错误: 命令执行超时"
        except FileNotFoundError:
            return f"错误: 找不到 sqltool 二进制文件: {self.binary_path}"

    # -------------------------------------------------------------------------
    # 数据迁移
    # -------------------------------------------------------------------------

    def transfer(
        self,
        source: str,
        target: str,
        source_type: str = "mysql",
        target_type: str = "postgresql",
        tables: str = "",
        batch_size: int = 1000
    ) -> str:
        """
        数据迁移

        示例:
            cli.transfer(
                "mysql://root:pass@localhost:3306/source",
                "postgresql://postgres:pass@localhost:5432/target",
                "mysql", "postgresql", "users,orders", 5000
            )
        """
        args = [
            "transfer",
            "-s", source,
            "-t", target,
            "-S", source_type,
            "-T", target_type,
            "-B", str(batch_size)
        ]
        if tables:
            args.extend(["--tables", tables])
        return self.run(*args)

    # -------------------------------------------------------------------------
    # 数据库备份
    # -------------------------------------------------------------------------

    def backup(
        self,
        source: str,
        output: str,
        db_type: str = "mysql",
        backup_type: str = "full",
        compress: bool = True
    ) -> str:
        """
        数据库备份

        示例:
            cli.backup(
                "mysql://root:pass@localhost:3306/mydb",
                "/tmp/backup.sql", "mysql", "full", True
            )
        """
        args = [
            "backup",
            "-s", source,
            "-T", db_type,
            "-o", output,
            "-t", backup_type
        ]
        if compress:
            args.append("-c")
        return self.run(*args)

    # -------------------------------------------------------------------------
    # 数据对比
    # -------------------------------------------------------------------------

    def compare_data(
        self,
        source: str,
        target: str,
        table: str,
        primary_key: str = "id",
        source_type: str = "mysql",
        target_type: str = "mysql"
    ) -> str:
        """
        数据对比

        示例:
            cli.compare_data(
                "mysql://root@localhost/db1",
                "mysql://root@localhost/db2",
                "users", "id", "mysql", "mysql"
            )
        """
        return self.run(
            "compare-data",
            "-s", source,
            "-t", target,
            "-S", source_type,
            "-T", target_type,
            "--table", table,
            "--primary-key", primary_key
        )

    # -------------------------------------------------------------------------
    # 创建分片
    # -------------------------------------------------------------------------

    def create_shard(
        self,
        source: str,
        table: str,
        strategy: str = "row_count",
        threshold: str = "1000000",
        prefix: str = "shard"
    ) -> str:
        """
        创建分片

        示例:
            cli.create_shard(
                "mysql://root:pass@localhost:3306/mydb",
                "orders", "row_count", "1000000", "orders_shard"
            )
        """
        return self.run(
            "create-shard",
            "-s", source,
            "--table", table,
            "--strategy", strategy,
            "--threshold", threshold,
            "--prefix", prefix
        )

    # -------------------------------------------------------------------------
    # 慢查询检测
    # -------------------------------------------------------------------------

    def detect_slow_query(
        self,
        source: str,
        db_type: str = "mysql",
        threshold_ms: int = 1000
    ) -> str:
        """
        慢查询检测

        示例:
            cli.detect_slow_query(
                "mysql://root:pass@localhost:3306/mydb",
                "mysql", 1000
            )
        """
        return self.run(
            "detect-slow-query",
            "-s", source,
            "-T", db_type,
            "--threshold-ms", str(threshold_ms)
        )

    # -------------------------------------------------------------------------
    # 跨分片查询
    # -------------------------------------------------------------------------

    def spanning_query(
        self,
        source: str,
        table: str,
        condition: str = "1=1",
        order_by: str = "",
        limit: int = 100,
        offset: int = 0
    ) -> str:
        """
        跨分片查询

        示例:
            cli.spanning_query(
                "mysql://root:pass@localhost:3306/mydb",
                "orders", "status='pending'", "created_at", 100, 0
            )
        """
        args = [
            "spanning-query",
            "-s", source,
            "--table", table,
            "--condition", condition,
            "-L", str(limit),
            "--offset", str(offset)
        ]
        if order_by:
            args.extend(["--order-by", order_by])
        return self.run(*args)

    # -------------------------------------------------------------------------
    # 插入日志
    # -------------------------------------------------------------------------

    def insert_log(
        self,
        source: str,
        message: str,
        table: str = "app_logs",
        level: str = "INFO",
        source_name: str = ""
    ) -> str:
        """
        插入日志

        示例:
            cli.insert_log(
                "mysql://root:pass@localhost:3306/mydb",
                "用户登录成功", "app_logs", "INFO", "auth-service"
            )
        """
        args = [
            "insert-log",
            "-s", source,
            "--table", table,
            "--level", level,
            "--message", message
        ]
        if source_name:
            args.extend(["--source-name", source_name])
        return self.run(*args)

    # -------------------------------------------------------------------------
    # 查询日志
    # -------------------------------------------------------------------------

    def query_logs(
        self,
        source: str,
        table: str = "app_logs",
        levels: str = "",
        keyword: str = "",
        limit: int = 100
    ) -> str:
        """
        查询日志

        示例:
            cli.query_logs(
                "mysql://root:pass@localhost:3306/mydb",
                "app_logs", "ERROR,WARN", "login", 50
            )
        """
        args = [
            "query-logs",
            "-s", source,
            "--table", table,
            "-L", str(limit)
        ]
        if levels:
            args.extend(["--levels", levels])
        if keyword:
            args.extend(["--keyword", keyword])
        return self.run(*args)

    # -------------------------------------------------------------------------
    # SQL注入检测
    # -------------------------------------------------------------------------

    def detect_injection(self, input_text: str) -> str:
        """
        SQL注入检测

        示例:
            cli.detect_injection("' OR '1'='1")
        """
        return self.run("detect-sql-injection", "-i", input_text)

    # -------------------------------------------------------------------------
    # 安全SQL构建
    # -------------------------------------------------------------------------

    def build_safe_sql(
        self,
        table: str,
        field: str,
        operator: str = "=",
        value: str = ""
    ) -> str:
        """
        安全SQL构建

        示例:
            cli.build_safe_sql("users", "name", "=", "John O'Brien")
        """
        return self.run(
            "build-safe-sql",
            "--table", table,
            "--field", field,
            "--operator", operator,
            "--value", value
        )


# =============================================================================
# 主函数
# =============================================================================

def print_result(title: str, result: Any):
    """打印结果"""
    print(f"\n{'='*60}")
    print(f"{title}")
    print('='*60)
    if isinstance(result, dict):
        print(json.dumps(result, indent=2, ensure_ascii=False))
    elif isinstance(result, list):
        print(json.dumps(result, indent=2, ensure_ascii=False))
    else:
        print(result)


def main():
    parser = argparse.ArgumentParser(description="SQLTool Python 完整示例 v0.4.1")
    parser.add_argument("--cli", action="store_true", help="使用 CLI 模式")
    parser.add_argument("--url", default="http://localhost:8080", help="API Server URL")
    parser.add_argument("--binary", default="sqltool", help="sqltool 二进制路径")
    args = parser.parse_args()

    print("""
╔════════════════════════════════════════════════════════════╗
║         SQLTool Python 完整调用示例 v0.4.1                ║
╚════════════════════════════════════════════════════════════╝
    """)

    if args.cli:
        print("模式: CLI")
        print(f"二进制: {args.binary}\n")
        cli = SqlToolCLI(args.binary)

        # 1. SQL注入检测
        print("1. SQL注入检测...")
        result = cli.detect_injection("' OR '1'='1")
        print_result("检测结果", result)

        # 2. 安全SQL构建
        print("2. 安全SQL构建...")
        result = cli.build_safe_sql("users", "name", "=", "test'; DROP TABLE")
        print_result("构建结果", result)

        # 3. 数据迁移
        print("3. 数据迁移...")
        result = cli.transfer(
            "mysql://root:pass@localhost:3306/source",
            "postgresql://postgres:pass@localhost:5432/target",
            "mysql", "postgresql", "users,orders", 5000
        )
        print_result("迁移结果", result)

        # 4. 数据库备份
        print("4. 数据库备份...")
        result = cli.backup(
            "mysql://root:pass@localhost:3306/mydb",
            "/tmp/backup.sql", "mysql", "full", True
        )
        print_result("备份结果", result)

        # 5. 数据对比
        print("5. 数据对比...")
        result = cli.compare_data(
            "mysql://root@localhost/db1",
            "mysql://root@localhost/db2",
            "users", "id"
        )
        print_result("对比结果", result)

    else:
        if not HAS_REQUESTS:
            print("错误: 需要安装 requests 库")
            print("  pip install requests")
            print("\n或使用 --cli 模式")
            sys.exit(1)

        print(f"模式: HTTP API")
        print(f"URL: {args.url}\n")

        client = SqlToolClient(args.url)

        try:
            # 健康检查
            print("0. 健康检查...")
            print_result("健康状态", client.health_check())

            # 1. SQL注入检测
            print("1. SQL注入检测...")
            result = client.detect_injection("' OR '1'='1")
            print_result("检测结果", result)
            if result.get("risk_level") in ["High", "Critical"]:
                print("⚠️ 警告: 检测到高风险SQL注入攻击!")

            # 2. 安全SQL构建
            print("2. 安全SQL构建...")
            result = client.build_safe_sql("users", "email", "LIKE", "%@example.com")
            print_result("构建结果", result)

            # 3. 数据迁移示例
            print("3. 数据迁移...")
            print("(需要真实数据库连接才能执行)")
            result = client.transfer(
                source="mysql://root:password@localhost:3306/source_db",
                target="postgresql://postgres:password@localhost:5432/target_db",
                source_type="mysql",
                target_type="postgresql",
                tables="users,orders,products",
                batch_size=5000,
                verify_data=True
            )
            print_result("迁移结果", result)

            # 4. 数据库备份示例
            print("4. 数据库备份...")
            print("(需要真实数据库连接才能执行)")
            result = client.backup(
                source="mysql://root:password@localhost:3306/mydb",
                db_type="mysql",
                output="/tmp/backup_20240101.sql",
                backup_type="full",
                compress=True
            )
            print_result("备份结果", result)

            # 5. 数据对比示例
            print("5. 数据对比...")
            print("(需要真实数据库连接才能执行)")
            result = client.compare_data(
                source="mysql://root:password@localhost:3306/db1",
                target="mysql://root:password@localhost:3306/db2",
                table="users",
                primary_key="id",
                ignore_fields="updated_at"
            )
            print_result("对比结果", result)

            # 6. 分库分表示例
            print("6. 分库分表...")
            print("(需要真实数据库连接才能执行)")
            result = client.create_shard(
                source="mysql://root:password@localhost:3306/mydb",
                table="orders",
                strategy="row_count",
                threshold="1000000",
                prefix="orders_shard"
            )
            print_result("分片结果", result)

            # 7. 慢查询检测示例
            print("7. 慢查询检测...")
            print("(需要真实数据库连接才能执行)")
            result = client.detect_slow_query(
                source="mysql://root:password@localhost:3306/mydb",
                threshold_ms=1000,
                limit=10
            )
            print_result("检测结果", result)

            # 8. 跨分片查询示例
            print("8. 跨分片查询...")
            print("(需要真实数据库连接才能执行)")
            result = client.spanning_query(
                source="mysql://root:password@localhost:3306/mydb",
                table="orders",
                condition="status='pending'",
                order_by="created_at",
                order_dir="DESC",
                limit=100
            )
            print_result("查询结果", result)

            # 9. 插入日志示例
            print("9. 插入日志...")
            print("(需要真实数据库连接才能执行)")
            result = client.insert_log(
                source="mysql://root:password@localhost:3306/mydb",
                table="app_logs",
                level="INFO",
                message="用户登录成功",
                source_name="auth-service"
            )
            print_result("插入结果", result)

            # 10. 查询日志示例
            print("10. 查询日志...")
            print("(需要真实数据库连接才能执行)")
            result = client.query_logs(
                source="mysql://root:password@localhost:3306/mydb",
                table="app_logs",
                levels="ERROR,WARN",
                keyword="login",
                limit=50
            )
            print_result("查询结果", result)

        except requests.exceptions.ConnectionError:
            print(f"\n错误: 无法连接到 {args.url}")
            print("请先启动 sqltool server:")
            print("  sqltool server -p 8080 -s mysql://localhost/mydb")
            sys.exit(1)
        except Exception as e:
            print(f"\n错误: {e}")
            import traceback
            traceback.print_exc()
            sys.exit(1)

    print(f"\n{'='*60}")
    print("示例执行完成!")
    print("="*60)


if __name__ == "__main__":
    main()
