#!/usr/bin/env ruby
# frozen_string_literal: true

# SQLTool Ruby 完整调用示例
#
# 功能覆盖：
#   - 数据迁移 (transfer)
#   - 数据备份 (backup)
#   - 数据对比 (compare-data)
#   - 分库分表 (create-shard)
#   - 慢查询检测 (detect-slow-query)
#   - 跨分片查询 (spanning-query)
#   - 日志管理 (insert-log/query-logs)
#   - SQL注入检测 (detect-sql-injection)
#   - 安全SQL构建 (build-safe-sql)
#
# 安装依赖: 无 (使用net/http)
#
# 使用方法:
#   ruby sqltool_demo.rb                    # HTTP API 模式
#   ruby sqltool_demo.rb --cli             # CLI 模式

require 'net/http'
require 'uri'
require 'json'
require 'open3'

# =============================================================================
# HTTP API 客户端
# =============================================================================

class SqlToolClient
  def initialize(base_url = 'http://localhost:8080', api_key = nil)
    @base_url = base_url.chomp('/')
    @headers = { 'Content-Type' => 'application/json' }
    @headers['Authorization'] = "Bearer #{api_key}" if api_key
  end

  def _post(path, data)
    uri = URI("#{@base_url}#{path}")
    req = Net::HTTP::Post.new(uri, @headers)
    req.body = data.to_json
    res = Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req) }
    JSON.parse(res.body)
  end

  def _get(path)
    uri = URI("#{@base_url}#{path}")
    req = Net::HTTP::Get.new(uri, @headers)
    res = Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req) }
    JSON.parse(res.body)
  end

  # 健康检查
  def health_check
    _get('/api/health')
  end

  # 数据迁移
  def transfer(source:, target:, source_type: 'mysql', target_type: 'postgresql',
               tables: '', batch_size: 1000, verify_data: true, skip_errors: true)
    _post('/api/transfer', {
      source:, target:,
      source_type:, target_type:,
      tables:, batch_size:,
      verify_data:, skip_errors:
    })
  end

  # 数据库备份
  def backup(source:, db_type: 'mysql', output: '/tmp/backup.sql',
             backup_type: 'full', compress: true)
    _post('/api/backup', {
      source:, db_type:, output:, backup_type:, compress:,
      include_procedures: true, include_functions: true,
      include_triggers: true, parallel_tables: 4
    })
  end

  # 数据对比
  def compare_data(source:, target:, table:, primary_key: 'id',
                   source_type: 'mysql', target_type: 'mysql')
    _post('/api/compare', {
      source:, target:, source_type:, target_type:,
      table:, primary_key:, ignore_fields: '', compare_mode: 'full'
    })
  end

  # 创建分片
  def create_shard(source:, table:, strategy: 'row_count',
                   threshold: '1000000', prefix: 'shard')
    _post('/api/shard/create', { source:, table:, strategy:, threshold:, prefix: })
  end

  # 慢查询检测
  def detect_slow_query(source:, db_type: 'mysql', threshold_ms: 1000, limit: 10)
    _post('/api/detect-slow', { source:, db_type:, threshold_ms:, limit: })
  end

  # 跨分片查询
  def spanning_query(source:, table:, condition: '1=1', order_by: '',
                     order_dir: 'ASC', limit: 100, offset: 0)
    _post('/api/spanning-query', {
      source:, table:, condition:, order_by:, order_dir:, limit:, offset:
    })
  end

  # 插入日志
  def insert_log(source:, level: 'INFO', message: '',
                 table: 'app_logs', source_name: '')
    _post('/api/log/insert', { source:, table:, level:, message:, source_name: })
  end

  # 查询日志
  def query_logs(source:, levels: '', keyword: '',
                 table: 'app_logs', limit: 100)
    result = _post('/api/log/query', {
      source:, table:, levels:, keyword:,
      start_time: 0, end_time: 0, limit:
    })
    result['rows'] || []
  end

  # SQL注入检测
  def detect_injection(input)
    _post('/api/security/detect-injection', { input: })
  end

  # 安全SQL构建
  def build_safe_sql(table:, field:, operator: '=', value: '')
    _post('/api/security/build-safe-sql', { table:, field:, operator:, value: })
  end
