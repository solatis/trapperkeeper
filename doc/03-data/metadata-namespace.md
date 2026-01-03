---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: validation
hub_document: doc/03-data/README.md
tags:
  - metadata
  - namespace
  - validation
---

# Client Metadata Namespace

## Context

TrapperKeeper events include metadata from client sensors for correlation with external systems (Airflow DAGs, Kubernetes pods, batch jobs). Two metadata sources must coexist without collision: system metadata (TrapperKeeper-generated) and user metadata (client-provided).

The reserved `$tk.*` prefix for system metadata prevents collision with user keys and prevents spoofing of system-generated fields. Strict limits on user metadata prevent resource exhaustion from malicious or buggy clients.

**Hub Document**: This document is part of the Data Hub. See [Data Architecture](README.md) for strategic overview of metadata namespace within TrapperKeeper's data model.

## Reserved Prefix ($tk.\*)

All TrapperKeeper system metadata uses `$tk.*` prefix.

### System Metadata Fields

Auto-collected fields:

- `$tk.api_type`: SDK type (`"python"`, `"java"`, `"airflow"`)
- `$tk.api_version`: TrapperKeeper SDK version (e.g., `"0.1.0"`)
- `$tk.client_ip`: IP address of sensor
- `$tk.client_timestamp`: Sensor-side event timestamp (ISO8601 UTC)
- `$tk.server_received_at`: Server ingestion timestamp (ISO8601 UTC)
- `$tk.server_version`: TrapperKeeper server version

**Framework-Specific Fields**:

- `$tk.airflow_dag_id`: Airflow DAG identifier (when using Airflow wrapper)
- `$tk.airflow_task_id`: Airflow task identifier
- `$tk.k8s_pod_name`: Kubernetes pod name (when running in K8s)

**Example**:

```json
{
  "metadata": {
    "sensor_id": "temp-01",
    "team": "data-platform",
    "$tk.api_type": "python",
    "$tk.api_version": "0.1.0",
    "$tk.client_ip": "192.168.1.100",
    "$tk.client_timestamp": "2025-10-29T10:00:00.000Z",
    "$tk.server_received_at": "2025-10-29T10:00:01.123Z"
  }
}
```

**Cross-References**:

- Data Architecture Section 4: Metadata namespace strategy
- Event Schema and Storage: Metadata field in event schema

## Client-Side Enforcement

SDKs reject metadata keys starting with `$` prefix.

### Validation at Sensor Initialization

```python
from trapperkeeper import Sensor

# Valid: User metadata without $ prefix
sensor = Sensor(
    api_key=api_key,
    metadata={
        'team': 'data-platform',
        'environment': 'production',
        'job_id': job_id
    }
)

# Invalid: Rejected at initialization
sensor = Sensor(
    api_key=api_key,
    metadata={
        '$custom_field': 'value'  # ERROR
    }
)
# → ValueError: Metadata keys cannot start with '$' (reserved for system use)
```

### Validation at add_metadata()

```python
sensor = Sensor(api_key=api_key)

# Valid
sensor.add_metadata('sensor_id', 'temp-01')

# Invalid
sensor.add_metadata('$tk.custom', 'value')
# → ValueError: Metadata keys cannot start with '$' (reserved for system use)
```

**Error Message**: `"Metadata keys cannot start with '$' (reserved for system use)"`

**Rationale**: Fail fast at client-side prevents round-trip errors.

**Cross-References**:

- SDK Model Section 7: Metadata collection patterns

## Server-Side Enforcement

API server strips client-supplied `$tk.*` keys and overwrites with correct values.

### Request Processing

```go
import (
    "strings"
    "time"
)

func sanitizeMetadata(metadata map[string]string, r *http.Request) map[string]string {
    // Strip any client-supplied $tk.* keys
    for key := range metadata {
        if strings.HasPrefix(key, "$tk.") {
            delete(metadata, key)
        }
    }

    // Add correct system metadata
    metadata["$tk.api_version"] = version.APIVersion
    metadata["$tk.server_version"] = version.ServerVersion
    metadata["$tk.server_received_at"] = time.Now().UTC().Format(time.RFC3339Nano)

    // Extract from request context
    metadata["$tk.client_ip"] = extractClientIP(r)

    return metadata
}
```

**Security Property**: Prevents malicious clients from forging SDK version, ingestion time, or server state.

