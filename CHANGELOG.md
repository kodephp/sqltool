# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-04-30

### Added
- **Enhanced SQL Injection Protection Module**
  - Added `InputValidator` for blacklist validation and input length control
  - Added `OutputEncoder` with multiple encoding schemes:
    - HTML encoding (XSS prevention)
    - HTML attribute encoding
    - URL encoding
    - JavaScript encoding
    - SQL encoding (single quote escaping)
    - JSON encoding
    - Hex encoding (for SQL LIKE queries)

- **Comprehensive Test Suite**
  - Added `full_feature_test.rs` with extensive tests covering:
    - Connection string parsing (MySQL, PostgreSQL, SQLite, Oracle)
    - SQL injection detection tests
    - SafeSqlBuilder tests
    - FieldSecurityValidator tests
    - OperationResult tests
    - BatchConfig tests
    - ProgressTracker tests
    - Database configuration tests
    - DataFilter tests
    - Oracle type conversion tests
    - Boundary condition tests
    - RiskLevel comparison tests

### Enhanced
- Improved SQL injection detection with additional pattern matching
- Enhanced field validation with SQL keyword detection
- Added progress tracking for batch operations

### Fixed
- Fixed `BatchConfig` field names in tests
- Fixed `ProgressTracker` async method usage
- Fixed Oracle type converter exports

## [0.3.0] - 2026-04-29

### Added
- HTTP API server mode with Axum framework
- Multi-language SDK examples (Python, Node.js, Go, PHP, Ruby, Java, C#)
- Oracle database support with type converters
- Enhanced SQL injection detection (`SqlInjectionDetector`)
- Safe SQL builder (`SafeSqlBuilder`)
- Field security validator (`FieldSecurityValidator`)

### Features
- Data transfer between databases
- Database backup and restore
- Data comparison
- Automatic sharding
- Slow query detection
- Log table management

## [0.2.0] - 2026-04-28

### Added
- Basic database migration support
- SQLite, PostgreSQL, MySQL support
- CLI interface with clap

## [0.1.0] - 2026-04-27

### Added
- Initial release
- Basic project structure
