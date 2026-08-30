export interface ProviderStateContext {
  events: any[];
  healthy: boolean;
  keys: any[];
}

export const defaultStateContext: ProviderStateContext = {
  events: [
    {
      index: 0,
      contract_id: "CA3D5KRYMCMUZ5AFQIBIIYA5T3W6U6R6N2P7Z5K4A5M7X5SLLM",
      topic: "anti_corruption",
      payload: {
        action: "incident_reported",
        severity: "CRITICAL"
      },
      timestamp: "2026-08-28T10:00:00Z"
    }
  ],
  healthy: true,
  keys: [
    {
      id: "key-12345",
      role: "admin",
      status: "active"
    }
  ]
};

export function setupProviderState(stateName: string, context: ProviderStateContext): void {
  console.log(`[Provider State Setup] Setting up state: "${stateName}"`);

  switch (stateName) {
    case 'events exist in the ledger':
      context.events = [
        {
          index: 0,
          contract_id: "CA3D5KRYMCMUZ5AFQIBIIYA5T3W6U6R6N2P7Z5K4A5M7X5SLLM",
          topic: "anti_corruption",
          payload: {
            action: "incident_reported",
            severity: "CRITICAL"
          },
          timestamp: "2026-08-28T10:00:00Z"
        }
      ];
      break;

    case 'event with ID 0 exists':
      context.events = [
        {
          index: 0,
          contract_id: "CA3D5KRYMCMUZ5AFQIBIIYA5T3W6U6R6N2P7Z5K4A5M7X5SLLM",
          topic: "anti_corruption",
          payload: {
            action: "incident_reported",
            severity: "CRITICAL"
          },
          timestamp: "2026-08-28T10:00:00Z"
        }
      ];
      break;

    case 'no events match query criteria':
      context.events = [];
      break;

    case 'system is healthy and operational':
      context.healthy = true;
      break;

    default:
      console.log(`[Provider State Setup] No specific mutation needed for: "${stateName}"`);
      break;
  }
}
