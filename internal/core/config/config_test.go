package config

import (
	"os"
	"testing"
	"time"
)

func TestHMACSecrets(t *testing.T) {
	// Clean environment
	os.Unsetenv("TK_HMAC_SECRET")
	os.Unsetenv("TK_HMAC_SECRET_1")
	os.Unsetenv("TK_HMAC_SECRET_2")

	t.Run("single secret", func(t *testing.T) {
		os.Setenv("TK_HMAC_SECRET", "0123456789abcdef0123456789abcdef:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		defer os.Unsetenv("TK_HMAC_SECRET")

		secrets, err := HMACSecrets()
		if err != nil {
			t.Fatalf("HMACSecrets failed: %v", err)
		}
		if len(secrets) != 1 {
			t.Errorf("expected 1 secret, got %d", len(secrets))
		}
		if _, ok := secrets["0123456789abcdef0123456789abcdef"]; !ok {
			t.Errorf("secret_id not found in map")
		}
	})

	t.Run("multiple numbered secrets", func(t *testing.T) {
		os.Setenv("TK_HMAC_SECRET_1", "0123456789abcdef0123456789abcdef:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		os.Setenv("TK_HMAC_SECRET_2", "fedcba9876543210fedcba9876543210:YW5vdGhlcnNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		defer os.Unsetenv("TK_HMAC_SECRET_1")
		defer os.Unsetenv("TK_HMAC_SECRET_2")

		secrets, err := HMACSecrets()
		if err != nil {
			t.Fatalf("HMACSecrets failed: %v", err)
		}
		if len(secrets) != 2 {
			t.Errorf("expected 2 secrets, got %d", len(secrets))
		}
	})

	t.Run("invalid format", func(t *testing.T) {
		os.Setenv("TK_HMAC_SECRET", "invalid_format")
		defer os.Unsetenv("TK_HMAC_SECRET")

		_, err := HMACSecrets()
		if err == nil {
			t.Error("expected error for invalid format")
		}
	})

	t.Run("invalid secret_id length", func(t *testing.T) {
		os.Setenv("TK_HMAC_SECRET", "short:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		defer os.Unsetenv("TK_HMAC_SECRET")

		_, err := HMACSecrets()
		if err == nil {
			t.Error("expected error for short secret_id")
		}
	})

	t.Run("non-hex secret_id", func(t *testing.T) {
		os.Setenv("TK_HMAC_SECRET", "0123456789abcdefGHIJKLMNOPQRSTUV:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		defer os.Unsetenv("TK_HMAC_SECRET")

		_, err := HMACSecrets()
		if err == nil {
			t.Error("expected error for non-hex secret_id")
		}
	})

	t.Run("duplicate secret_id in numbered secrets", func(t *testing.T) {
		os.Setenv("TK_HMAC_SECRET_1", "0123456789abcdef0123456789abcdef:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		os.Setenv("TK_HMAC_SECRET_2", "0123456789abcdef0123456789abcdef:YW5vdGhlcnNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		defer os.Unsetenv("TK_HMAC_SECRET_1")
		defer os.Unsetenv("TK_HMAC_SECRET_2")

		_, err := HMACSecrets()
		if err == nil {
			t.Error("expected error for duplicate secret_id")
		}
	})

	t.Run("duplicate secret_id between single and numbered", func(t *testing.T) {
		os.Setenv("TK_HMAC_SECRET", "0123456789abcdef0123456789abcdef:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		os.Setenv("TK_HMAC_SECRET_1", "0123456789abcdef0123456789abcdef:YW5vdGhlcnNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		defer os.Unsetenv("TK_HMAC_SECRET")
		defer os.Unsetenv("TK_HMAC_SECRET_1")

		_, err := HMACSecrets()
		if err == nil {
			t.Error("expected error for duplicate secret_id between TK_HMAC_SECRET and TK_HMAC_SECRET_1")
		}
	})
}

func TestLoadConfig(t *testing.T) {
	// Clean environment
	os.Unsetenv("TK_SENSOR_API_HOST")
	os.Unsetenv("TK_SENSOR_API_PORT")

	t.Run("defaults", func(t *testing.T) {
		cfg, err := LoadConfig("")
		if err != nil {
			t.Fatalf("LoadConfig failed: %v", err)
		}
		if cfg.Host != "0.0.0.0" {
			t.Errorf("expected host 0.0.0.0, got %s", cfg.Host)
		}
		if cfg.Port != 50051 {
			t.Errorf("expected port 50051, got %d", cfg.Port)
		}
		if cfg.MaxConnections != 1000 {
			t.Errorf("expected max_connections 1000, got %d", cfg.MaxConnections)
		}
		if cfg.RequestTimeout != 30*time.Second {
			t.Errorf("expected timeout 30s, got %v", cfg.RequestTimeout)
		}
		if cfg.MaxBatchSize != 1000 {
			t.Errorf("expected max_batch_size 1000, got %d", cfg.MaxBatchSize)
		}
		if cfg.DataDir != "./data" {
			t.Errorf("expected data_dir ./data, got %s", cfg.DataDir)
		}
	})

	t.Run("environment override", func(t *testing.T) {
		os.Setenv("TK_SENSOR_API_PORT", "9999")
		os.Setenv("TK_SENSOR_API_HOST", "127.0.0.1")
		defer os.Unsetenv("TK_SENSOR_API_PORT")
		defer os.Unsetenv("TK_SENSOR_API_HOST")

		cfg, err := LoadConfig("")
		if err != nil {
			t.Fatalf("LoadConfig failed: %v", err)
		}
		if cfg.Port != 9999 {
			t.Errorf("expected port 9999, got %d", cfg.Port)
		}
		if cfg.Host != "127.0.0.1" {
			t.Errorf("expected host 127.0.0.1, got %s", cfg.Host)
		}
	})

	t.Run("invalid port range", func(t *testing.T) {
		os.Setenv("TK_SENSOR_API_PORT", "70000")
		defer os.Unsetenv("TK_SENSOR_API_PORT")

		_, err := LoadConfig("")
		if err == nil {
			t.Error("expected error for port > 65535")
		}
	})

	t.Run("invalid negative values", func(t *testing.T) {
		os.Setenv("TK_SENSOR_API_MAX_CONNECTIONS", "-1")
		defer os.Unsetenv("TK_SENSOR_API_MAX_CONNECTIONS")

		_, err := LoadConfig("")
		if err == nil {
			t.Error("expected error for negative max_connections")
		}
	})
}

func TestParseHMACSecret(t *testing.T) {
	t.Run("valid base64", func(t *testing.T) {
		secret, err := ParseHMACSecret("dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		if err != nil {
			t.Fatalf("ParseHMACSecret failed: %v", err)
		}
		if len(secret) < 32 {
			t.Errorf("secret too short: %d bytes", len(secret))
		}
	})

	t.Run("invalid base64", func(t *testing.T) {
		_, err := ParseHMACSecret("not-valid-base64!!!")
		if err == nil {
			t.Error("expected error for invalid base64")
		}
	})

	t.Run("secret too short", func(t *testing.T) {
		_, err := ParseHMACSecret("c2hvcnQ=") // "short" in base64
		if err == nil {
			t.Error("expected error for secret < 32 bytes")
		}
	})
}

func TestParseHMACSecretWithID(t *testing.T) {
	t.Run("valid format", func(t *testing.T) {
		secretID, secret, err := ParseHMACSecretWithID("0123456789abcdef0123456789abcdef:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		if err != nil {
			t.Fatalf("ParseHMACSecretWithID failed: %v", err)
		}
		if secretID != "0123456789abcdef0123456789abcdef" {
			t.Errorf("unexpected secret_id: %s", secretID)
		}
		if len(secret) == 0 {
			t.Error("secret should not be empty")
		}
	})

	t.Run("missing colon", func(t *testing.T) {
		_, _, err := ParseHMACSecretWithID("0123456789abcdef0123456789abcdef")
		if err == nil {
			t.Error("expected error for missing colon")
		}
	})

	t.Run("invalid secret_id length", func(t *testing.T) {
		_, _, err := ParseHMACSecretWithID("tooshort:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		if err == nil {
			t.Error("expected error for short secret_id")
		}
	})

	t.Run("non-hex chars in secret_id", func(t *testing.T) {
		_, _, err := ParseHMACSecretWithID("0123456789abcdefGHIJKLMNOPQRSTUV:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		if err == nil {
			t.Error("expected error for non-hex secret_id")
		}
	})
}
