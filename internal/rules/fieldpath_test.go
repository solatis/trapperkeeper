package rules

import (
	"encoding/json"
	"testing"

	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
	"github.com/solatis/trapperkeeper/internal/types"
)

// Test normal path resolution cases
func TestResolve_Normal(t *testing.T) {
	tests := []struct {
		name     string
		path     []types.PathSegment
		data     string
		expected any
		wantErr  error
	}{
		{
			name:     "nested object traversal",
			path:     []types.PathSegment{{Key: "user"}, {Key: "name"}},
			data:     `{"user": {"name": "Alice"}}`,
			expected: "Alice",
			wantErr:  nil,
		},
		{
			name:     "array index access",
			path:     []types.PathSegment{{Key: "users"}, {Index: 0, IsIndex: true}, {Key: "name"}},
			data:     `{"users": [{"name": "Bob"}]}`,
			expected: "Bob",
			wantErr:  nil,
		},
		{
			name:     "single wildcard first match",
			path:     []types.PathSegment{{Key: "items"}, {Wildcard: true}, {Key: "price"}},
			data:     `{"items": [{"price": 10}, {"price": 20}]}`,
			expected: float64(10),
			wantErr:  nil,
		},
		{
			name:     "wildcard on object sorted keys",
			path:     []types.PathSegment{{Wildcard: true}, {Key: "value"}},
			data:     `{"z": {"value": 1}, "a": {"value": 2}, "m": {"value": 3}}`,
			expected: float64(2), // 'a' comes first alphabetically
			wantErr:  nil,
		},
		{
			name:     "deep nesting",
			path:     []types.PathSegment{{Key: "a"}, {Key: "b"}, {Key: "c"}, {Key: "d"}},
			data:     `{"a": {"b": {"c": {"d": "deep"}}}}`,
			expected: "deep",
			wantErr:  nil,
		},
		{
			name:     "nested wildcards",
			path:     []types.PathSegment{{Key: "orders"}, {Wildcard: true}, {Key: "items"}, {Wildcard: true}, {Key: "price"}},
			data:     `{"orders": [{"items": [{"price": 100}, {"price": 200}]}, {"items": [{"price": 300}]}]}`,
			expected: float64(100),
			wantErr:  nil,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := Resolve(tt.path, json.RawMessage(tt.data))
			if err != tt.wantErr {
				t.Fatalf("Resolve() error = %v, wantErr %v", err, tt.wantErr)
			}
			if tt.wantErr == nil {
				if !result.Found {
					t.Fatalf("Resolve() Found = false, want true")
				}
				if result.Value != tt.expected {
					t.Errorf("Resolve() Value = %v, expected %v", result.Value, tt.expected)
				}
			}
		})
	}
}

// Test resolved path tracking with wildcards
func TestResolve_ResolvedPath(t *testing.T) {
	path := []types.PathSegment{{Key: "items"}, {Wildcard: true}, {Key: "price"}}
	data := `{"items": [{"price": 10}, {"price": 20}]}`

	result, err := Resolve(path, json.RawMessage(data))
	if err != nil {
		t.Fatalf("Resolve() error = %v", err)
	}

	expectedPath := []types.PathSegment{
		{Key: "items"},
		{Index: 0, IsIndex: true},
		{Key: "price"},
	}

	if len(result.ResolvedPath) != len(expectedPath) {
		t.Fatalf("ResolvedPath length = %d, expected %d", len(result.ResolvedPath), len(expectedPath))
	}

	for i, seg := range result.ResolvedPath {
		exp := expectedPath[i]
		if seg.Key != exp.Key || seg.Index != exp.Index || seg.IsIndex != exp.IsIndex || seg.Wildcard != exp.Wildcard {
			t.Errorf("ResolvedPath[%d] = %+v, expected %+v", i, seg, exp)
		}
	}
}

// Test edge cases
func TestResolve_EdgeCases(t *testing.T) {
	tests := []struct {
		name    string
		path    []types.PathSegment
		data    string
		wantErr error
	}{
		{
			name:    "empty object",
			path:    []types.PathSegment{{Key: "missing"}},
			data:    `{}`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "empty array",
			path:    []types.PathSegment{{Index: 0, IsIndex: true}},
			data:    `[]`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "empty array with wildcard",
			path:    []types.PathSegment{{Wildcard: true}, {Key: "price"}},
			data:    `[]`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "null value at intermediate level",
			path:    []types.PathSegment{{Key: "user"}, {Key: "name"}},
			data:    `{"user": null}`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "scalar value but path continues",
			path:    []types.PathSegment{{Key: "value"}, {Key: "nested"}},
			data:    `{"value": "scalar"}`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "array index out of bounds",
			path:    []types.PathSegment{{Index: 5, IsIndex: true}},
			data:    `[1, 2, 3]`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "negative array index",
			path:    []types.PathSegment{{Index: -1, IsIndex: true}},
			data:    `[1, 2, 3]`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "string key on array",
			path:    []types.PathSegment{{Key: "key"}},
			data:    `[1, 2, 3]`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "integer index on object",
			path:    []types.PathSegment{{Index: 0, IsIndex: true}},
			data:    `{"0": "value"}`,
			wantErr: types.ErrFieldNotFound,
		},
		{
			name:    "wildcard on empty object",
			path:    []types.PathSegment{{Wildcard: true}, {Key: "value"}},
			data:    `{}`,
			wantErr: types.ErrFieldNotFound,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := Resolve(tt.path, json.RawMessage(tt.data))
			if err != tt.wantErr {
				t.Errorf("Resolve() error = %v, wantErr %v", err, tt.wantErr)
			}
			if err == nil && !result.Found {
				t.Errorf("Resolve() Found = false, want true")
			}
		})
	}
}

