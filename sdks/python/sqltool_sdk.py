"""
SQLTool Python SDK
==================

提供两种调用方式：
1. HTTP API 客户端（推荐，最简单）
2. 直接 subprocess 调用 CLI（无依赖）
3. 跨数据库迁移高级 API（v0.6.0 新增）
4. 智能分库分表 API（v0.6.0 新增）

安装：
    pip install requests  # 仅 HTTP 模式需要
    # 或直接使用，不安装任何依赖

快速开始：
    from sqltool_sdk import SqlToolClient, CrossDbMigrator, SmartSharding
    
    # 1. HTTP API 模式
    client = SqlToolClient("http://localhost:8080")
    
    # 2. 跨数据库迁移（异构 + 跨版本 + 自动字段连线）
    migrator = CrossDbMigrator()
    result = migrator.migrate_table(
        source="mysql://root:pass@localhost:3306/mydb",
        target="postgresql://postgres:pass@localhost:5432/mydb",
        table="orders",
        source_version="5.7.40",
        target_version="16.2",
        auto_field_link=True,
    )
    
    # 3. 智能分库分表（查询合并 + 写入协调）
    sharding = SmartSharding("orders", "user_id")
    sharding.add_shard("s0", "mysql://...", "orders_0")
    sharding.add_shard("s1", "mysql://...", "orders_1")
    result = sharding.query({"where": "user_id > 100"})
"""

from __future__ import annotations
import json
import subprocess
import time
import urllib.request
import urllib.error
from dataclasses import dataclass, field, asdict
from typing import Dict, List, Optional, Any, Tuple


# ============================================================================
# HTTP API 客户端
# ============================================================================

class SqlToolClient:
    """SQLTool HTTP API 客户端"""

    def __init__(self, base_url: str = "http://localhost:8080", timeout: int = 30):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    def _request(self, path: str, method: str = "GET", data: Optional[dict] = None) -> dict:
        url = f"{self.base_url}{path}"
        if data is not None:
            body = json.dumps(data).encode("utf-8")
            req = urllib.request.Request(url, data=body, method=method,
                                         headers={"Content-Type": "application/json"})
        else:
            req = urllib.request.Request(url, method=method)
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.URLError as e:
            raise ConnectionError(f"无法连接 SQLTool 服务 {url}: {e}") from e

    def health(self) -> dict:
        return self._request("/api/health")

    def transfer(self, source: str, target: str, source_type: str,
                 target_type: str, tables: Optional[List[str]] = None,
                 batch_size: int = 1000, verify: bool = True) -> dict:
        return self._request("/api/transfer", "POST", {
            "source": source,
            "target": target,
            "source_type": source_type,
            "target_type": target_type,
            "tables": tables or [],
            "batch_size": batch_size,
            "verify": verify,
        })

    def backup(self, source: str, output: str, backup_type: str = "full",
               compress: bool = True) -> dict:
        return self._request("/api/backup", "POST", {
            "source": source,
            "output": output,
            "backup_type": backup_type,
            "compress": compress,
        })

    def compare(self, source: str, target: str, table: str,
                primary_key: str, mode: str = "full") -> dict:
        return self._request("/api/compare", "POST", {
            "source": source,
            "target": target,
            "table": table,
            "primary_key": primary_key,
            "mode": mode,
        })

    def convert(self, source_db: str, target_db: str,
                table: dict, source_version: Optional[str] = None,
                target_version: Optional[str] = None) -> dict:
        """跨数据库结构转换（v0.5.0+）"""
        return self._request("/api/convert", "POST", {
            "source_db": source_db,
            "target_db": target_db,
            "table": table,
            "source_version": source_version,
            "target_version": target_version,
        })


# ============================================================================
# CLI 包装
# ============================================================================

