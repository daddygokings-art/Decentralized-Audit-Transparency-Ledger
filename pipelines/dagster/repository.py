"""
Dagster Repository Definitions (#523)
"""

from dagster import Definitions, load_assets_from_modules
from .assets import events, features
from .resources.ledger_resource import LedgerRpcResource
from .jobs.daily_aggregation import daily_pipeline_job, daily_schedule

all_assets = [*load_assets_from_modules([events]), *load_assets_from_modules([features])]

defs = Definitions(
    assets=all_assets,
    jobs=[daily_pipeline_job],
    schedules=[daily_schedule],
    resources={
        "ledger_rpc": LedgerRpcResource(rpc_url="https://soroban-rpc.mainnet.stellar.org"),
    },
)
