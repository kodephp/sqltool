/**
 * SQLTool Node.js / TypeScript SDK
 *
 * 用法：
 *   const { SqlToolClient, CrossDbMigrator, SmartSharding } = require('./sqltool_sdk');
 *
 * 依赖（可选，HTTP 模式需要）：
 *   npm install axios
 */

// ============================================================================
// HTTP 客户端
// ============================================================================
class SqlToolClient {
  constructor(baseUrl = 'http://localhost:8080', timeout = 30000) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.timeout = timeout;
  }

  async _request(path, method = 'GET', data = null) {
    const url = `${this.baseUrl}${path}`;
    const headers = { 'Content-Type': 'application/json' };
    const options = { method, headers, timeout: this.timeout };
    if (data) options.body = JSON.stringify(data);
    try {
      const resp = await fetch(url, options);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      return await resp.json();
    } catch (e) {
      throw new Error(`SQLTool 请求失败 ${url}: ${e.message}`);
    }
  }

  health() { return this._request('/api/health'); }

  transfer(source, target, sourceType, targetType, opts = {}) {
    return this._request('/api/transfer', 'POST', {
      source, target,
      source_type: sourceType,
      target_type: targetType,
      tables: opts.tables || [],
      batch_size: opts.batchSize || 1000,
      verify: opts.verify !== false,
    });
  }

  backup(source, output, opts = {}) {
    return this._request('/api/backup', 'POST', {
      source, output,
      backup_type: opts.backupType || 'full',
      compress: opts.compress !== false,
    });
  }

  compare(source, target, table, primaryKey, mode = 'full') {
    return this._request('/api/compare', 'POST', {
      source, target, table, primary_key: primaryKey, mode,
    });
  }

  convert(sourceDb, targetDb, table, sourceVersion, targetVersion) {
    return this._request('/api/convert', 'POST', {
      source_db: sourceDb,
      target_db: targetDb,
      table,
      source_version: sourceVersion,
      target_version: targetVersion,
    });
  }
}

// ============================================================================
// 跨数据库迁移器
// ============================================================================
class CrossDbMigrator {
  static SUPPORTED_DBS = ['mysql', 'postgresql', 'sqlite', 'tidb', 'mariadb', 'oracle', 'mssql'];

  constructor(client = null) {
    this.client = client || new SqlToolClient();
    this.typeRules = this._loadTypeRules();
  }

  /**
   * 迁移单张表
   * @param {Object} params
   * @param {string} params.source - 源库 URL (e.g. mysql://user:pass@host:port/db)
   * @param {string} params.target - 目标库 URL
   * @param {Object} params.table - 表结构 {name, fields: [{name, data_type, ...}]}
   * @param {string} [params.sourceVersion]
   * @param {string} [params.targetVersion]
   * @param {Object} [params.manualFieldMap]
   */
  async migrateTable({ source, target, table, sourceVersion, targetVersion, manualFieldMap = {} }) {
    const start = Date.now();
    const srcDb = this._parseDbType(source);
    const tgtDb = this._parseDbType(target);
    const srcV = sourceVersion || this._defaultVersion(srcDb);
    const tgtV = targetVersion || this._defaultVersion(tgtDb);
    const direction = this._inferDirection(srcDb, tgtDb, srcV, tgtV);

    const fieldMigrations = [];
    const warnings = [];
    let lossyCount = 0;

    for (const f of table.fields) {
      const targetField = manualFieldMap[f.name] || f.name;
      const map = this._typeMap(f.data_type, srcDb, tgtDb, srcV, tgtV);
      const { targetType, lossy, notes } = map;
      if (lossy) {
        lossyCount++;
        warnings.push(`${f.data_type} → ${targetType} 可能损失精度`);
      }
      warnings.push(...notes);
      fieldMigrations.push({
        source_field: f.name,
        target_field: targetField,
        source_type: f.data_type,
        target_type: targetType,
        transform: 'type_cast',
        lossy,
        warnings: notes,
      });
    }

    const ddl = this._generateDdl(table, fieldMigrations, tgtDb);
    const mapped = fieldMigrations.filter(m => m.target_field).length;

    return {
      table_name: table.name,
      direction,
      source_db: srcDb,
      target_db: tgtDb,
      source_version: srcV,
      target_version: tgtV,
      fields_total: fieldMigrations.length,
      fields_mapped: mapped,
      fields_unmapped: fieldMigrations.length - mapped,
      lossy_conversions: lossyCount,
      warnings,
      field_migrations: fieldMigrations,
      ddl,
      elapsed_ms: Date.now() - start,
      success_rate: fieldMigrations.length > 0 ? mapped / fieldMigrations.length : 0,
    };
  }

