#!/usr/bin/env node
/**
 * SQLTool Node.js 完整调用示例
 *
 * 功能覆盖：
 *   - 数据迁移 (transfer)
 *   - 数据备份 (backup)
 *   - 数据对比 (compare-data)
 *   - 分库分表 (create-shard)
 *   - 慢查询检测 (detect-slow-query)
 *   - 跨分片查询 (spanning-query)
 *   - 日志管理 (insert-log/query-logs)
 *   - SQL注入检测 (detect-sql-injection)
 *   - 安全SQL构建 (build-safe-sql)
 *
 * 安装依赖:
 *   npm install axios
 *
 * 使用方法:
 *   node sqltool_demo.js                    # HTTP API 模式
 *   node sqltool_demo.js --cli             # CLI 模式
 */

const { execSync } = require('child_process');
const axios = require('axios');

// =============================================================================
// HTTP API 客户端
// =============================================================================

class SqlToolClient {
    constructor(baseUrl = 'http://localhost:8080', apiKey = null) {
        this.baseUrl = baseUrl.replace(/\/$/, '');
        this.client = axios.create({
            baseURL: this.baseUrl,
            timeout: 60000,
            headers: { 'Content-Type': 'application/json' }
        });
        if (apiKey) {
            this.client.defaults.headers.common['Authorization'] = `Bearer ${apiKey}`;
        }
    }

    async _post(path, data) {
        const response = await this.client.post(path, data);
        return response.data;
    }

    async _get(path) {
        const response = await this.client.get(path);
        return response.data;
    }

    // -------------------------------------------------------------------------
    // 健康检查
    // -------------------------------------------------------------------------

    async healthCheck() {
        return this._get('/api/health');
    }

    // -------------------------------------------------------------------------
    // 数据迁移
    // -------------------------------------------------------------------------

    /**
     * 数据迁移
     *
     * @param {Object} params
     * @param {string} params.source - 源数据库连接字符串
     * @param {string} params.target - 目标数据库连接字符串
     * @param {string} params.sourceType - 源数据库类型 (mysql/postgresql/sqlite/oracle)
     * @param {string} params.targetType - 目标数据库类型
     * @param {string} params.tables - 表名列表，逗号分隔，空表示所有表
     * @param {number} params.batchSize - 批量大小
     * @param {boolean} params.verifyData - 是否验证数据
     * @param {boolean} params.skipErrors - 是否跳过错误
     */
    async transfer({
        source,
        target,
        sourceType = 'mysql',
        targetType = 'postgresql',
        tables = '',
        batchSize = 1000,
        verifyData = true,
        skipErrors = true
    }) {
        return this._post('/api/transfer', {
            source,
            target,
            source_type: sourceType,
            target_type: targetType,
            tables,
            batch_size: batchSize,
            verify_data: verifyData,
            skip_errors: skipErrors
        });
    }

    // -------------------------------------------------------------------------
    // 数据库备份
    // -------------------------------------------------------------------------

    /**
     * 数据库备份
     *
     * @param {Object} params
     * @param {string} params.source - 数据库连接字符串
     * @param {string} params.dbType - 数据库类型
     * @param {string} params.output - 备份文件路径
     * @param {string} params.backupType - 备份类型 (full/incremental/differential)
     * @param {boolean} params.compress - 是否压缩
     * @param {boolean} params.includeProcedures - 包含存储过程
     * @param {boolean} params.includeFunctions - 包含函数
     * @param {boolean} params.includeTriggers - 包含触发器
     * @param {number} params.parallelTables - 并行备份表数
     */
    async backup({
        source,
        dbType = 'mysql',
        output = '/tmp/backup.sql',
        backupType = 'full',
        compress = true,
        includeProcedures = true,
        includeFunctions = true,
        includeTriggers = true,
        parallelTables = 4
    }) {
        return this._post('/api/backup', {
            source,
            db_type: dbType,
            output,
            backup_type: backupType,
            compress,
            include_procedures: includeProcedures,
            include_functions: includeFunctions,
            include_triggers: includeTriggers,
            parallel_tables: parallelTables
        });
    }

