#!/usr/bin/env ruby
# SQLTool Ruby 调用示例
#
# 安装依赖:
#   gem install json (通常已内置)
#
# 使用方法:
#   ruby sqltool_demo.rb           # HTTP API 模式
#   ruby sqltool_demo.rb --cli    # CLI 模式

require 'json'
require 'open-uri'
require 'net/http'

class SqlToolClient
  def initialize(base_url = 'http://localhost:8080')
    @base_url = base_url.chomp('/')
  end

  def health_check
    uri = URI("#{@base_url}/api/health")
    Net::HTTP.get(uri)
  end

  def detect_injection(input)
    uri = URI("#{@base_url}/api/security/detect-injection")
    req = Net::HTTP::Post.new(uri, 'Content-Type' => 'application/json')
    req.body = { input: input }.to_json
    Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req).body }
  end

  def build_safe_sql(table, field, operator, value)
    uri = URI("#{@base_url}/api/security/build-safe-sql")
    req = Net::HTTP::Post.new(uri, 'Content-Type' => 'application/json')
    req.body = { table: table, field: field, operator: operator, value: value }.to_json
    Net::HTTP.start(uri.hostname, uri.port) { |http| http.request(req).body }
  end
end

class SqlToolCLI
  def run(*args)
    cmd = ['sqltool'] + args
    IO.popen(cmd, 'r') { |io| io.read }
  end

  def detect_injection(input)
    run('detect-injection', '--input', input)
  end

  def build_safe_sql(table, field, operator, value)
    run('build-safe-sql', '--table', table, '--field', field, '--operator', operator, '--value', value)
  end
end

def print_result(title, result)
  puts "\n#{'=' * 50}"
  puts title
  puts '#' * 50
  puts JSON.pretty_generate(JSON.parse(result))
rescue JSON::ParserError
  puts result
end

use_cli = ARGV[0] == '--cli'

puts "
╔══════════════════════════════════════════════════╗
║         SQLTool Ruby 调用示例                      ║
╚══════════════════════════════════════════════════╝
"

if use_cli
  puts "模式: CLI (不需要启动 server)\n"
  cli = SqlToolCLI.new
  print_result('1. SQL注入检测', cli.detect_injection("' OR '1'='1"))
  print_result('2. 构建安全SQL', cli.build_safe_sql('users', 'name', '=', "test'; DROP TABLE"))
else
  puts "模式: HTTP API (需要启动 sqltool server)\n"
  client = SqlToolClient.new

  begin
    print_result('0. 健康检查', client.health_check)
    print_result('1. SQL注入检测 - 恶意输入', client.detect_injection("' OR '1'='1"))
    print_result('2. SQL注入检测 - 正常输入', client.detect_injection('normal_input'))
    print_result('3. 构建安全SQL', client.build_safe_sql('users', 'name', '=', "test'; DROP TABLE"))
  rescue => e
    puts "\n错误: 无法连接到 http://localhost:8080"
    puts "请先启动 sqltool server:"
    puts "  sqltool server -p 8080 -s mysql://localhost/mydb"
    exit 1
  end
end

puts "\n#{'=' * 50}"
puts "示例执行完成!"
puts '#' * 50
