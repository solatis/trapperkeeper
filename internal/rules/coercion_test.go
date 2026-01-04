package rules

import (
	"math"
	"testing"

	"github.com/solatis/trapperkeeper/internal/types"
)

func TestCoerce(t *testing.T) {
	tests := []struct {
		name      string
		value     any
		fieldType FieldType
		wantValue any
		wantNull  bool
		wantErr   error
	}{
		// NUMERIC type tests
		{
			name:      "numeric: string to float64",
			value:     "25",
			fieldType: FieldTypeNumeric,
			wantValue: 25.0,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: float64 passthrough",
			value:     42.5,
			fieldType: FieldTypeNumeric,
			wantValue: 42.5,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: int to float64",
			value:     100,
			fieldType: FieldTypeNumeric,
			wantValue: 100.0,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: int64 to float64",
			value:     int64(999),
			fieldType: FieldTypeNumeric,
			wantValue: 999.0,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: string with whitespace",
			value:     "  42  ",
			fieldType: FieldTypeNumeric,
			wantValue: 42.0,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: decimal string",
			value:     "3.14159",
			fieldType: FieldTypeNumeric,
			wantValue: 3.14159,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: negative number",
			value:     "-100",
			fieldType: FieldTypeNumeric,
			wantValue: -100.0,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: scientific notation",
			value:     "1e10",
			fieldType: FieldTypeNumeric,
			wantValue: 1e10,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: very large number",
			value:     "1.7976931348623157e+308",
			fieldType: FieldTypeNumeric,
			wantValue: 1.7976931348623157e+308,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "numeric: non-numeric string fails",
			value:     "abc",
			fieldType: FieldTypeNumeric,
			wantValue: nil,
			wantNull:  false,
			wantErr:   types.ErrCoercionFailed,
		},
		{
			name:      "numeric: empty string fails",
			value:     "",
			fieldType: FieldTypeNumeric,
			wantValue: nil,
			wantNull:  false,
			wantErr:   types.ErrCoercionFailed,
		},
		{
			name:      "numeric: whitespace-only string fails",
			value:     "   ",
			fieldType: FieldTypeNumeric,
			wantValue: nil,
			wantNull:  false,
			wantErr:   types.ErrCoercionFailed,
		},
		{
			name:      "numeric: boolean fails (strict mode)",
			value:     true,
			fieldType: FieldTypeNumeric,
			wantValue: nil,
			wantNull:  false,
			wantErr:   types.ErrCoercionFailed,
		},
		{
			name:      "numeric: nil returns null",
			value:     nil,
			fieldType: FieldTypeNumeric,
			wantValue: nil,
			wantNull:  true,
			wantErr:   nil,
		},

		// TEXT type tests
		{
			name:      "text: string passthrough",
			value:     "hello",
			fieldType: FieldTypeText,
			wantValue: "hello",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "text: int to string",
			value:     100,
			fieldType: FieldTypeText,
			wantValue: "100",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "text: int64 to string",
			value:     int64(999),
			fieldType: FieldTypeText,
			wantValue: "999",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "text: float64 to string",
			value:     3.14,
			fieldType: FieldTypeText,
			wantValue: "3.14",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "text: boolean true to string",
			value:     true,
			fieldType: FieldTypeText,
			wantValue: "true",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "text: boolean false to string",
			value:     false,
			fieldType: FieldTypeText,
			wantValue: "false",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "text: empty string passthrough",
			value:     "",
			fieldType: FieldTypeText,
			wantValue: "",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "text: nil returns null",
			value:     nil,
			fieldType: FieldTypeText,
			wantValue: nil,
			wantNull:  true,
			wantErr:   nil,
		},

		// BOOLEAN type tests
		{
			name:      "boolean: true passthrough",
			value:     true,
			fieldType: FieldTypeBoolean,
			wantValue: true,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "boolean: false passthrough",
			value:     false,
			fieldType: FieldTypeBoolean,
			wantValue: false,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "boolean: string fails (strict mode)",
			value:     "true",
			fieldType: FieldTypeBoolean,
			wantValue: nil,
			wantNull:  false,
			wantErr:   types.ErrCoercionFailed,
		},
		{
			name:      "boolean: int fails (strict mode)",
			value:     1,
			fieldType: FieldTypeBoolean,
			wantValue: nil,
			wantNull:  false,
			wantErr:   types.ErrCoercionFailed,
		},
		{
			name:      "boolean: nil returns null",
			value:     nil,
			fieldType: FieldTypeBoolean,
			wantValue: nil,
			wantNull:  true,
			wantErr:   nil,
		},

		// ANY type tests
		{
			name:      "any: string preserved",
			value:     "hello",
			fieldType: FieldTypeAny,
			wantValue: "hello",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "any: int preserved",
			value:     42,
			fieldType: FieldTypeAny,
			wantValue: 42,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "any: float64 preserved",
			value:     3.14,
			fieldType: FieldTypeAny,
			wantValue: 3.14,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "any: boolean preserved",
			value:     true,
			fieldType: FieldTypeAny,
			wantValue: true,
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "any: nil returns null",
			value:     nil,
			fieldType: FieldTypeAny,
			wantValue: nil,
			wantNull:  true,
			wantErr:   nil,
		},

		// UNSPECIFIED type tests (treated as ANY)
		{
			name:      "unspecified: string preserved",
			value:     "test",
			fieldType: FieldTypeUnspecified,
			wantValue: "test",
			wantNull:  false,
			wantErr:   nil,
		},
		{
			name:      "unspecified: int preserved",
			value:     123,
			fieldType: FieldTypeUnspecified,
			wantValue: 123,
			wantNull:  false,
			wantErr:   nil,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := Coerce(tt.value, tt.fieldType)

			if tt.wantErr != nil {
				if err != tt.wantErr {
					t.Errorf("Coerce() error = %v, wantErr %v", err, tt.wantErr)
				}
				return
			}

			if err != nil {
				t.Errorf("Coerce() unexpected error = %v", err)
				return
			}

			if result.IsNull != tt.wantNull {
				t.Errorf("Coerce() IsNull = %v, want %v", result.IsNull, tt.wantNull)
			}

			if !tt.wantNull {
				// Special handling for float64 comparison (NaN, Inf)
				if wantFloat, ok := tt.wantValue.(float64); ok {
					gotFloat, ok := result.Value.(float64)
					if !ok {
						t.Errorf("Coerce() Value type = %T, want float64", result.Value)
						return
					}
					if math.IsNaN(wantFloat) {
						if !math.IsNaN(gotFloat) {
							t.Errorf("Coerce() Value = %v, want NaN", gotFloat)
						}
					} else if math.IsInf(wantFloat, 0) {
						if !math.IsInf(gotFloat, 0) {
							t.Errorf("Coerce() Value = %v, want Inf", gotFloat)
						}
					} else if gotFloat != wantFloat {
						t.Errorf("Coerce() Value = %v, want %v", gotFloat, wantFloat)
					}
				} else if result.Value != tt.wantValue {
					t.Errorf("Coerce() Value = %v, want %v", result.Value, tt.wantValue)
				}
			}
		})
	}
}