    // -------------------------------------------------------------------------
    // 数据对比
    // -------------------------------------------------------------------------

    /**
     * 数据对比
     *
     * @param {Object} params
     * @param {string} params.source - 源数据库
     * @param {string} params.target - 目标数据库
     * @param {string} params.table - 表名
     * @param {string} params.sourceType - 源类型
     * @param {string} params.targetType - 目标类型
     * @param {string} params.primaryKey - 主键字段
     * @param {string} params.ignoreFields - 忽略字段
     * @param {string} params.compareMode - 对比模式 (full/sample)
     */
    async compareData({
        source,
        target,
        table,
        sourceType = 'mysql',
        targetType = 'mysql',
        primaryKey = 'id',
        ignoreFields = '',
        compareMode = 'full'
    }) {
        return this._post('/api/compare', {
            source,
            target,
            source_type: sourceType,
            target_type: targetType,
            table,
            primary_key: primaryKey,
            ignore_fields: ignoreFields,
            compare_mode: compareMode
        });
    }

    // -------------------------------------------------------------------------
    // 分库分表
    // -------------------------------------------------------------------------

    /**
     * 创建分片
     *
     * @param {Object} params
     * @param {string} params.source - 数据库连接
     * @param {string} params.table - 表名
     * @param {string} params.strategy - 分片策略 (row_count/time/size/hash)
     * @param {string} params.threshold - 阈值
     * @param {string} params.prefix - 分片前缀
     */
    async createShard({
        source,
        table,
        strategy = 'row_count',
        threshold = '1000000',
        prefix = 'shard'
    }) {
        return this._post('/api/shard/create', {
            source,
            table,
            strategy,
            threshold,
            prefix
        });
    }

    // -------------------------------------------------------------------------
    // 慢查询检测
    // -------------------------------------------------------------------------

    /**
     * 慢查询检测
     *
     * @param {Object} params
     * @param {string} params.source - 数据库连接
     * @param {string} params.dbType - 数据库类型
     * @param {number} params.thresholdMs - 阈值（毫秒）
     * @param {number} params.limit - 返回数量
     */
    async detectSlowQuery({
        source,
        dbType = 'mysql',
        thresholdMs = 1000,
        limit = 10
    }) {
        return this._post('/api/detect-slow', {
            source,
            db_type: dbType,
            threshold_ms: thresholdMs,
            limit
        });
    }

    // -------------------------------------------------------------------------
    // 跨分片查询
    // -------------------------------------------------------------------------

    /**
     * 跨分片查询
     *
     * @param {Object} params
     * @param {string} params.source - 数据库连接
     * @param {string} params.table - 表名
     * @param {string} params.condition - WHERE条件
     * @param {string} params.orderBy - 排序字段
     * @param {string} params.orderDir - 排序方向 (ASC/DESC)
     * @param {number} params.limit - 返回数量
     * @param {number} params.offset - 偏移量
     */
    async spanningQuery({
        source,
        table,
        condition = '1=1',
        orderBy = '',
        orderDir = 'ASC',
        limit = 100,
        offset = 0
    }) {
        return this._post('/api/spanning-query', {
            source,
            table,
            condition,
            order_by: orderBy,
            order_dir: orderDir,
            limit,
            offset
        });
    }

    // -------------------------------------------------------------------------
    // 日志管理 - 插入
    // -------------------------------------------------------------------------

    /**
     * 插入日志
     *
     * @param {Object} params
     * @param {string} params.source - 数据库连接
     * @param {string} params.table - 日志表名
     * @param {string} params.level - 日志级别 (DEBUG/INFO/WARN/ERROR)
     * @param {string} params.message - 日志消息
     * @param {string} params.sourceName - 来源名称
     */
    async insertLog({
        source,
        table = 'app_logs',
        level = 'INFO',
        message = '',
        sourceName = ''
    }) {
        return this._post('/api/log/insert', {
            source,
            table,
            level,
            message,
            source_name: sourceName
        });
    }