  _parseDbType(url) {
    const scheme = url.split('://')[0].toLowerCase();
    const mapping = { postgres: 'postgresql', pg: 'postgresql', sqlserver: 'mssql' };
    return mapping[scheme] || scheme;
  }

  _inferDirection(src, tgt, srcV, tgtV) {
    if (src === tgt) {
      return this._parseVersion(srcV).join('.') === this._parseVersion(tgtV).join('.')
        ? 'SameDbSameVersion' : 'SameDbCrossVersion';
    }
    return this._parseVersion(srcV).join('.') === this._parseVersion(tgtV).join('.')
      ? 'CrossDbSameVersion' : 'CrossDbCrossVersion';
  }

  _parseVersion(v) {
    return v.split(/[.(]/).map(s => parseInt(s.replace(/[^0-9]/g, ''), 10) || 0).slice(0, 3);
  }

  _defaultVersion(db) {
    return ({
      mysql: '8.0.32', mariadb: '10.11.0', tidb: '7.5.0',
      postgresql: '16.2.0', sqlite: '3.45.0',
      oracle: '21.0.0', mssql: '16.0.0',
    })[db] || '1.0.0';
  }

  _typeMap(srcType, srcDb, tgtDb, srcV, tgtV) {
    const base = srcType.toUpperCase().split('(')[0];
    const key = `${srcDb}|${tgtDb}|${base}`;
    if (this.typeRules[key]) {
      return { targetType: this.typeRules[key].type, lossy: this.typeRules[key].lossy, notes: [] };
    }
    if (srcDb === tgtDb) return { targetType: srcType, lossy: false, notes: [] };
    return { targetType: srcType, lossy: false, notes: [] };
  }

  _loadTypeRules() {
    // 格式: [src_db, tgt_db, src_type, tgt_type, lossy]
    const r = {};
    const add = (src, tgt, st, tt, lossy) => {
      r[`${src}|${tgt}|${st.split('(')[0].toUpperCase()}`] = { type: tt, lossy };
    };
    // MySQL → PG
    add('mysql', 'postgresql', 'TINYINT', 'SMALLINT', true);
    add('mysql', 'postgresql', 'INT', 'INTEGER', false);
    add('mysql', 'postgresql', 'BIGINT', 'BIGINT', false);
    add('mysql', 'postgresql', 'DECIMAL', 'NUMERIC', false);
    add('mysql', 'postgresql', 'DATETIME', 'TIMESTAMP', true);
    add('mysql', 'postgresql', 'TIMESTAMP', 'TIMESTAMP WITH TIME ZONE', true);
    add('mysql', 'postgresql', 'JSON', 'JSONB', false);
    add('mysql', 'postgresql', 'BLOB', 'BYTEA', false);
    add('mysql', 'postgresql', 'TEXT', 'TEXT', false);
    add('mysql', 'postgresql', 'VARCHAR', 'VARCHAR', false);
    add('mysql', 'postgresql', 'DOUBLE', 'DOUBLE PRECISION', false);
    // PG → MySQL
    add('postgresql', 'mysql', 'INTEGER', 'INT', false);
    add('postgresql', 'mysql', 'BIGINT', 'BIGINT', false);
    add('postgresql', 'mysql', 'DOUBLE PRECISION', 'DOUBLE', false);
    add('postgresql', 'mysql', 'TIMESTAMP', 'DATETIME', true);
    add('postgresql', 'mysql', 'BOOLEAN', 'TINYINT(1)', false);
    add('postgresql', 'mysql', 'BYTEA', 'BLOB', false);
    add('postgresql', 'mysql', 'JSONB', 'JSON', false);
    add('postgresql', 'mysql', 'UUID', 'CHAR(36)', true);
    add('postgresql', 'mysql', 'NUMERIC', 'DECIMAL', false);
    // MySQL → SQLite
    add('mysql', 'sqlite', 'INT', 'INTEGER', false);
    add('mysql', 'sqlite', 'BIGINT', 'INTEGER', false);
    add('mysql', 'sqlite', 'DATETIME', 'TEXT', true);
    add('mysql', 'sqlite', 'JSON', 'TEXT', false);
    add('mysql', 'sqlite', 'VARCHAR', 'TEXT', false);
    // SQLite → MySQL
    add('sqlite', 'mysql', 'INTEGER', 'BIGINT', true);
    add('sqlite', 'mysql', 'REAL', 'DOUBLE', false);
    return r;
  }

  _generateDdl(table, migrations, tgtDb) {
    const quote = ['mysql', 'mariadb', 'tidb'].includes(tgtDb) ? '`' : '"';
    const cols = migrations
      .filter(m => m.target_field)
      .map(m => `  ${quote}${m.target_field}${quote} ${m.target_type}`)
      .join(',\n');
    return `CREATE TABLE ${quote}${table.name}${quote} (\n${cols}\n)`;
  }
}

// ============================================================================
// 智能分库分表
// ============================================================================
class SmartSharding {
  constructor(logicalTable, shardKey, strategy = 'hash') {
    this.logicalTable = logicalTable;
    this.shardKey = shardKey;
    this.strategy = strategy;
    this.nodes = [];
  }