func TestCoerceNumericEdgeCases(t *testing.T) {
	tests := []struct {
		name      string
		value     any
		wantValue float64
		wantErr   error
	}{
		{
			name:      "NaN string",
			value:     "NaN",
			wantValue: math.NaN(),
			wantErr:   nil,
		},
		{
			name:      "positive infinity",
			value:     "Inf",
			wantValue: math.Inf(1),
			wantErr:   nil,
		},
		{
			name:      "negative infinity",
			value:     "-Inf",
			wantValue: math.Inf(-1),
			wantErr:   nil,
		},
		{
			name:      "plus infinity",
			value:     "+Inf",
			wantValue: math.Inf(1),
			wantErr:   nil,
		},
		{
			name:    "invalid mixed string",
			value:   "123abc",
			wantErr: types.ErrCoercionFailed,
		},
		{
			name:    "multiple decimals",
			value:   "1.2.3",
			wantErr: types.ErrCoercionFailed,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := Coerce(tt.value, FieldTypeNumeric)

			if tt.wantErr != nil {
				if err != tt.wantErr {
					t.Errorf("Coerce() error = %v, wantErr %v", err, tt.wantErr)
				}
				return
			}

			if err != nil {
				t.Errorf("Coerce() unexpected error = %v", err)
				return
			}

			gotFloat, ok := result.Value.(float64)
			if !ok {
				t.Errorf("Coerce() Value type = %T, want float64", result.Value)
				return
			}

			if math.IsNaN(tt.wantValue) {
				if !math.IsNaN(gotFloat) {
					t.Errorf("Coerce() Value = %v, want NaN", gotFloat)
				}
			} else if math.IsInf(tt.wantValue, 1) {
				if !math.IsInf(gotFloat, 1) {
					t.Errorf("Coerce() Value = %v, want +Inf", gotFloat)
				}
			} else if math.IsInf(tt.wantValue, -1) {
				if !math.IsInf(gotFloat, -1) {
					t.Errorf("Coerce() Value = %v, want -Inf", gotFloat)
				}
			} else if gotFloat != tt.wantValue {
				t.Errorf("Coerce() Value = %v, want %v", gotFloat, tt.wantValue)
			}
		})
	}
}
