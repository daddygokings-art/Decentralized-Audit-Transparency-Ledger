"""
Feast Feature View Definitions (#524)
"""

try:
    from feast import Entity, FeatureView, Field, FileSource, ValueType
    from feast.types import Float32, Int64, String
    from datetime import timedelta

    submitter = Entity(name="submitter", join_keys=["submitter_address"])

    events_source = FileSource(
        path="data/features/offline/submitter_behavior",
        timestamp_field="event_timestamp",
        created_timestamp_column="created_timestamp",
    )

    submitter_behavior_fv = FeatureView(
        name="submitter_behavior_fv",
        entities=[submitter],
        ttl=timedelta(days=30),
        schema=[
            Field(name="tx_count_1h", dtype=Float32),
            Field(name="tx_count_24h", dtype=Float32),
            Field(name="avg_metadata_bytes_24h", dtype=Float32),
            Field(name="burst_ratio_1h_to_24h", dtype=Float32),
            Field(name="velocity_anomaly_score", dtype=Float32),
        ],
        online=True,
        source=events_source,
    )
except ImportError:
    pass
