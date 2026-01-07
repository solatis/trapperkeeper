package config

import (
	"fmt"
	"strings"

	"github.com/spf13/viper"
)

// LoadConfig loads configuration from file using viper.
// CLI flags > environment > config file > defaults precedence.
func LoadConfig(configPath string) (*SensorAPIConfig, error) {
	v := viper.New()

	// Set defaults matching DefaultSensorAPIConfig
	v.SetDefault("sensor_api.host", "0.0.0.0")
	v.SetDefault("sensor_api.port", 50051)
	v.SetDefault("sensor_api.max_connections", 1000)
	v.SetDefault("sensor_api.request_timeout", "30s")
	v.SetDefault("sensor_api.max_batch_size", 1000)
	v.SetDefault("sensor_api.data_dir", "./data")

	// Bind environment variables with TK_ prefix
	v.SetEnvPrefix("TK")
	v.SetEnvKeyReplacer(strings.NewReplacer(".", "_"))
	v.AutomaticEnv()

	// Load config file if provided
	if configPath != "" {
		v.SetConfigFile(configPath)
		if err := v.ReadInConfig(); err != nil {
			return nil, fmt.Errorf("failed to read config file: %w", err)
		}
	}

	// Security check: reject secrets in config files
	// Secrets must be environment-only per 12-factor principles
	if err := validateNoSecretsInConfig(v); err != nil {
		return nil, err
	}

	cfg := &SensorAPIConfig{
		Host:           v.GetString("sensor_api.host"),
		Port:           v.GetInt("sensor_api.port"),
		MaxConnections: v.GetInt("sensor_api.max_connections"),
		RequestTimeout: v.GetDuration("sensor_api.request_timeout"),
		MaxBatchSize:   v.GetInt("sensor_api.max_batch_size"),
		DataDir:        v.GetString("sensor_api.data_dir"),
	}

	if err := validateConfig(cfg); err != nil {
		return nil, err
	}

	return cfg, nil
}

// validateConfig checks port range, positive values for connections, timeout, batch size.
func validateConfig(cfg *SensorAPIConfig) error {
	if cfg.Port <= 0 || cfg.Port > 65535 {
		return fmt.Errorf("port must be between 1 and 65535, got %d", cfg.Port)
	}
	if cfg.MaxConnections <= 0 {
		return fmt.Errorf("max_connections must be positive, got %d", cfg.MaxConnections)
	}
	if cfg.RequestTimeout <= 0 {
		return fmt.Errorf("request_timeout must be positive, got %v", cfg.RequestTimeout)
	}
	if cfg.MaxBatchSize <= 0 {
		return fmt.Errorf("max_batch_size must be positive, got %d", cfg.MaxBatchSize)
	}
	return nil
}

// validateNoSecretsInConfig enforces environment-only secrets (12-factor principle).
func validateNoSecretsInConfig(v *viper.Viper) error {
	if v.IsSet("hmac_secret") || v.IsSet("sensor_api.hmac_secret") {
		return fmt.Errorf("HMAC secrets not allowed in config files (use TK_HMAC_SECRET environment variable)")
	}
	return nil
}