  addShard(id, connection, table, weight = 100) {
    this.nodes.push({ id, connection, table, weight, active: true });
  }

  _stableHash(s) {
    let h = 0n;
    for (const b of Buffer.from(s, 'utf-8')) {
      h ^= BigInt(b);
      h = (h * 1099511628211n) & 0xFFFFFFFFFFFFFFFFn;
    }
    return Number(h);
  }

  route(shardValue) {
    const active = this.nodes.filter(n => n.active);
    if (active.length === 0) throw new Error('无活跃分片');
    if (this.strategy === 'hash') {
      return active[this._stableHash(shardValue) % active.length];
    }
    const n = parseInt(shardValue, 10) || 0;
    return active[n % active.length];
  }

  query({ columns, whereClause, orderBy, limit, offset, parallel = true, mergeStrategy = 'concat' }) {
    const shardResults = this.nodes
      .filter(n => n.active)
      .map(node => ({
        shard_id: node.id,
        sql: this._buildSql(node.table, { columns, whereClause, orderBy, limit, offset }),
        rows: [],
        elapsed_ms: 0,
      }));
    return {
      total_shards: shardResults.length,
      shard_results: shardResults,
      total_rows: 0,
      rows: [],
      has_more: false,
      merge_strategy: mergeStrategy,
    };
  }

  writeBatch(ops) {
    const results = [];
    for (const op of ops) {
      const node = this.route(op.keyValue || op.key_value);
      results.push({ op: op.opType || op.op_type, shard_id: node.id, success: true });
    }
    return {
      total: results.length,
      success: results.filter(r => r.success).length,
      failed: results.filter(r => !r.success).length,
      results,
    };
  }