// Test error conditions
func TestResolve_Errors(t *testing.T) {
	tests := []struct {
		name    string
		path    []types.PathSegment
		data    string
		wantErr error
	}{
		{
			name: "path too deep",
			path: []types.PathSegment{
				{Key: "a"}, {Key: "b"}, {Key: "c"}, {Key: "d"},
				{Key: "e"}, {Key: "f"}, {Key: "g"}, {Key: "h"},
				{Key: "i"}, {Key: "j"}, {Key: "k"}, {Key: "l"},
				{Key: "m"}, {Key: "n"}, {Key: "o"}, {Key: "p"},
				{Key: "q"}, // 17 segments
			},
			data:    `{}`,
			wantErr: types.ErrPathTooDeep,
		},
		{
			name: "too many wildcards",
			path: []types.PathSegment{
				{Wildcard: true},
				{Wildcard: true},
				{Wildcard: true}, // 3 wildcards
			},
			data:    `[]`,
			wantErr: types.ErrTooManyWildcards,
		},
		{
			name:    "invalid JSON",
			path:    []types.PathSegment{{Key: "key"}},
			data:    `{invalid json}`,
			wantErr: nil, // json.Unmarshal error, not a types error
		},
		{
			name:    "missing intermediate key",
			path:    []types.PathSegment{{Key: "a"}, {Key: "b"}, {Key: "c"}},
			data:    `{"a": {"x": "wrong"}}`,
			wantErr: types.ErrFieldNotFound,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := Resolve(tt.path, json.RawMessage(tt.data))
			if tt.wantErr != nil && err != tt.wantErr {
				t.Errorf("Resolve() error = %v, wantErr %v", err, tt.wantErr)
			}
			if tt.wantErr == nil && tt.name == "invalid JSON" && err == nil {
				t.Errorf("Resolve() expected JSON error, got nil")
			}
		})
	}
}

// Property-based test: resolution never crashes
func TestResolve_PropertyNeverCrashes(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	properties := gopter.NewProperties(parameters)

	properties.Property("resolution never crashes regardless of input", prop.ForAll(
		func(depth int, wildcards int, useArray bool) bool {
			// Generate arbitrary path
			path := make([]types.PathSegment, depth)
			wildcardCount := 0
			for i := 0; i < depth; i++ {
				if wildcardCount < wildcards && i%2 == 0 {
					path[i] = types.PathSegment{Wildcard: true}
					wildcardCount++
				} else if useArray && i%3 == 0 {
					path[i] = types.PathSegment{Index: i, IsIndex: true}
				} else {
					path[i] = types.PathSegment{Key: "key"}
				}
			}

			// Generate arbitrary JSON data
			data := `{"key": [{"key": "value"}]}`

			// Resolution should never panic
			defer func() {
				if r := recover(); r != nil {
					t.Errorf("Resolve() panicked: %v", r)
				}
			}()

			_, _ = Resolve(path, json.RawMessage(data))
			return true
		},
		gen.IntRange(0, 20),
		gen.IntRange(0, 5),
		gen.Bool(),
	))

	properties.TestingRun(t)
}

// Property-based test: schema variations
func TestResolve_PropertySchemaVariations(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	properties := gopter.NewProperties(parameters)

	properties.Property("handles arbitrary nested structures", prop.ForAll(
		func(nestLevel int, hasNull bool, isEmpty bool) bool {
			var data string
			if isEmpty {
				if nestLevel%2 == 0 {
					data = `{}`
				} else {
					data = `[]`
				}
			} else if hasNull {
				data = `{"a": null, "b": {"c": null}}`
			} else {
				data = `{"a": {"b": {"c": "deep"}}}`
			}

			path := []types.PathSegment{{Key: "a"}, {Key: "b"}, {Key: "c"}}

			defer func() {
				if r := recover(); r != nil {
					t.Errorf("Resolve() panicked with data=%s: %v", data, r)
				}
			}()

			_, _ = Resolve(path, json.RawMessage(data))
			return true
		},
		gen.IntRange(0, 10),
		gen.Bool(),
		gen.Bool(),
	))

	properties.TestingRun(t)
}

// Property-based test: wildcard determinism
func TestResolve_PropertyWildcardDeterminism(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 50
	properties := gopter.NewProperties(parameters)

	properties.Property("wildcard resolution is deterministic", prop.ForAll(
		func(seed int) bool {
			// Same input should always produce same result
			path := []types.PathSegment{{Wildcard: true}, {Key: "value"}}
			data := `{"z": {"value": 1}, "a": {"value": 2}, "m": {"value": 3}}`

			result1, err1 := Resolve(path, json.RawMessage(data))
			result2, err2 := Resolve(path, json.RawMessage(data))

			if err1 != err2 {
				return false
			}
			if err1 == nil {
				if result1.Value != result2.Value {
					return false
				}
				if len(result1.ResolvedPath) != len(result2.ResolvedPath) {
					return false
				}
			}
			return true
		},
		gen.Int(),
	))

	properties.TestingRun(t)
}
