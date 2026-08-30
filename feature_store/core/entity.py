"""
Entity Definitions for ML Feature Store (#524)
"""

from dataclasses import dataclass
from typing import Optional

@dataclass(frozen=True)
class Entity:
    name: str
    join_key: str
    description: str
    owner: str = "ml-team"

# Standard Entities
SUBMITTER_ENTITY = Entity(
    name="submitter",
    join_key="submitter_address",
    description="Stellar address submitting audit events",
)

CONTRACT_ENTITY = Entity(
    name="contract",
    join_key="contract_id",
    description="Target Soroban smart contract",
)

CATEGORY_ENTITY = Entity(
    name="category",
    join_key="category_name",
    description="Regulatory or functional category",
)