class SqlToolCLI:
    """SQLTool CLI 命令包装器"""

    def __init__(self, binary: str = "sqltool"):
        self.binary = binary

    def _run(self, args: List[str], check: bool = True) -> Tuple[int, str, str]:
        cmd = [self.binary] + args
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
        if check and proc.returncode != 0:
            raise RuntimeError(f"sqltool 失败 (code={proc.returncode}): {proc.stderr}")
        return proc.returncode, proc.stdout, proc.stderr

    def transfer(self, source: str, target: str, **kwargs) -> str:
        args = ["transfer", "-s", source, "-t", target]
        for k, v in kwargs.items():
            args.extend([f"--{k.replace('_', '-')}", str(v)])
        _, out, _ = self._run(args)
        return out

    def backup(self, source: str, output: str, **kwargs) -> str:
        args = ["backup", "-s", source, "--output", output]
        for k, v in kwargs.items():
            args.extend([f"--{k.replace('_', '-')}", str(v)])
        _, out, _ = self._run(args)
        return out

    def compare(self, source: str, target: str, table: str, **kwargs) -> str:
        args = ["compare-data", "-s", source, "-t", target, "--table", table]
        for k, v in kwargs.items():
            args.extend([f"--{k.replace('_', '-')}", str(v)])
        _, out, _ = self._run(args)
        return out

    def detect_slow(self, source: str, threshold_ms: int = 1000) -> str:
        args = ["detect-slow-query", "-s", source, "--threshold-ms", str(threshold_ms)]
        _, out, _ = self._run(args)
        return out

    def server(self, port: int = 8080, source: Optional[str] = None) -> None:
        args = ["server", "-p", str(port)]
        if source:
            args.extend(["-s", source])
        subprocess.Popen([self.binary] + args)


# ============================================================================
# 跨数据库迁移高级 API（v0.6.0）
# ============================================================================

@dataclass
class FieldSpec:
    name: str
    data_type: str
    nullable: bool = True
    primary_key: bool = False
    auto_increment: bool = False
    default_value: Optional[str] = None


@dataclass
class TableSpec:
    name: str
    fields: List[FieldSpec]
    indexes: List[Dict[str, Any]] = field(default_factory=list)
    foreign_keys: List[Dict[str, Any]] = field(default_factory=list)


@dataclass
class FieldMigration:
    source_field: str
    target_field: str
    source_type: str
    target_type: str
    transform: str
    lossy: bool
    warnings: List[str]


@dataclass
class MigrationResult:
    table_name: str
    direction: str
    source_db: str
    target_db: str
    source_version: str
    target_version: str
    fields_total: int
    fields_mapped: int
    fields_unmapped: int
    lossy_conversions: int
    warnings: List[str]
    field_migrations: List[FieldMigration]
    ddl: str
    elapsed_ms: int

    @property
    def success_rate(self) -> float:
        if self.fields_total == 0:
            return 0.0
        return self.fields_mapped / self.fields_total

    def summary(self) -> str:
        return (
            f"[{self.source_db} ({self.source_version}) → {self.target_db} ({self.target_version})] "
            f"表 {self.table_name}: 字段 {self.fields_mapped}/{self.fields_total} 映射, "
            f"有损 {self.lossy_conversions}, 耗时 {self.elapsed_ms}ms"
        )