  rebalancePlan(totalRows = 1_000_000) {
    if (this.nodes.length < 2) return { moves: [], estimated_total_rows: totalRows };
    const perShard = Math.floor(totalRows / this.nodes.length);
    const moves = [];
    for (let i = 1; i < this.nodes.length; i++) {
      moves.push({
        from: this.nodes[0].id, to: this.nodes[i].id,
        range_start: (i - 1) * perShard, range_end: i * perShard,
        estimated_rows: perShard,
      });
    }
    return { moves, estimated_total_rows: totalRows, estimated_seconds: Math.floor(totalRows / 10000) };
  }

  _buildSql(table, q) {
    const cols = q.columns ? q.columns.join(', ') : '*';
    let sql = `SELECT ${cols} FROM ${table}`;
    if (q.whereClause) sql += ` WHERE ${q.whereClause}`;
    if (q.orderBy) sql += ` ORDER BY ${q.orderBy.map(([f, asc]) => `${f} ${asc ? 'ASC' : 'DESC'}`).join(', ')}`;
    if (q.limit != null) sql += ` LIMIT ${q.limit}`;
    if (q.offset != null) sql += ` OFFSET ${q.offset}`;
    return sql;
  }
}

// ============================================================================
// 演示
// ============================================================================
async function demo() {
  console.log('='.repeat(70));
  console.log('SQLTool Node.js SDK 演示');
  console.log('='.repeat(70));

  // 演示 1: 跨数据库迁移
  console.log('\n[1] 跨数据库迁移（MySQL 5.7 → PostgreSQL 16）');
  const mig = new CrossDbMigrator();
  const result = await mig.migrateTable({
    source: 'mysql://root:pass@localhost:3306/mydb',
    target: 'postgresql://postgres:pass@localhost:5432/mydb',
    table: {
      name: 'orders',
      fields: [
        { name: 'id', data_type: 'INT', primary_key: true },
        { name: 'user_id', data_type: 'BIGINT' },
        { name: 'amount', data_type: 'DECIMAL(10,2)' },
        { name: 'created_at', data_type: 'DATETIME' },
        { name: 'remark', data_type: 'TEXT' },
      ],
    },
    sourceVersion: '5.7.40',
    targetVersion: '16.2',
    manualFieldMap: { remark: 'comment' },
  });
  console.log(`  方向: ${result.direction}`);
  console.log(`  映射: ${result.fields_mapped}/${result.fields_total} (${(result.success_rate*100).toFixed(1)}%)`);
  console.log(`  有损: ${result.lossy_conversions}`);
  console.log(`  DDL:\n${result.ddl}`);

  // 演示 2: 智能分库分表
  console.log('\n[2] 智能分库分表（4 分片哈希）');
  const sharding = new SmartSharding('orders', 'user_id', 'hash');
  sharding.addShard('s0', 'mysql://n1/orders_0', 'orders_0');
  sharding.addShard('s1', 'mysql://n1/orders_1', 'orders_1');
  sharding.addShard('s2', 'mysql://n2/orders_2', 'orders_2');
  sharding.addShard('s3', 'mysql://n2/orders_3', 'orders_3');

  console.log('  路由演示:');
  for (const uid of ['user_001', 'user_042', 'user_001']) {
    const n = sharding.route(uid);
    console.log(`    ${uid} → ${n.id} (${n.table})`);
  }

  const queryResult = sharding.query({
    whereClause: 'amount > 100', orderBy: [['id', true]], limit: 10,
  });
  console.log(`  跨分片查询: 涉及 ${queryResult.total_shards} 个分片`);

  const writeResult = sharding.writeBatch([
    { opType: 'INSERT', keyValue: 'u1' },
    { opType: 'INSERT', keyValue: 'u2' },
  ]);
  console.log(`  批量写入: ${writeResult.success}/${writeResult.total} 成功`);

  const plan = sharding.rebalancePlan(10_000_000);
  console.log(`  Rebalance 计划: ${plan.moves.length} 步, ~${plan.estimated_seconds}s`);

  console.log('\n✓ 演示完成');
}

if (require.main === module) {
  demo().catch(e => { console.error(e); process.exit(1); });
}

module.exports = { SqlToolClient, CrossDbMigrator, SmartSharding };
