#!/usr/bin/env node
/**
 * SQLTool Node.js 调用示例
 *
 * 安装依赖:
 *   npm install axios
 *
 * 使用方法:
 *   node sqltool_demo.js           # HTTP API 模式
 *   node sqltool_demo.js --cli     # CLI 模式
 */

const { execSync } = require('child_process');
const axios = require('axios');

class SqlToolClient {
    constructor(baseUrl = 'http://localhost:8080') {
        this.baseUrl = baseUrl;
        this.client = axios.create({
            baseURL: baseUrl,
            headers: { 'Content-Type': 'application/json' },
            timeout: 30000
        });
    }

    async healthCheck() {
        const resp = await this.client.get('/api/health');
        return resp.data;
    }

    async detectInjection(input) {
        const resp = await this.client.post('/api/security/detect-injection', { input });
        return resp.data;
    }

    async buildSafeSql(table, field, operator, value) {
        const resp = await this.client.post('/api/security/build-safe-sql', {
            table, field, operator, value
        });
        return resp.data;
    }

    async backup(source, dbType, backupType = 'full') {
        const resp = await this.client.post('/api/backup', {
            source, db_type: dbType, backup_type: backupType
        });
        return resp.data;
    }

    async transfer(source, target, sourceType, targetType) {
        const resp = await this.client.post('/api/transfer', {
            source, target, source_type: sourceType, target_type: targetType
        });
        return resp.data;
    }
}

class SqlToolCLI {
    run(...args) {
        try {
            const cmd = ['sqltool', ...args].join(' ');
            return execSync(cmd, { encoding: 'utf8', stdio: 'pipe' });
        } catch (error) {
            console.error('命令执行失败:', error.message);
            return error.message;
        }
    }

    detectInjection(input) {
        return this.run('detect-sql-injection', '--input', input);
    }

    buildSafeSql(table, field, operator, value) {
        return this.run('build-safe-sql', '--table', table, '--field', field,
                        '--operator', operator, '--value', value);
    }

    backup(source, output, backupType = 'full') {
        return this.run('backup', '-s', source, '--output', output,
                        '--backup-type', backupType);
    }

    transfer(source, target, batchSize = 1000) {
        return this.run('transfer', '-s', source, '-t', target, '-B', String(batchSize));
    }
}

function printResult(title, result) {
    console.log('\n' + '='.repeat(50));
    console.log(title);
    console.log('='.repeat(50));
    console.log(typeof result === 'object' ? JSON.stringify(result, null, 2) : result);
}

async function main() {
    const args = process.argv.slice(2);
    const useCLI = args.includes('--cli');

    console.log(`
╔══════════════════════════════════════════════════╗
║         SQLTool Node.js 调用示例                  ║
╚══════════════════════════════════════════════════╝
    `);

    if (useCLI) {
        console.log('模式: CLI (不需要启动 server)\n');
        const client = new SqlToolCLI();

        printResult('1. SQL注入检测', client.detectInjection("' OR '1'='1"));
        printResult('2. 构建安全SQL', client.buildSafeSql('users', 'name', '=', "test'; DROP TABLE"));
    } else {
        console.log('模式: HTTP API (需要启动 sqltool server)\n');
        const client = new SqlToolClient();

        try {
            printResult('0. 健康检查', await client.healthCheck());
            printResult('1. SQL注入检测 - 恶意输入', await client.detectInjection("' OR '1'='1"));
            printResult('2. SQL注入检测 - 正常输入', await client.detectInjection('normal_input'));
            printResult('3. 构建安全SQL', await client.buildSafeSql('users', 'name', '=', "test'; DROP TABLE"));
        } catch (error) {
            if (error.code === 'ECONNREFUSED') {
                console.error('\n错误: 无法连接到 http://localhost:8080');
                console.error('请先启动 sqltool server:');
                console.error('  sqltool server -p 8080 -s mysql://localhost/mydb');
                process.exit(1);
            }
            throw error;
        }
    }

    console.log('\n' + '='.repeat(50));
    console.log('示例执行完成!');
    console.log('='.repeat(50));
}

main().catch(console.error);