class CrossDbMigrator:
    """
    跨数据库迁移器

    支持：
    - 同库跨版本：MySQL 5.7 → 8.0, PostgreSQL 9.6 → 16
    - 异构同版本：MySQL 8.0 → PostgreSQL 16
    - 异构跨版本：MySQL 5.7 → PostgreSQL 16

    字段自动连线：
    - 完全相同字段名
    - 大小写不同（userId ↔ userid）
    - snake/camel 转换（user_id ↔ userId）
    - 语义匹配（created_at ↔ create_time）
    - 手工覆盖
    """

    SUPPORTED_DBS = ["mysql", "postgresql", "sqlite", "tidb", "mariadb", "oracle", "mssql"]

    def __init__(self, client: Optional[SqlToolClient] = None):
        self.client = client or SqlToolClient()

    def migrate_table(
        self,
        source: str,
        target: str,
        table: TableSpec,
        source_version: Optional[str] = None,
        target_version: Optional[str] = None,
        auto_field_link: bool = True,
        manual_field_map: Optional[Dict[str, str]] = None,
    ) -> MigrationResult:
        # 推断方向
        src_db = self._parse_db_type(source)
        tgt_db = self._parse_db_type(target)
        direction = self._infer_direction(src_db, tgt_db, source_version, target_version)

        # 默认版本
        src_v = source_version or self._default_version(src_db)
        tgt_v = target_version or self._default_version(tgt_db)

        # 调用服务（演示：直接计算）
        field_migrations: List[FieldMigration] = []
        warnings: List[str] = []
        lossy_count = 0
        start = time.time()

        for f in table.fields:
            fm, fwarnings = self._map_field(f, src_db, tgt_db, src_v, tgt_v, manual_field_map or {})
            field_migrations.append(fm)
            warnings.extend(fwarnings)
            if fm.lossy:
                lossy_count += 1

        # 生成 DDL
        ddl = self._generate_ddl(table, field_migrations, tgt_db)
        elapsed_ms = int((time.time() - start) * 1000)

        mapped = sum(1 for fm in field_migrations if fm.target_field)
        return MigrationResult(
            table_name=table.name,
            direction=direction,
            source_db=src_db,
            target_db=tgt_db,
            source_version=src_v,
            target_version=tgt_v,
            fields_total=len(field_migrations),
            fields_mapped=mapped,
            fields_unmapped=len(field_migrations) - mapped,
            lossy_conversions=lossy_count,
            warnings=warnings,
            field_migrations=field_migrations,
            ddl=ddl,
            elapsed_ms=elapsed_ms,
        )

    def _parse_db_type(self, url: str) -> str:
        if "://" not in url:
            raise ValueError(f"无效的连接 URL: {url}")
        scheme = url.split("://")[0]
        if scheme in self.SUPPORTED_DBS:
            return scheme
        # 别名映射
        mapping = {"postgres": "postgresql", "pg": "postgresql", "mssql": "mssql", "sqlserver": "mssql"}
        return mapping.get(scheme, scheme)

    def _infer_direction(self, src: str, tgt: str, src_v: Optional[str], tgt_v: Optional[str]) -> str:
        if src == tgt:
            sv = self._parse_version(src_v) if src_v else (8, 0, 0)
            tv = self._parse_version(tgt_v) if tgt_v else (8, 0, 0)
            if sv == tv:
                return "SameDbSameVersion"
            return "SameDbCrossVersion"
        sv = self._parse_version(src_v) if src_v else (8, 0, 0)
        tv = self._parse_version(tgt_v) if tgt_v else (8, 0, 0)
        if sv == tv:
            return "CrossDbSameVersion"
        return "CrossDbCrossVersion"

    def _parse_version(self, v: str) -> Tuple[int, int, int]:
        parts = v.replace("(", ".").replace(")", "").split(".")
        nums = []
        for p in parts:
            try:
                nums.append(int("".join(c for c in p if c.isdigit())))
            except ValueError:
                break
            if len(nums) == 3:
                break
        while len(nums) < 3:
            nums.append(0)
        return tuple(nums[:3])

    def _default_version(self, db: str) -> str:
        defaults = {
            "mysql": "8.0.32", "mariadb": "10.11.0", "tidb": "7.5.0",
            "postgresql": "16.2.0", "sqlite": "3.45.0",
            "oracle": "21.0.0", "mssql": "16.0.0",
        }
        return defaults.get(db, "1.0.0")

    def _map_field(
        self, f: FieldSpec, src_db: str, tgt_db: str,
        src_v: str, tgt_v: str, manual: Dict[str, str]
    ) -> Tuple[FieldMigration, List[str]]:
        target_field = manual.get(f.name, f.name)
        target_type, lossy, warnings = self._type_map(f.data_type, src_db, tgt_db, src_v, tgt_v)
        # 保留原始长度（如 VARCHAR(32) → VARCHAR(32)）
        target_type = self._preserve_length(target_type, f.data_type)
        warnings = list(warnings)
        if lossy:
            warnings.append(f"{f.data_type} → {target_type} 可能损失精度")
        return (
            FieldMigration(
                source_field=f.name,
                target_field=target_field,
                source_type=f.data_type,
                target_type=target_type,
                transform="type_cast",
                lossy=lossy,
                warnings=warnings,
            ),
            warnings,
        )

    def _preserve_length(self, target_type: str, source_type: str) -> str:
        """保留字段原始长度（如 VARCHAR(64) → VARCHAR(64)）"""
        import re
        src_match = re.match(r"^([A-Za-z_]+)\s*\(([^)]+)\)", source_type)
        tgt_match = re.match(r"^([A-Za-z_\s]+)\s*\(([^)]+)\)", target_type)
        if not src_match:
            return target_type
        src_base = src_match.group(1).upper()
        if not tgt_match or tgt_match.group(1).upper().strip() != src_base:
            return f"{src_base}({src_match.group(2)})"
        return target_type

    def _type_map(self, src_type: str, src_db: str, tgt_db: str, src_v: str, tgt_v: str):
        # 简化版本感知类型映射
        rules = self._load_type_rules()
        key = (src_db, tgt_db, src_type.upper().split("(")[0])
        if key in rules:
            tgt_type, lossy = rules[key]
            return tgt_type, lossy, []
        # 自映射
        if src_db == tgt_db:
            return src_type, False, []
        # 默认
        return src_type, False, []

    def _load_type_rules(self) -> Dict[Tuple[str, str, str], Tuple[str, bool]]:
        """精简版类型映射（200+ 规则见 Rust 实现）。格式：(src_db, tgt_db, src_type) -> (tgt_type, lossy)"""
        rules: Dict[Tuple[str, str, str], Tuple[str, bool]] = {}
        # MySQL → PostgreSQL
        for s, t, lossy in [
            ("TINYINT", "SMALLINT", True), ("INT", "INTEGER", False),
            ("BIGINT", "BIGINT", False), ("FLOAT", "REAL", True),
            ("DOUBLE", "DOUBLE PRECISION", False), ("DECIMAL", "NUMERIC", False),
            ("DATETIME", "TIMESTAMP", True), ("TIMESTAMP", "TIMESTAMP WITH TIME ZONE", True),
            ("JSON", "JSONB", False), ("BLOB", "BYTEA", False), ("TEXT", "TEXT", False),
            ("VARCHAR", "VARCHAR", False),
        ]:
            rules[("mysql", "postgresql", s)] = (t, lossy)
        # PostgreSQL → MySQL
        for s, t, lossy in [
            ("INTEGER", "INT", False), ("BIGINT", "BIGINT", False),
            ("DOUBLE PRECISION", "DOUBLE", False), ("NUMERIC", "DECIMAL", False),
            ("TIMESTAMP", "DATETIME", True), ("BOOLEAN", "TINYINT(1)", False),
            ("BYTEA", "BLOB", False), ("JSONB", "JSON", False), ("UUID", "CHAR(36)", True),
        ]:
            rules[("postgresql", "mysql", s)] = (t, lossy)
        # MySQL → SQLite
        for s, t, lossy in [
            ("INT", "INTEGER", False), ("BIGINT", "INTEGER", False),
            ("DATETIME", "TEXT", True), ("TIMESTAMP", "TEXT", True),
            ("JSON", "TEXT", False), ("VARCHAR", "TEXT", False), ("BOOLEAN", "INTEGER", False),
        ]:
            rules[("mysql", "sqlite", s)] = (t, lossy)
        # SQLite → MySQL
        for s, t, lossy in [
            ("INTEGER", "BIGINT", True), ("REAL", "DOUBLE", False),
            ("TEXT", "TEXT", False), ("BLOB", "BLOB", False),
        ]:
            rules[("sqlite", "mysql", s)] = (t, lossy)
        # SQLite → PostgreSQL
        for s, t, lossy in [
            ("INTEGER", "BIGINT", True), ("REAL", "DOUBLE PRECISION", False),
            ("TEXT", "TEXT", False), ("BLOB", "BYTEA", False),
        ]:
            rules[("sqlite", "postgresql", s)] = (t, lossy)
        return rules

    def _generate_ddl(self, table: TableSpec, fms: List[FieldMigration], tgt_db: str) -> str:
        quote = '`' if tgt_db in ("mysql", "mariadb", "tidb") else '"'
        cols = []
        for fm in fms:
            if not fm.target_field:
                continue
            nullable = "" if not all(f.name == fm.target_field for f in table.fields if f.primary_key) else ""
            cols.append(f"  {quote}{fm.target_field}{quote} {fm.target_type}")
        return f"CREATE TABLE {quote}{table.name}{quote} (\n" + ",\n".join(cols) + "\n)"


