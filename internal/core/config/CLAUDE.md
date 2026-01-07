# internal/core/config/

Configuration loading and validation for TrapperKeeper services.

## Index

| File                    | Contents (WHAT)                                          | Read When (WHEN)                                      |
| ----------------------- | -------------------------------------------------------- | ----------------------------------------------------- |
| `config.go`             | SensorAPIConfig struct, HMAC secret parsing              | Adding config fields, modifying defaults              |
| `viper.go`              | Viper integration, precedence (CLI > env > file)         | Debugging config loading, adding new services         |
| `config_test.go`        | Unit tests for config parsing and validation             | Writing new config tests, debugging parsing issues    |
| `acceptance_test.go`    | Acceptance tests for environment variable loading        | Validating end-to-end config behavior                 |
