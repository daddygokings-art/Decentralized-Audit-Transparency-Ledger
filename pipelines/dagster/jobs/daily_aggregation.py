"""
Dagster Scheduled Jobs & Definitions (#523)
"""

from dagster import define_asset_job, ScheduleDefinition

daily_pipeline_job = define_asset_job(
    name="daily_audit_pipeline_job",
    selection=["raw_contract_events", "silver_clean_events", "gold_daily_audit_summary", "submitter_behavioral_features"],
)

daily_schedule = ScheduleDefinition(
    job=daily_pipeline_job,
    cron_schedule="0 0 * * *", # Every midnight
)