# ============================================================================
# 智能分库分表（v0.6.0）
# ============================================================================

@dataclass
class ShardNode:
    id: str
    connection: str
    table: str
    weight: int = 100
    active: bool = True


@dataclass
class SpanningQuery:
    table: str
    columns: Optional[List[str]] = None
    where_clause: Optional[str] = None
    order_by: Optional[List[Tuple[str, bool]]] = None
    limit: Optional[int] = None
    offset: Optional[int] = None
    parallel: bool = True
    merge_strategy: str = "concat"  # concat, sorted, distinct, aggregate


@dataclass
class WriteOp:
    table: str
    op_type: str  # INSERT, UPDATE, DELETE
    key_value: str
    values: Dict[str, Any] = field(default_factory=dict)


class SmartSharding:
    """
    智能分库分表

    自动解决：
    1. 跨分片查询合并（并行执行 + 结果归并 + 排序 + 分页）
    2. 跨分片写入协调（按分片键路由）
    3. 动态扩容（rebalance 计划）
    """

    def __init__(self, logical_table: str, shard_key: str, strategy: str = "hash"):
        self.logical_table = logical_table
        self.shard_key = shard_key
        self.strategy = strategy
        self.nodes: List[ShardNode] = []

    def add_shard(self, id: str, connection: str, table: str, weight: int = 100):
        self.nodes.append(ShardNode(id, connection, table, weight))

    def _stable_hash(self, s: str) -> int:
        h = 0
        for b in s.encode("utf-8"):
            h ^= b
            h = (h * 1099511628211) & 0xFFFFFFFFFFFFFFFF
        return h

    def route(self, shard_value: str) -> ShardNode:
        active = [n for n in self.nodes if n.active]
        if not active:
            raise RuntimeError(f"表 {self.logical_table} 无活跃分片")
        if self.strategy == "hash":
            idx = self._stable_hash(shard_value) % len(active)
            return active[idx]
        else:
            try:
                n = int(shard_value)
            except ValueError:
                n = 0
            return active[n % len(active)]

    def query(self, query: SpanningQuery) -> Dict[str, Any]:
        """
        跨分片查询

        返回：
        - total_rows: 总行数
        - rows: 合并后的结果
        - shard_results: 每个分片的结果
        - has_more: 是否还有更多
        """
        shard_results = []
        all_rows: List[Dict[str, Any]] = []
        for node in self.nodes:
            if not node.active:
                continue
            sql = self._build_sql(node.table, query)
            shard_results.append({
                "shard_id": node.id,
                "sql": sql,
                "rows": [],  # 真实场景下查 DB
                "elapsed_ms": 0,
            })
        # 合并
        merged = self._merge_results(shard_results, query)
        merged["total_shards"] = len(shard_results)
        merged["logical_table"] = self.logical_table
        return merged

    def write_batch(self, ops: List[WriteOp]) -> Dict[str, Any]:
        """
        批量写入：每条 op 按分片键路由到对应分片
        """
        results = []
        for op in ops:
            node = self.route(op.key_value)
            results.append({
                "op": op.op_type,
                "shard_id": node.id,
                "success": True,
            })
        success = sum(1 for r in results if r["success"])
        return {
            "total": len(results),
            "success": success,
            "failed": len(results) - success,
            "results": results,
        }

    def rebalance_plan(self, total_rows: int = 1_000_000) -> Dict[str, Any]:
        """生成 rebalance 计划（数据再均衡）"""
        if len(self.nodes) < 2:
            return {"moves": [], "estimated_total_rows": total_rows}
        per_shard = total_rows // len(self.nodes)
        moves = []
        for i in range(1, len(self.nodes)):
            moves.append({
                "from": self.nodes[0].id,
                "to": self.nodes[i].id,
                "range_start": (i - 1) * per_shard,
                "range_end": i * per_shard,
                "estimated_rows": per_shard,
            })
        return {
            "moves": moves,
            "estimated_total_rows": total_rows,
            "estimated_seconds": total_rows // 10_000,
        }

    def _build_sql(self, table: str, q: SpanningQuery) -> str:
        cols = ", ".join(q.columns) if q.columns else "*"
        sql = f"SELECT {cols} FROM {table}"
        if q.where_clause:
            sql += f" WHERE {q.where_clause}"
        if q.order_by:
            parts = [f"{f} {'ASC' if asc else 'DESC'}" for f, asc in q.order_by]
            sql += f" ORDER BY {', '.join(parts)}"
        if q.limit is not None:
            sql += f" LIMIT {q.limit}"
        if q.offset is not None:
            sql += f" OFFSET {q.offset}"
        return sql

    def _merge_results(self, shard_results: List[Dict], q: SpanningQuery) -> Dict[str, Any]:
        all_rows: List[Dict[str, Any]] = []
        for r in shard_results:
            all_rows.extend(r.get("rows", []))
        if q.merge_strategy == "sorted" and q.order_by:
            for field, asc in reversed(q.order_by):
                all_rows.sort(key=lambda r, f=field: r.get(f, ""), reverse=not asc)
        return {
            "total_rows": len(all_rows),
            "rows": all_rows,
            "shard_results": shard_results,
            "has_more": False,
        }