    // -------------------------------------------------------------------------
    // 日志管理 - 查询
    // -------------------------------------------------------------------------

    /**
     * 查询日志
     *
     * @param {Object} params
     * @param {string} params.source - 数据库连接
     * @param {string} params.table - 日志表名
     * @param {string} params.levels - 级别过滤（逗号分隔）
     * @param {string} params.keyword - 关键字过滤
     * @param {number} params.startTime - 开始时间
     * @param {number} params.endTime - 结束时间
     * @param {number} params.limit - 返回数量
     */
    async queryLogs({
        source,
        table = 'app_logs',
        levels = '',
        keyword = '',
        startTime = 0,
        endTime = 0,
        limit = 100
    }) {
        const result = await this._post('/api/log/query', {
            source,
            table,
            levels,
            keyword,
            start_time: startTime,
            end_time: endTime,
            limit
        });
        return result.rows || [];
    }

    // -------------------------------------------------------------------------
    // SQL注入检测
    // -------------------------------------------------------------------------

    /**
     * SQL注入检测
     *
     * @param {string} inputText - 要检测的输入
     */
    async detectInjection(inputText) {
        return this._post('/api/security/detect-injection', {
            input: inputText
        });
    }

    // -------------------------------------------------------------------------
    // 安全SQL构建
    // -------------------------------------------------------------------------

    /**
     * 安全SQL构建
     *
     * @param {Object} params
     * @param {string} params.table - 表名
     * @param {string} params.field - 字段名
     * @param {string} params.operator - 操作符 (=, !=, <, >, LIKE, IN)
     * @param {string} params.value - 值
     */
    async buildSafeSql({
        table,
        field,
        operator = '=',
        value = ''
    }) {
        return this._post('/api/security/build-safe-sql', {
            table,
            field,
            operator,
            value
        });
    }
}


// =============================================================================
// CLI 客户端
// =============================================================================

class SqlToolCLI {
    constructor(binaryPath = 'sqltool') {
        this.binaryPath = binaryPath;
    }

    run(...args) {
        try {
            const result = execSync(`${this.binaryPath} ${args.join(' ')}`, {
                encoding: 'utf-8',
                timeout: 60000
            });
            return result;
        } catch (error) {
            return `错误: ${error.message}`;
        }
    }

    // -------------------------------------------------------------------------
    // 数据迁移
    // -------------------------------------------------------------------------

    /**
     * 数据迁移
     *
     * 示例:
     *   cli.transfer(
     *     'mysql://root:pass@localhost:3306/source',
     *     'postgresql://postgres:pass@localhost:5432/target',
     *     'mysql', 'postgresql', 'users,orders', 5000
     *   );
     */
    transfer(source, target, sourceType, targetType, tables = '', batchSize = 1000) {
        const args = [
            'transfer',
            '-s', source,
            '-t', target,
            '-S', sourceType,
            '-T', targetType,
            '-B', batchSize
        ];
        if (tables) args.push('--tables', tables);
        return this.run(...args);
    }

    // -------------------------------------------------------------------------
    // 数据库备份
    // -------------------------------------------------------------------------

    /**
     * 数据库备份
     *
     * 示例:
     *   cli.backup(
     *     'mysql://root:pass@localhost:3306/mydb',
     *     '/tmp/backup.sql', 'mysql', 'full', true
     *   );
     */
    backup(source, output, dbType = 'mysql', backupType = 'full', compress = true) {
        const args = ['backup', '-s', source, '-T', dbType, '-o', output, '-t', backupType];
        if (compress) args.push('-c');
        return this.run(...args);
    }