**No Error Returned**: Silent overwrite for security (don't reveal detection).

**Logging**: Warning logged if client sent reserved keys (potential security issue).

**Cross-References**:

- API Service Architecture: Request processing pipeline
- Unified Validation and Input Sanitization: Metadata validation rules

## User Metadata Limits

User-provided metadata has strict limits preventing resource exhaustion.

### Limit Definitions

- Max key-value pairs: 64 per sensor
- Max key length: 128 characters (UTF-8)
- Max value length: 1024 characters (UTF-8)
- Total metadata size: 64KB maximum (sum of all keys + values)
- Character restrictions: UTF-8 encoding, no control characters (except values may contain newlines/tabs)

### Validation Errors

```python
sensor = Sensor(api_key=api_key)

# Error: Too many pairs
for i in range(65):
    sensor.add_metadata(f'key_{i}', 'value')
# → ValueError: Metadata limit exceeded: 64 key-value pairs maximum

# Error: Key too long
sensor.add_metadata('a' * 129, 'value')
# → ValueError: Metadata key too long: 'aaa...' (129 chars, max 128)

# Error: Value too long
sensor.add_metadata('description', 'x' * 1025)
# → ValueError: Metadata value too long for key 'description' (1025 chars, max 1024)

# Error: Total size exceeded
for i in range(64):
    sensor.add_metadata(f'key_{i}', 'x' * 1024)
# → ValueError: Total metadata size 80KB exceeds 64KB limit
```

**Enforcement Points**:

- Client-side: SDK validates limits during `add_metadata()`
- Server-side: API validates limits as defense-in-depth
- Database: Metadata column size enforced by schema

**Cross-References**:

- SDK Model Section 7: Metadata collection
- Unified Validation and Input Sanitization Section 3.3: Metadata validation rules

## Environment Variable Scanning

SDKs automatically collect environment variables with `TK_META_` prefix.

### Automatic Collection

```bash
# Set environment variables
export TK_META_AIRFLOW_DAG_ID=daily_etl_pipeline
export TK_META_AIRFLOW_TASK_ID=process_batch
export TK_META_K8S_POD_NAME=etl-worker-abc123
export TK_META_TEAM=data-platform

# Python SDK automatically includes
sensor = Sensor(api_key=api_key)
# Metadata includes:
# {
#   'airflow_dag_id': 'daily_etl_pipeline',
#   'airflow_task_id': 'process_batch',
#   'k8s_pod_name': 'etl-worker-abc123',
#   'team': 'data-platform'
# }
```

**Key Transformation**:

- `TK_META_AIRFLOW_DAG_ID` → `airflow_dag_id` (lowercase, remove prefix)
- `TK_META_K8S_POD_NAME` → `k8s_pod_name`

**Benefits**:

- Zero-ceremony metadata collection
- Integrates with container orchestration
- Airflow operators set variables automatically
- Standard Unix convention

**Limit Validation**: Environment-collected metadata counts toward 64-pair limit.

**Cross-References**:

- SDK Model Section 7: Automatic metadata collection
- Configuration Management: Environment variable conventions

## Framework-Specific Metadata

Special handling for Airflow and Spark frameworks.

### Airflow Wrapper

```python
# trapperkeeper/airflow.py
from airflow.operators.python import PythonOperator
from trapperkeeper import Sensor

class TrapperKeeperOperator(PythonOperator):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

    def execute(self, context):
        # Auto-inject Airflow metadata
        sensor = Sensor(
            api_key=os.environ['TK_API_KEY'],
            metadata={
                'airflow_dag_id': context['dag'].dag_id,
                'airflow_task_id': context['task'].task_id,
                'airflow_run_id': context['run_id'],
                'airflow_execution_date': context['execution_date'].isoformat(),
            }
        )

        # User task logic
        return super().execute(context)
```

**Automatic $tk.api_type**: Airflow wrapper sets `$tk.api_type = "airflow"` (not `"python"`).

### Spark Integration

```java
// trapperkeeper-spark Scala wrapper
import ai.trapperkeeper.Sensor;

Dataset<Row> df = spark.read().parquet("data.parquet");

// Auto-inject Spark metadata
Sensor sensor = new Sensor(
    apiKey,
    Map.of(
        "spark_app_id", spark.sparkContext().applicationId(),
        "spark_executor_id", spark.sparkContext().executorId(),
        "spark_job_id", spark.sparkContext().jobId()
    )
);

sensor.observeDataFrame(df);
```

**Cross-References**:

- Batch Processing and Vectorization: Pandas and Spark integration
- SDK Model: Framework-specific SDK extensions

## Future Prefix Extensions

Reserved prefixes for future functionality.

**Currently Reserved**:

- `$tk.*` - TrapperKeeper system metadata

**Potential Future Prefixes** (not implemented in MVP):

- `$user.*` - User identity fields (multi-tenant expansion)
- `$env.*` - Environment detection metadata
- `$fw.*` - Framework-specific fields

**Rationale**: Single prefix sufficient for MVP. Future prefixes enable namespaced extensions without breaking existing clients.

**Cross-References**:

- Data Architecture Section 4: Metadata namespace future considerations

## Related Documents

**Dependencies** (read these first):

- Data Architecture: Metadata namespace strategic overview

**Related Spokes** (siblings in this hub):

- Event Schema and Storage: Metadata field in event schema
- Timestamps: System timestamp metadata fields

**Extended by**:

- SDK Model: Automatic metadata collection
- Unified Validation and Input Sanitization: Metadata validation rules