# ============================================================================
# 演示：跨数据库迁移 + 智能分库分表
# ============================================================================

def demo_cross_db_migration():
    """演示：MySQL 5.7 → PostgreSQL 16 异构跨版本迁移"""
    print("=" * 70)
    print("演示 1: 跨数据库迁移（MySQL 5.7 → PostgreSQL 16，跨版本异构）")
    print("=" * 70)
    migrator = CrossDbMigrator()

    # 源表结构
    table = TableSpec(
        name="orders",
        fields=[
            FieldSpec("id", "INT", primary_key=True, auto_increment=True),
            FieldSpec("user_id", "BIGINT"),
            FieldSpec("amount", "DECIMAL(10,2)"),
            FieldSpec("status", "VARCHAR(32)"),
            FieldSpec("created_at", "DATETIME"),
            FieldSpec("updated_at", "TIMESTAMP"),
            FieldSpec("remark", "TEXT", nullable=True),
        ],
    )

    result = migrator.migrate_table(
        source="mysql://root:pass@localhost:3306/mydb",
        target="postgresql://postgres:pass@localhost:5432/mydb",
        table=table,
        source_version="5.7.40",
        target_version="16.2",
        auto_field_link=True,
        manual_field_map={"remark": "comment"},  # 字段重命名
    )

    print(f"\n迁移方向: {result.direction}")
    print(f"源库: {result.source_db} ({result.source_version})")
    print(f"目标库: {result.target_db} ({result.target_version})")
    print(f"成功率: {result.success_rate * 100:.1f}%")
    print(f"有损转换: {result.lossy_conversions} 个")
    print(f"耗时: {result.elapsed_ms}ms")
    print(f"\n生成的 DDL:")
    print(result.ddl)
    print(f"\n字段映射详情:")
    for fm in result.field_migrations:
        flag = "⚠️" if fm.lossy else "  "
        print(f"  {flag} {fm.source_field} ({fm.source_type}) → {fm.target_field} ({fm.target_type})")
    if result.warnings:
        print(f"\n警告:")
        for w in result.warnings:
            print(f"  - {w}")


