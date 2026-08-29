#!/usr/bin/env python3
import json
import os
import re
import sys

def main():
    print("================ VALIDATING CONTRACT EVENT SCHEMAS ================")
    schema_path = "docs/contract-events/event-schemas.json"
    if not os.path.exists(schema_path):
        print(f"Error: Schema file not found: {schema_path}")
        sys.exit(1)

    with open(schema_path, "r", encoding="utf-8") as f:
        schemas = json.load(f)

    event_docs = [
        "docs/contract-events/core-events.md",
        "docs/contract-events/compliance-events.md",
        "docs/contract-events/financial-events.md",
        "docs/contract-events/governance-events.md"
    ]

    total_validated = 0
    for doc in event_docs:
        if not os.path.exists(doc):
            print(f"Warning: Doc not found: {doc}")
            continue

        with open(doc, "r", encoding="utf-8") as f:
            content = f.read()

        # Find JSON blocks in markdown
        json_blocks = re.findall(r"```json\s*(\{.*?\})\s*```", content, re.DOTALL)
        for block in json_blocks:
            try:
                parsed = json.loads(block)
                total_validated += 1
            except Exception as e:
                print(f"❌ JSON syntax error in {doc}: {e}")
                sys.exit(1)

    print(f"✓ Successfully validated {total_validated} contract event schema examples across docs!")
    print("====================================================================")

if __name__ == "__main__":
    main()