end

# =============================================================================
# CLI 客户端
# =============================================================================

class SqlToolCLI
  def initialize(binary_path = 'sqltool')
    @binary_path = binary_path
  end

  def run(*args)
    cmd = "#{@binary_path} #{args.join(' ')}"
    stdout, _status = Open3.capture2(cmd)
    stdout
  rescue => e
    "错误: #{e.message}"
  end

  def transfer(source, target, source_type, target_type, tables = '', batch_size = 1000)
    run('transfer', '-s', source, '-t', target, '-S', source_type,
        '-T', target_type, '-B', batch_size.to_s, '--tables', tables)
  end

  def backup(source, output, db_type = 'mysql', backup_type = 'full', compress = true)
    args = ['backup', '-s', source, '-T', db_type, '-o', output, '-t', backup_type]
    args << '-c' if compress
    run(*args)
  end

  def compare_data(source, target, table, primary_key = 'id')
    run('compare-data', '-s', source, '-t', target, '--table', table,
        '--primary-key', primary_key)
  end

  def create_shard(source, table, strategy = 'row_count',
                   threshold = '1000000', prefix = 'shard')
    run('create-shard', '-s', source, '--table', table,
        '--strategy', strategy, '--threshold', threshold, '--prefix', prefix)
  end

  def detect_slow_query(source, db_type = 'mysql', threshold_ms = 1000)
    run('detect-slow-query', '-s', source, '-T', db_type,
        '--threshold-ms', threshold_ms.to_s)
  end

  def spanning_query(source, table, condition = '1=1', order_by = '', limit = 100, offset = 0)
    args = ['spanning-query', '-s', source, '--table', table,
            '--condition', condition, '-L', limit.to_s, '--offset', offset.to_s]
    args += ['--order-by', order_by] unless order_by.empty?
    run(*args)
  end

  def insert_log(source, message, table = 'app_logs', level = 'INFO', source_name = '')
    args = ['insert-log', '-s', source, '--table', table,
            '--level', level, '--message', message]
    args += ['--source-name', source_name] unless source_name.empty?
    run(*args)
  end

  def query_logs(source, table = 'app_logs', levels = '', keyword = '', limit = 100)
    args = ['query-logs', '-s', source, '--table', table, '-L', limit.to_s]
    args += ['--levels', levels] unless levels.empty?
    args += ['--keyword', keyword] unless keyword.empty?
    run(*args)
  end

  def detect_injection(input)
    run('detect-sql-injection', '-i', input)
  end

  def build_safe_sql(table, field, operator = '=', value = '')
    run('build-safe-sql', '--table', table, '--field', field,
        '--operator', operator, '--value', value)
  end
end

# =============================================================================
# 主函数
# =============================================================================

def print_result(title, result)
  puts "\n#{'=' * 60}"
  puts title
  puts '#' * 60
  if result.is_a?(Hash) || result.is_a?(Array)
    puts JSON.pretty_generate(result)
  else
    puts result
  end
end

use_cli = ARGV.include?('--cli')
binary_path = '/Users/Zhuanz/Desktop/website/composer/sqlmap/target/release/sqltool'

puts <<~EOS
  ╔════════════════════════════════════════════════════════════╗
  ║         SQLTool Ruby 完整调用示例 v0.4.1                ║
  ╚════════════════════════════════════════════════════════════╝
EOS