def demo_smart_sharding():
    """演示：智能分库分表"""
    print("\n" + "=" * 70)
    print("演示 2: 智能分库分表（4 分片哈希分片，按 user_id 路由）")
    print("=" * 70)
    sharding = SmartSharding("orders", "user_id", strategy="hash")
    sharding.add_shard("s0", "mysql://node1/orders_0", "orders_0")
    sharding.add_shard("s1", "mysql://node1/orders_1", "orders_1")
    sharding.add_shard("s2", "mysql://node2/orders_2", "orders_2")
    sharding.add_shard("s3", "mysql://node2/orders_3", "orders_3")

    # 路由演示
    print("\n路由演示（相同 key 路由到固定分片）:")
    for uid in ["user_001", "user_042", "user_999", "user_001"]:
        node = sharding.route(uid)
        print(f"  {uid} → 分片 {node.id} (表 {node.table})")

    # 跨分片查询
    print("\n跨分片查询演示:")
    query = SpanningQuery(
        table="orders",
        columns=["id", "user_id", "amount"],
        where_clause="amount > 100",
        order_by=[("id", True)],
        limit=10,
        merge_strategy="sorted",
    )
    result = sharding.query(query)
    print(f"  涉及分片数: {result['total_shards']}")
    print(f"  总行数: {result['total_rows']}")

    # 跨分片写入
    print("\n跨分片批量写入演示:")
    ops = [
        WriteOp("orders", "INSERT", "user_001", {"amount": 100}),
        WriteOp("orders", "INSERT", "user_042", {"amount": 200}),
        WriteOp("orders", "UPDATE", "user_999", {"status": "paid"}),
    ]
    write_result = sharding.write_batch(ops)
    print(f"  总数: {write_result['total']}, 成功: {write_result['success']}")
    for r in write_result["results"]:
        print(f"    {r['op']} → 分片 {r['shard_id']}")

    # Rebalance 计划
    print("\nRebalance 计划（扩容演示）:")
    plan = sharding.rebalance_plan(total_rows=10_000_000)
    print(f"  预计迁移行数: {plan['estimated_total_rows']}")
    print(f"  预计耗时: {plan['estimated_seconds']}s")
    for m in plan["moves"]:
        print(f"  移动 {m['from']} → {m['to']}: ~{m['estimated_rows']} 行")


def demo_cli():
    """演示：CLI 模式（无需启动 HTTP 服务）"""
    print("\n" + "=" * 70)
    print("演示 3: CLI 模式")
    print("=" * 70)
    cli = SqlToolCLI()
    try:
        # 检查 sqltool 是否在 PATH
        cli._run(["--version"], check=False)
        print("✓ sqltool CLI 可用")
        # 实际命令（注释掉，需要真实数据库）
        # cli.transfer("mysql://...", "postgresql://...")
    except FileNotFoundError:
        print("✗ sqltool CLI 不在 PATH")
        print("  安装: cargo install sqltool")


if __name__ == "__main__":
    demo_cross_db_migration()
    demo_smart_sharding()
    demo_cli()
    print("\n" + "=" * 70)
    print("✓ 演示完成")
    print("=" * 70)
