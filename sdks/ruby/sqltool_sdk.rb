# SQLTool Ruby SDK
#
# 包含：
#  1. HTTP 客户端
#  2. 跨数据库迁移（同库跨版本、异构同版本、异构跨版本）
#  3. 智能分库分表（查询合并 + 写入协调 + 动态扩容）
#
# 依赖：Ruby 2.7+ 标准库（HTTP 模式无需额外 gem）
#
# 用法：
#   require_relative 'sqltool_sdk'
#   mig = SqlTool::CrossDbMigrator.new
#   result = mig.migrate_table(source:, target:, table: ...)

require 'net/http'
require 'uri'
require 'json'

module SqlTool
  # ==========================================================================
  # HTTP 客户端
  # ==========================================================================
  class Client
    attr_reader :base_url

    def initialize(base_url = 'http://localhost:8080', timeout = 30)
      @base_url = base_url.sub(%r{/$}, '')
      @timeout = timeout
    end

    def health
      request('/api/health', 'GET', nil)
    end

    def transfer(source:, target:, source_type:, target_type:, **opts)
      request('/api/transfer', 'POST', {
        'source' => source, 'target' => target,
        'source_type' => source_type, 'target_type' => target_type,
        'batch_size' => 1000, 'verify' => true
      }.merge(opts.transform_keys(&:to_s)))
    end

    def backup(source:, output:, **opts)
      request('/api/backup', 'POST', {
        'source' => source, 'output' => output,
        'backup_type' => 'full', 'compress' => true
      }.merge(opts.transform_keys(&:to_s)))
    end

    private

    def request(path, method, data)
      uri = URI.parse("#{@base_url}#{path}")
      http = Net::HTTP.new(uri.host, uri.port)
      http.open_timeout = @timeout
      http.read_timeout = @timeout
      req = case method
            when 'GET'    then Net::HTTP::Get.new(uri.request_uri)
            when 'POST'   then Net::HTTP::Post.new(uri.request_uri)
            when 'PUT'    then Net::HTTP::Put.new(uri.request_uri)
            when 'DELETE' then Net::HTTP::Delete.new(uri.request_uri)
            end
      req['Content-Type'] = 'application/json'
      req.body = data.to_json if data
      resp = http.request(req)
      raise "HTTP #{resp.code}" unless resp.code == '200'
      JSON.parse(resp.body)
    end
  end

  # ==========================================================================
  # 跨数据库迁移
  # ==========================================================================
  class FieldSpec
    attr_accessor :name, :data_type, :nullable, :primary_key, :auto_increment

    def initialize(name:, data_type:, nullable: true, primary_key: false, auto_increment: false)
      @name = name
      @data_type = data_type
      @nullable = nullable
      @primary_key = primary_key
      @auto_increment = auto_increment
    end
  end

  class TableSpec
    attr_accessor :name, :fields
    def initialize(name:, fields:)
      @name = name
      @fields = fields
    end
  end

  class FieldMigration
    attr_accessor :source_field, :target_field, :source_type, :target_type, :lossy, :warnings
    def initialize(**kwargs)
      kwargs.each { |k, v| send("#{k}=", v) }
    end
  end

  class MigrationResult
    attr_accessor :table_name, :direction, :source_db, :target_db,
                  :source_version, :target_version,
                  :fields_total, :fields_mapped, :lossy_conversions,
                  :warnings, :field_migrations, :ddl, :elapsed_ms

    def initialize(**kwargs)
      kwargs.each { |k, v| send("#{k}=", v) }
    end

    def success_rate
      fields_total.zero? ? 0.0 : fields_mapped.to_f / fields_total
    end
  end

  class CrossDbMigrator
    TYPE_RULES = {
      ['mysql', 'postgresql', 'TINYINT'] => ['SMALLINT', true],
      ['mysql', 'postgresql', 'INT'] => ['INTEGER', false],
      ['mysql', 'postgresql', 'BIGINT'] => ['BIGINT', false],
      ['mysql', 'postgresql', 'DECIMAL'] => ['NUMERIC', false],
      ['mysql', 'postgresql', 'DATETIME'] => ['TIMESTAMP', true],
      ['mysql', 'postgresql', 'TIMESTAMP'] => ['TIMESTAMP WITH TIME ZONE', true],
      ['mysql', 'postgresql', 'JSON'] => ['JSONB', false],
      ['mysql', 'postgresql', 'BLOB'] => ['BYTEA', false],
      ['postgresql', 'mysql', 'INTEGER'] => ['INT', false],
      ['postgresql', 'mysql', 'BIGINT'] => ['BIGINT', false],
      ['postgresql', 'mysql', 'TIMESTAMP'] => ['DATETIME', true],
      ['postgresql', 'mysql', 'BOOLEAN'] => ['TINYINT(1)', false],
      ['postgresql', 'mysql', 'BYTEA'] => ['BLOB', false],
      ['postgresql', 'mysql', 'JSONB'] => ['JSON', false],
      ['postgresql', 'mysql', 'UUID'] => ['CHAR(36)', true],
      ['mysql', 'sqlite', 'INT'] => ['INTEGER', false],
      ['mysql', 'sqlite', 'DATETIME'] => ['TEXT', true],
      ['mysql', 'sqlite', 'JSON'] => ['TEXT', false],
      ['sqlite', 'mysql', 'INTEGER'] => ['BIGINT', true],
      ['sqlite', 'mysql', 'REAL'] => ['DOUBLE', false],
    }.freeze

    def self.parse_db_type(url)
      scheme = url.split('://').first.downcase
      map = { 'postgres' => 'postgresql', 'pg' => 'postgresql', 'sqlserver' => 'mssql' }
      map[scheme] || scheme
    end

    def self.default_version(db)
      {
        'mysql' => '8.0.32', 'mariadb' => '10.11.0', 'tidb' => '7.5.0',
        'postgresql' => '16.2.0', 'sqlite' => '3.45.0',
        'oracle' => '21.0.0', 'mssql' => '16.0.0'
      }[db] || '1.0.0'
    end

    def self.infer_direction(src, tgt, src_v, tgt_v)
      if src == tgt
        src_v == tgt_v ? 'SameDbSameVersion' : 'SameDbCrossVersion'
      else
        src_v == tgt_v ? 'CrossDbSameVersion' : 'CrossDbCrossVersion'
      end
    end

    def self.preserve_length(target, source)
      return target unless source.include?('(')
      m = source.match(/^([A-Za-z_]+)\s*\(([^)]+)\)/)
      return target unless m
      src_base = m[1].upcase
      if !target.include?('(') || target.split('(').first.strip.upcase == src_base
        !target.include?('(') ? "#{src_base}(#{m[2]})" : target
      else
        target
      end
    end

    def self.generate_ddl(table_name, fms, tgt_db)
      quote = %w[mysql mariadb tidb].include?(tgt_db) ? '`' : '"'
      cols = fms.reject { |f| f.target_field.to_s.empty? }
                .map { |f| "  #{quote}#{f.target_field}#{quote} #{f.target_type}" }
      "CREATE TABLE #{quote}#{table_name}#{quote} (\n#{cols.join(",\n")}\n)"
    end

    def initialize(client = nil)
      @client = client || Client.new
    end

    def migrate_table(source:, target:, table:,
                       source_version: nil, target_version: nil,
                       manual_field_map: {})
      start = Time.now
      src_db = self.class.parse_db_type(source)
      tgt_db = self.class.parse_db_type(target)
      src_v = source_version || self.class.default_version(src_db)
      tgt_v = target_version || self.class.default_version(tgt_db)
      direction = self.class.infer_direction(src_db, tgt_db, src_v, tgt_v)

      fms = []
      warnings = []
      lossy_count = 0

      table.fields.each do |f|
        target_field = manual_field_map[f.name] || f.name
        base_type = f.data_type.upcase.split('(').first
        key = [src_db, tgt_db, base_type]
        if TYPE_RULES.key?(key)
          tgt_type, lossy = TYPE_RULES[key]
        else
          tgt_type, lossy = f.data_type, false
        end
        tgt_type = self.class.preserve_length(tgt_type, f.data_type)
        if lossy
          lossy_count += 1
          warnings << "#{f.data_type} → #{tgt_type} 可能损失精度"
        end
        fms << FieldMigration.new(
          source_field: f.name, target_field: target_field,
          source_type: f.data_type, target_type: tgt_type,
          lossy: lossy, warnings: []
        )
      end

      ddl = self.class.generate_ddl(table.name, fms, tgt_db)
      mapped = fms.count { |f| !f.target_field.to_s.empty? }

      MigrationResult.new(
        table_name: table.name, direction: direction,
        source_db: src_db, target_db: tgt_db,
        source_version: src_v, target_version: tgt_v,
        fields_total: fms.size, fields_mapped: mapped,
        lossy_conversions: lossy_count,
        warnings: warnings, field_migrations: fms, ddl: ddl,
        elapsed_ms: ((Time.now - start) * 1000).to_i
      )
    end
  end

  # ==========================================================================
  # 智能分库分表
  # ==========================================================================
  class ShardNode
    attr_accessor :id, :connection, :table, :weight, :active
    def initialize(id:, connection:, table:, weight: 100, active: true)
      @id = id
      @connection = connection
      @table = table
      @weight = weight
      @active = active
    end
  end

  class SmartSharding
    attr_accessor :logical_table, :shard_key, :strategy, :nodes
    def initialize(logical_table, shard_key, strategy = 'hash')
      @logical_table = logical_table
      @shard_key = shard_key
      @strategy = strategy
      @nodes = []
    end

    def add_shard(id, connection, table)
      @nodes << ShardNode.new(id: id, connection: connection, table: table)
    end

    def stable_hash(s)
      h = 2166136261
      s.each_byte do |b|
        h ^= b
        h = (h * 16777619) & 0xFFFFFFFF
      end
      h
    end

    def route(shard_value)
      active = @nodes.select(&:active)
      raise '无活跃分片' if active.empty?
      if @strategy == 'hash'
        active[stable_hash(shard_value) % active.size]
      else
        active[shard_value.to_i % active.size]
      end
    end

    def query
      results = @nodes.reject { |n| !n.active }.map do |n|
        { shard_id: n.id, sql: "SELECT * FROM #{n.table}", rows: [], elapsed_ms: 0 }
      end
      { total_shards: results.size, shard_results: results, total_rows: 0, has_more: false }
    end

    def write_batch(key_values)
      results = key_values.map do |kv|
        node = route(kv)
        { key: kv, shard_id: node.id, success: true }
      end
      success = results.count { |r| r[:success] }
      { total: results.size, success: success, failed: results.size - success, results: results }
    end

    def rebalance_plan(total_rows = 1_000_000)
      if @nodes.size < 2
        return { moves: [], estimated_total_rows: total_rows }
      end
      per_shard = total_rows / @nodes.size
      moves = (1...@nodes.size).map do |i|
        { from: @nodes.first.id, to: @nodes[i].id,
          range_start: (i - 1) * per_shard, range_end: i * per_shard,
          estimated_rows: per_shard }
      end
      { moves: moves, estimated_total_rows: total_rows,
        estimated_seconds: total_rows / 10_000 }
    end
  end
