// Package types provides domain models shared across TrapperKeeper components.
//
// Zero-dependency design: types.go and errors.go use only encoding/json to keep
// SDK binary size minimal (~10KB overhead). ID utilities in ids.go import uuid
// but are isolated for selective SDK inclusion.
//
// Separation from protobuf: Generated proto types live in internal/protobuf.
// This package contains hand-written types for concepts that don't belong in
// proto (error types, helper methods) or need to avoid pulling in proto deps.
package types

import "encoding/json"

// EventID represents a UUIDv7 event identifier.
// String alias enables type safety while maintaining JSON string serialization.
// UUIDv7 time-ordering ensures sequential IDs cluster in B-tree indexes.
type EventID string

// RuleID represents a UUIDv7 rule identifier.
// String alias enables type safety while maintaining JSON string serialization.
type RuleID string

// Payload represents an arbitrary JSON payload from a sensor.
// json.RawMessage wrapper preserves original bytes for schema-agnostic storage.
// No validation or parsing; rule engine operates directly on raw JSON structure.
type Payload json.RawMessage

// MarshalJSON implements json.Marshaler.
// Delegates to json.RawMessage to preserve original payload bytes unchanged.
func (p Payload) MarshalJSON() ([]byte, error) {
	if p == nil {
		return []byte("null"), nil
	}
	return json.RawMessage(p).MarshalJSON()
}

// UnmarshalJSON implements json.Unmarshaler.
// Delegates to json.RawMessage to capture raw bytes without parsing.
func (p *Payload) UnmarshalJSON(data []byte) error {
	return (*json.RawMessage)(p).UnmarshalJSON(data)
}

// Metadata represents user-provided key-value metadata.
// String-only values enforce consistent type handling; complex metadata belongs in payload.
// $tk.* prefix reserved for system metadata (sensor_id, timestamp, etc.).
type Metadata map[string]string

// Resource limits enforced by the rule engine to prevent DoS and maintain performance.
const (
	// MaxMetadataPairs limits metadata pairs to prevent unbounded iteration.
	// 64 pairs allows rich context without significant overhead per event.
	MaxMetadataPairs = 64

	// MaxMetadataKeyLength prevents excessively long keys.
	// 128 chars accommodates namespaced keys like "$tk.sensor.environment.cluster_id".
	MaxMetadataKeyLength = 128

	// MaxMetadataValueLength prevents unbounded value sizes.
	// 1KB allows structured identifiers (UUIDs, URLs, small JSON) without blob storage.
	MaxMetadataValueLength = 1024

	// MaxMetadataTotalSize caps total metadata size to bound memory per event.
	// 64KB = MaxMetadataPairs * MaxMetadataValueLength worst case.
	MaxMetadataTotalSize = 64 * 1024

	// MaxPayloadSize limits event payload to prevent OOM during batch processing.
	// 1MB allows typical application events; larger payloads should use external storage.
	MaxPayloadSize = 1024 * 1024

	// MaxPathDepth prevents stack overflow during recursive path resolution.
	// 16 levels handles deeply nested JSON ($.a.b.c...) without performance degradation.
	MaxPathDepth = 16

	// MaxNestedWildcards limits wildcard expansion to prevent combinatorial explosion.
	// 2 wildcards allow patterns like $.orders[*].items[*].price without exponential fan-out.
	MaxNestedWildcards = 2

	// MaxInOperatorValues limits IN operator list size to prevent quadratic comparison cost.
	// 64 values supports typical enum-style checks without degrading to O(n^2) behavior.
	MaxInOperatorValues = 64
)
