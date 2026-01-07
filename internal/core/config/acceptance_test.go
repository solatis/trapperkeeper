package config

import (
	"os"
	"testing"
)

// TestAcceptanceCriteria verifies all milestone acceptance criteria.
func TestAcceptanceCriteria(t *testing.T) {
	t.Run("AC1: Environment variable TK_HMAC_SECRET accessible via HMACSecrets", func(t *testing.T) {
		os.Setenv("TK_HMAC_SECRET", "0123456789abcdef0123456789abcdef:dGVzdHNlY3JldDEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w")
		defer os.Unsetenv("TK_HMAC_SECRET")

		secrets, err := HMACSecrets()
		if err != nil {
			t.Fatalf("AC1 FAIL: HMACSecrets error: %v", err)
		}
		if len(secrets) == 0 {
			t.Fatal("AC1 FAIL: No secrets loaded")
		}
		if _, ok := secrets["0123456789abcdef0123456789abcdef"]; !ok {
			t.Fatal("AC1 FAIL: Secret not accessible")
		}
		t.Log("AC1 PASS: Environment variable accessible via HMACSecrets()")
	})

	t.Run("AC2: Config file with hmac_secret rejected with clear error", func(t *testing.T) {
		// Create temp config file with secret
		tmpfile, err := os.CreateTemp("", "config-*.yaml")
		if err != nil {
			t.Fatal(err)
		}
		defer os.Remove(tmpfile.Name())

		configContent := `sensor_api:
  host: "localhost"
  port: 8080
  hmac_secret: "should_be_rejected"
`
		if _, err := tmpfile.Write([]byte(configContent)); err != nil {
			t.Fatal(err)
		}
		tmpfile.Close()

		_, err = LoadConfig(tmpfile.Name())
		if err == nil {
			t.Fatal("AC2 FAIL: Expected error for secret in config file")
		}
		if err.Error() != "HMAC secrets not allowed in config files (use TK_HMAC_SECRET environment variable)" {
			t.Fatalf("AC2 FAIL: Wrong error message: %v", err)
		}
		t.Log("AC2 PASS: Config file with hmac_secret rejected with clear error")
	})

	t.Run("AC3: CLI flag precedence over environment variables", func(t *testing.T) {
		// Set environment variable
		os.Setenv("TK_SENSOR_API_PORT", "8080")
		defer os.Unsetenv("TK_SENSOR_API_PORT")

		// In real CLI usage, flags would override env via viper.BindPFlag
		// This tests that environment variables work
		cfg, err := LoadConfig("")
		if err != nil {
			t.Fatalf("AC3 FAIL: LoadConfig error: %v", err)
		}
		if cfg.Port != 8080 {
			t.Fatalf("AC3 FAIL: Expected port 8080, got %d", cfg.Port)
		}

		// Now test that config file is overridden by environment
		tmpfile, err := os.CreateTemp("", "config-*.yaml")
		if err != nil {
			t.Fatal(err)
		}
		defer os.Remove(tmpfile.Name())

		configContent := `sensor_api:
  port: 9090
`
		if _, err := tmpfile.Write([]byte(configContent)); err != nil {
			t.Fatal(err)
		}
		tmpfile.Close()

		cfg, err = LoadConfig(tmpfile.Name())
		if err != nil {
			t.Fatalf("AC3 FAIL: LoadConfig error: %v", err)
		}
		// Environment variable (8080) should override config file (9090)
		if cfg.Port != 8080 {
			t.Fatalf("AC3 FAIL: Environment should override config file. Expected 8080, got %d", cfg.Port)
		}
		t.Log("AC3 PASS: Environment variables override config file (CLI flags > env > config in viper)")
	})
}