    // -------------------------------------------------------------------------
    // 数据对比
    // -------------------------------------------------------------------------

    /**
     * 数据对比
     *
     * 示例:
     *   cli.compareData(
     *     'mysql://root@localhost/db1',
     *     'mysql://root@localhost/db2',
     *     'users', 'id', 'mysql', 'mysql'
     *   );
     */
    compareData(source, target, table, primaryKey = 'id', sourceType = 'mysql', targetType = 'mysql') {
        return this.run(
            'compare-data',
            '-s', source,
            '-t', target,
            '-S', sourceType,
            '-T', targetType,
            '--table', table,
            '--primary-key', primaryKey
        );
    }

    // -------------------------------------------------------------------------
    // 创建分片
    // -------------------------------------------------------------------------

    /**
     * 创建分片
     *
     * 示例:
     *   cli.createShard(
     *     'mysql://root:pass@localhost:3306/mydb',
     *     'orders', 'row_count', '1000000', 'orders_shard'
     *   );
     */
    createShard(source, table, strategy = 'row_count', threshold = '1000000', prefix = 'shard') {
        return this.run(
            'create-shard',
            '-s', source,
            '--table', table,
            '--strategy', strategy,
            '--threshold', threshold,
            '--prefix', prefix
        );
    }

    // -------------------------------------------------------------------------
    // 慢查询检测
    // -------------------------------------------------------------------------

    /**
     * 慢查询检测
     *
     * 示例:
     *   cli.detectSlowQuery(
     *     'mysql://root:pass@localhost:3306/mydb',
     *     'mysql', 1000
     *   );
     */
    detectSlowQuery(source, dbType = 'mysql', thresholdMs = 1000) {
        return this.run(
            'detect-slow-query',
            '-s', source,
            '-T', dbType,
            '--threshold-ms', thresholdMs
        );
    }

    // -------------------------------------------------------------------------
    // 跨分片查询
    // -------------------------------------------------------------------------

    /**
     * 跨分片查询
     *
     * 示例:
     *   cli.spanningQuery(
     *     'mysql://root:pass@localhost:3306/mydb',
     *     'orders', "status='pending'", 'created_at', 100, 0
     *   );
     */
    spanningQuery(source, table, condition = '1=1', orderBy = '', limit = 100, offset = 0) {
        const args = [
            'spanning-query',
            '-s', source,
            '--table', table,
            '--condition', condition,
            '-L', limit,
            '--offset', offset
        ];
        if (orderBy) args.push('--order-by', orderBy);
        return this.run(...args);
    }

    // -------------------------------------------------------------------------
    // 插入日志
    // -------------------------------------------------------------------------

    /**
     * 插入日志
     *
     * 示例:
     *   cli.insertLog(
     *     'mysql://root:pass@localhost:3306/mydb',
     *     '用户登录成功', 'app_logs', 'INFO', 'auth-service'
     *   );
     */
    insertLog(source, message, table = 'app_logs', level = 'INFO', sourceName = '') {
        const args = [
            'insert-log',
            '-s', source,
            '--table', table,
            '--level', level,
            '--message', message
        ];
        if (sourceName) args.push('--source-name', sourceName);
        return this.run(...args);
    }

    // -------------------------------------------------------------------------
    // 查询日志
    // -------------------------------------------------------------------------

    /**
     * 查询日志
     *
     * 示例:
     *   cli.queryLogs(
     *     'mysql://root:pass@localhost:3306/mydb',
     *     'app_logs', 'ERROR,WARN', 'login', 50
     *   );
     */
    queryLogs(source, table = 'app_logs', levels = '', keyword = '', limit = 100) {
        const args = ['query-logs', '-s', source, '--table', table, '-L', limit];
        if (levels) args.push('--levels', levels);
        if (keyword) args.push('--keyword', keyword);
        return this.run(...args);
    }

    // -------------------------------------------------------------------------
    // SQL注入检测
    // -------------------------------------------------------------------------