if use_cli
  puts "模式: CLI"
  puts "二进制: #{binary_path}\n\n"

  cli = SqlToolCLI.new(binary_path)

  puts '1. SQL注入检测...'
  print_result('检测结果', cli.detect_injection("' OR '1'='1"))

  puts '2. 安全SQL构建...'
  print_result('构建结果', cli.build_safe_sql('users', 'name', '=', "test'; DROP TABLE"))

  puts '3. 数据迁移...'
  print_result('迁移结果', cli.transfer(
    'mysql://root:pass@localhost:3306/source',
    'postgresql://postgres:pass@localhost:5432/target',
    'mysql', 'postgresql', 'users,orders', 5000
  ))

  puts '4. 数据库备份...'
  print_result('备份结果', cli.backup(
    'mysql://root:pass@localhost:3306/mydb',
    '/tmp/backup.sql', 'mysql', 'full', true
  ))

  puts '5. 数据对比...'
  print_result('对比结果', cli.compare_data(
    'mysql://root@localhost/db1',
    'mysql://root@localhost/db2',
    'users', 'id'
  ))
else
  puts "模式: HTTP API"
  puts "URL: http://localhost:8080\n\n"

  client = SqlToolClient.new('http://localhost:8080')

  begin
    puts '0. 健康检查...'
    print_result('健康状态', client.health_check)

    puts '1. SQL注入检测...'
    result = client.detect_injection("' OR '1'='1")
    print_result('检测结果', result)
    puts '⚠️ 警告: 检测到高风险SQL注入攻击!' if %w[High Critical].include?(result['risk_level'])

    puts '2. 安全SQL构建...'
    print_result('构建结果', client.build_safe_sql(table: 'users', field: 'email', operator: 'LIKE', value: '%@example.com'))

    puts '3. 数据迁移 (需要真实数据库连接)...'
    print_result('迁移结果', client.transfer(
      source: 'mysql://root:password@localhost:3306/source_db',
      target: 'postgresql://postgres:password@localhost:5432/target_db',
      tables: 'users,orders,products', batch_size: 5000
    ))

    puts '4. 数据库备份 (需要真实数据库连接)...'
    print_result('备份结果', client.backup(
      source: 'mysql://root:password@localhost:3306/mydb',
      output: '/tmp/backup_20240101.sql'
    ))

    puts '5. 数据对比 (需要真实数据库连接)...'
    print_result('对比结果', client.compare_data(
      source: 'mysql://root:password@localhost:3306/db1',
      target: 'mysql://root:password@localhost:3306/db2',
      table: 'users', primary_key: 'id'
    ))

    puts '6. 分库分表 (需要真实数据库连接)...'
    print_result('分片结果', client.create_shard(
      source: 'mysql://root:password@localhost:3306/mydb',
      table: 'orders', strategy: 'row_count'
    ))

    puts '7. 慢查询检测 (需要真实数据库连接)...'
    print_result('检测结果', client.detect_slow_query(
      source: 'mysql://root:password@localhost:3306/mydb', threshold_ms: 1000
    ))

    puts '8. 跨分片查询 (需要真实数据库连接)...'
    print_result('查询结果', client.spanning_query(
      source: 'mysql://root:password@localhost:3306/mydb',
      table: 'orders', condition: "status='pending'", order_by: 'created_at'
    ))

    puts '9. 插入日志 (需要真实数据库连接)...'
    print_result('插入结果', client.insert_log(
      source: 'mysql://root:password@localhost:3306/mydb',
      message: '用户登录成功', source_name: 'auth-service'
    ))

    puts '10. 查询日志 (需要真实数据库连接)...'
    print_result('查询结果', client.query_logs(
      source: 'mysql://root:password@localhost:3306/mydb',
      levels: 'ERROR,WARN', keyword: 'login'
    ))
  rescue => e
    puts "\n错误: #{e.message}"
    puts "\n请先启动 sqltool server:"
    puts '  sqltool server -p 8080 -s mysql://localhost/mydb'
    exit 1
  end
end

puts "\n#{'=' * 60}"
puts '示例执行完成!'
puts '#' * 60