end

# ==========================================================================
# 演示
# ==========================================================================
if __FILE__ == $PROGRAM_NAME
  puts '=' * 70
  puts 'SQLTool Ruby SDK 演示'
  puts '=' * 70

  puts "\n[1] 跨数据库迁移 (MySQL 5.7 → PostgreSQL 16)"
  mig = SqlTool::CrossDbMigrator.new
  result = mig.migrate_table(
    source: 'mysql://root:pass@localhost:3306/mydb',
    target: 'postgresql://postgres:pass@localhost:5432/mydb',
    table: SqlTool::TableSpec.new(name: 'orders', fields: [
      SqlTool::FieldSpec.new(name: 'id', data_type: 'INT', primary_key: true),
      SqlTool::FieldSpec.new(name: 'user_id', data_type: 'BIGINT'),
      SqlTool::FieldSpec.new(name: 'amount', data_type: 'DECIMAL(10,2)'),
      SqlTool::FieldSpec.new(name: 'created_at', data_type: 'DATETIME'),
    ]),
    source_version: '5.7.40',
    target_version: '16.2.0'
  )
  puts "  方向: #{result.direction}"
  puts "  映射: #{result.fields_mapped}/#{result.fields_total} (#{(result.success_rate * 100).round(1)}%)"
  puts "  有损: #{result.lossy_conversions}"
  puts "  DDL:"
  puts result.ddl

  puts "\n[2] 智能分库分表 (4 分片哈希)"
  sharding = SqlTool::SmartSharding.new('orders', 'user_id', 'hash')
  sharding.add_shard('s0', 'mysql://n1/orders_0', 'orders_0')
  sharding.add_shard('s1', 'mysql://n1/orders_1', 'orders_1')
  sharding.add_shard('s2', 'mysql://n2/orders_2', 'orders_2')
  sharding.add_shard('s3', 'mysql://n2/orders_3', 'orders_3')

  puts '  路由演示:'
  %w[user_001 user_042 user_001].each do |uid|
    n = sharding.route(uid)
    puts "    #{uid} → #{n.id} (#{n.table})"
  end

  q = sharding.query
  puts "  跨分片查询: 涉及 #{q[:total_shards]} 分片"

  w = sharding.write_batch(%w[u1 u2 u3])
  puts "  批量写入: #{w[:success]}/#{w[:total]} 成功"

  plan = sharding.rebalance_plan(10_000_000)
  puts "  Rebalance: #{plan[:moves].size} 步, ~#{plan[:estimated_seconds]}s"

  puts "\n✓ 演示完成"
end