    /**
     * SQL注入检测
     *
     * 示例:
     *   cli.detectInjection("' OR '1'='1");
     */
    detectInjection(inputText) {
        return this.run('detect-sql-injection', '-i', inputText);
    }

    // -------------------------------------------------------------------------
    // 安全SQL构建
    // -------------------------------------------------------------------------

    /**
     * 安全SQL构建
     *
     * 示例:
     *   cli.buildSafeSql('users', 'name', '=', "John O'Brien");
     */
    buildSafeSql(table, field, operator = '=', value = '') {
        return this.run(
            'build-safe-sql',
            '--table', table,
            '--field', field,
            '--operator', operator,
            '--value', value
        );
    }
}


// =============================================================================
// 主函数
// =============================================================================

async function main() {
    const args = process.argv.slice(2);
    const useCLI = args.includes('--cli');
    const binaryPath = args.find(arg => arg.startsWith('--binary='))?.split('=')[1]
        || '/Users/Zhuanz/Desktop/website/composer/sqlmap/target/release/sqltool';

    console.log(`
╔════════════════════════════════════════════════════════════╗
║         SQLTool Node.js 完整调用示例 v0.4.1           ║
╚════════════════════════════════════════════════════════════╝
    `);

    if (useCLI) {
        console.log('模式: CLI');
        console.log(`二进制: ${binaryPath}\n`);

        const cli = new SqlToolCLI(binaryPath);

        // 1. SQL注入检测
        console.log('1. SQL注入检测...');
        console.log('='.repeat(60));
        console.log(cli.detectInjection("' OR '1'='1"));

        // 2. 安全SQL构建
        console.log('\n2. 安全SQL构建...');
        console.log('='.repeat(60));
        console.log(cli.buildSafeSql('users', 'name', '=', "test'; DROP TABLE"));

        // 3. 数据迁移
        console.log('\n3. 数据迁移...');
        console.log('='.repeat(60));
        console.log(cli.transfer(
            'mysql://root:pass@localhost:3306/source',
            'postgresql://postgres:pass@localhost:5432/target',
            'mysql', 'postgresql', 'users,orders', 5000
        ));

        // 4. 数据库备份
        console.log('\n4. 数据库备份...');
        console.log('='.repeat(60));
        console.log(cli.backup(
            'mysql://root:pass@localhost:3306/mydb',
            '/tmp/backup.sql', 'mysql', 'full', true
        ));

        // 5. 数据对比
        console.log('\n5. 数据对比...');
        console.log('='.repeat(60));
        console.log(cli.compareData(
            'mysql://root@localhost/db1',
            'mysql://root@localhost/db2',
            'users', 'id'
        ));

    } else {
        console.log('模式: HTTP API');
        console.log('URL: http://localhost:8080\n');

        const client = new SqlToolClient('http://localhost:8080');

        try {
            // 健康检查
            console.log('0. 健康检查...');
            console.log('='.repeat(60));
            console.log(JSON.stringify(await client.healthCheck(), null, 2));

            // 1. SQL注入检测
            console.log('\n1. SQL注入检测...');
            console.log('='.repeat(60));
            const injResult = await client.detectInjection("' OR '1'='1");
            console.log(JSON.stringify(injResult, null, 2));
            if (injResult.risk_level === 'High' || injResult.risk_level === 'Critical') {
                console.log('⚠️ 警告: 检测到高风险SQL注入攻击!');
            }

            // 2. 安全SQL构建
            console.log('\n2. 安全SQL构建...');
            console.log('='.repeat(60));
            const sqlResult = await client.buildSafeSql({
                table: 'users',
                field: 'email',
                operator: 'LIKE',
                value: '%@example.com'
            });
            console.log(JSON.stringify(sqlResult, null, 2));

            // 3. 数据迁移示例
            console.log('\n3. 数据迁移 (需要真实数据库连接)...');
            console.log('='.repeat(60));
            const transferResult = await client.transfer({
                source: 'mysql://root:password@localhost:3306/source_db',
                target: 'postgresql://postgres:password@localhost:5432/target_db',
                sourceType: 'mysql',
                targetType: 'postgresql',
                tables: 'users,orders,products',
                batchSize: 5000,
                verifyData: true
            });
            console.log(JSON.stringify(transferResult, null, 2));

            // 4. 数据库备份示例
            console.log('\n4. 数据库备份 (需要真实数据库连接)...');
            console.log('='.repeat(60));
            const backupResult = await client.backup({
                source: 'mysql://root:password@localhost:3306/mydb',
                dbType: 'mysql',
                output: '/tmp/backup_20240101.sql',
                backupType: 'full',
                compress: true
            });
            console.log(JSON.stringify(backupResult, null, 2));

            // 5. 数据对比示例
            console.log('\n5. 数据对比 (需要真实数据库连接)...');
            console.log('='.repeat(60));
            const compareResult = await client.compareData({
                source: 'mysql://root:password@localhost:3306/db1',
                target: 'mysql://root:password@localhost:3306/db2',
                table: 'users',
                primaryKey: 'id',
                ignoreFields: 'updated_at'
            });
            console.log(JSON.stringify(compareResult, null, 2));

            // 6. 分库分表示例
            console.log('\n6. 分库分表 (需要真实数据库连接)...');
            console.log('='.repeat(60));
            const shardResult = await client.createShard({
                source: 'mysql://root:password@localhost:3306/mydb',
                table: 'orders',
                strategy: 'row_count',
                threshold: '1000000',
                prefix: 'orders_shard'
            });
            console.log(JSON.stringify(shardResult, null, 2));

            // 7. 慢查询检测示例
            console.log('\n7. 慢查询检测 (需要真实数据库连接)...');
            console.log('='.repeat(60));
            const slowResult = await client.detectSlowQuery({
                source: 'mysql://root:password@localhost:3306/mydb',
                thresholdMs: 1000,
                limit: 10
            });
            console.log(JSON.stringify(slowResult, null, 2));

            // 8. 跨分片查询示例
            console.log('\n8. 跨分片查询 (需要真实数据库连接)...');
            console.log('='.repeat(60));
            const spanResult = await client.spanningQuery({
                source: 'mysql://root:password@localhost:3306/mydb',
                table: 'orders',
                condition: "status='pending'",
                orderBy: 'created_at',
                orderDir: 'DESC',
                limit: 100
            });
            console.log(JSON.stringify(spanResult, null, 2));

            // 9. 插入日志示例
            console.log('\n9. 插入日志 (需要真实数据库连接)...');
            console.log('='.repeat(60));
            const logInsertResult = await client.insertLog({
                source: 'mysql://root:password@localhost:3306/mydb',
                table: 'app_logs',
                level: 'INFO',
                message: '用户登录成功',
                sourceName: 'auth-service'
            });
            console.log(JSON.stringify(logInsertResult, null, 2));

            // 10. 查询日志示例
            console.log('\n10. 查询日志 (需要真实数据库连接)...');
            console.log('='.repeat(60));
            const logQueryResult = await client.queryLogs({
                source: 'mysql://root:password@localhost:3306/mydb',
                table: 'app_logs',
                levels: 'ERROR,WARN',
                keyword: 'login',
                limit: 50
            });
            console.log(JSON.stringify(logQueryResult, null, 2));

        } catch (error) {
            console.error(`\n错误: ${error.message}`);
            if (error.code === 'ECONNREFUSED') {
                console.log('\n请先启动 sqltool server:');
                console.log('  sqltool server -p 8080 -s mysql://localhost/mydb');
            }
            process.exit(1);
        }
    }

    console.log('\n' + '='.repeat(60));
    console.log('示例执行完成!');
    console.log('='.repeat(60));
}

main();
