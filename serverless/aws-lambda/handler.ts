/**
 * AWS Lambda Event Processor (#522)
 *
 * Handles contract event processing from SQS, EventBridge, and Direct Invocation.
 */

import { SQSEvent, SQSRecord, EventBridgeEvent, Context } from "aws-lambda";
import { ServerlessEventPipeline } from "../core/pipeline";
import { ContractEvent } from "../core/types";

const pipeline = new ServerlessEventPipeline(
  {
    targetFormat: "json",
    anonymizeFields: ["submitterIp"],
  },
  [
    {
      name: "IgnoreHeartbeats",
      mode: "exclude",
      predicates: [{ field: "eventType", operator: "eq", value: "HEARTBEAT" }],
    },
  ],
  [
    { destination: "aws-eventbridge", topicOrQueueArn: process.env.EVENTBRIDGE_BUS_ARN },
    { destination: "aws-sqs", topicOrQueueArn: process.env.AUDIT_ARCHIVE_QUEUE_ARN },
  ]
);

export const handler = async (event: any, _context: Context): Promise<any> => {
  // 1. Direct invocation with event payload
  if (event.eventType && event.submitter) {
    const res = await pipeline.processEvent(event as ContractEvent);
    return { statusCode: res.success ? 200 : 500, body: JSON.stringify(res) };
  }

  // 2. SQS Batch Trigger
  if (event.Records && Array.isArray(event.Records)) {
    const results = [];
    for (const record of event.Records as SQSRecord[]) {
      try {
        const body: ContractEvent = JSON.parse(record.body);
        const res = await pipeline.processEvent(body);
        results.push(res);
      } catch (err: any) {
        results.push({ success: false, error: err.message });
      }
    }
    return { statusCode: 200, processedCount: results.length, results };
  }

  // 3. EventBridge Trigger
  if (event["detail-type"]) {
    const detail: ContractEvent = event.detail;
    const res = await pipeline.processEvent(detail);
    return { statusCode: 200, result: res };
  }

  return { statusCode: 400, body: JSON.stringify({ error: "Unsupported event source" }) };
};
