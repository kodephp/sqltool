#!/usr/bin/env python3
"""
SQLTool Python 调用示例

安装依赖:
    pip install requests

使用方法:
    # 方式1: HTTP API (需要先启动 sqltool server)
    python examples/python/sqltool_demo.py

    # 方式2: CLI 直接调用
    python examples/python/sqltool_demo.py --cli
"""

import argparse
import json
import subprocess
import sys
from typing import Optional, Dict, Any, List

try:
    import requests
    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False


class SqlToolClient:
    """SQLTool 客户端 - 通过 HTTP API 调用"""

    def __init__(self, base_url: str = "http://localhost:8080"):
        self.base_url = base_url.rstrip('/')
        self.headers = {"Content-Type": "application/json"}

    def health_check(self) -> Dict[str, Any]:
        """健康检查"""
        resp = requests.get(f"{self.base_url}/api/health", timeout=5)
        resp.raise_for_status()
        return resp.json()

    def detect_injection(self, input_text: str) -> Dict[str, Any]:
        """SQL注入检测"""
        resp = requests.post(
            f"{self.base_url}/api/security/detect-injection",
            json={"input": input_text},
            headers=self.headers,
            timeout=5
        )
        resp.raise_for_status()
        return resp.json()

    def build_safe_sql(self, table: str, field: str, operator: str, value: str) -> Dict[str, Any]:
        """构建安全SQL"""
        resp = requests.post(
            f"{self.base_url}/api/security/build-safe-sql",
            json={"table": table, "field": field, "operator": operator, "value": value},
            headers=self.headers,
            timeout=5
        )
        resp.raise_for_status()
        return resp.json()

    def backup(self, source: str, db_type: str, backup_type: str = "full") -> Dict[str, Any]:
        """数据库备份"""
        resp = requests.post(
            f"{self.base_url}/api/backup",
            json={"source": source, "db_type": db_type, "backup_type": backup_type},
            headers=self.headers,
            timeout=30
        )
        resp.raise_for_status()
        return resp.json()

    def transfer(self, source: str, target: str, source_type: str, target_type: str) -> Dict[str, Any]:
        """数据迁移"""
        resp = requests.post(
            f"{self.base_url}/api/transfer",
            json={
                "source": source,
                "target": target,
                "source_type": source_type,
                "target_type": target_type,
            },
            headers=self.headers,
            timeout=60
        )
        resp.raise_for_status()
        return resp.json()


class SqlToolCLI:
    """SQLTool CLI 调用"""

    @staticmethod
    def run(*args: str) -> str:
        """执行 sqltool CLI 命令"""
        cmd = ["sqltool"] + list(args)
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            check=False
        )
        if result.returncode != 0:
            print(f"命令执行失败: {result.stderr}", file=sys.stderr)
        return result.stdout

    def detect_injection(self, input_text: str) -> str:
        """SQL注入检测"""
        return self.run("detect-injection", "--input", input_text)

    def build_safe_sql(self, table: str, field: str, operator: str, value: str) -> str:
        """构建安全SQL"""
        return self.run(
            "build-safe-sql",
            "--table", table,
            "--field", field,
            "--operator", operator,
            "--value", value
        )

    def backup(self, source: str, output: str, backup_type: str = "full") -> str:
        """数据库备份"""
        return self.run(
            "backup",
            "-s", source,
            "--output", output,
            "--backup-type", backup_type
        )

    def transfer(self, source: str, target: str, batch_size: int = 1000) -> str:
        """数据迁移"""
        return self.run(
            "transfer",
            "-s", source,
            "-t", target,
            "-B", str(batch_size)
        )


def print_result(title: str, result: Any):
    """打印结果"""
    print(f"\n{'='*50}")
    print(f"{title}")
    print('='*50)
    if isinstance(result, dict):
        print(json.dumps(result, indent=2, ensure_ascii=False))
    else:
        print(result)


def main():
    parser = argparse.ArgumentParser(description="SQLTool Python 示例")
    parser.add_argument("--cli", action="store_true", help="使用 CLI 模式 (不需要启动 server)")
    parser.add_argument("--url", default="http://localhost:8080", help="SQLTool Server URL")
    args = parser.parse_args()

    print("""
╔══════════════════════════════════════════════════╗
║         SQLTool Python 调用示例                   ║
╚══════════════════════════════════════════════════╝
    """)

    if args.cli:
        print("模式: CLI (不需要启动 sqltool server)")
        client = SqlToolCLI()

        # SQL注入检测
        print_result("1. SQL注入检测", client.detect_injection("' OR '1'='1"))
        print_result("2. 构建安全SQL", client.build_safe_sql("users", "name", "=", "test"))

    else:
        if not HAS_REQUESTS:
            print("错误: 需要安装 requests 库")
            print("  pip install requests")
            print("\n或使用 --cli 模式")
            sys.exit(1)

        print(f"模式: HTTP API (需要启动 sqltool server)")
        print(f"URL: {args.url}\n")

        client = SqlToolClient(args.url)

        try:
            # 健康检查
            print_result("0. 健康检查", client.health_check())

            # SQL注入检测
            print_result("1. SQL注入检测 - 恶意输入", client.detect_injection("' OR '1'='1"))
            print_result("2. SQL注入检测 - 正常输入", client.detect_injection("normal_input"))

            # 构建安全SQL
            print_result("3. 构建安全SQL", client.build_safe_sql("users", "name", "=", "test'; DROP TABLE"))

        except requests.exceptions.ConnectionError:
            print(f"\n错误: 无法连接到 {args.url}")
            print("请先启动 sqltool server:")
            print("  sqltool server -p 8080 -s mysql://localhost/mydb")
            sys.exit(1)
        except Exception as e:
            print(f"\n错误: {e}")
            sys.exit(1)

    print(f"\n{'='*50}")
    print("示例执行完成!")
    print("="*50)


if __name__ == "__main__":
    main()
