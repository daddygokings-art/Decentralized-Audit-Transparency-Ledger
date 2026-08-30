/**
 * Google Cloud Functions Event Processor (#522)
 *
 * Handles HTTP and Cloud Pub/Sub triggers for contract events.
 */

import { HttpFunction, EventFunction } from "@google-cloud/functions-framework";
import { ServerlessEventPipeline } from "../core/pipeline";
import { ContractEvent } from "../core/types";

const pipeline = new ServerlessEventPipeline(
  { targetFormat: "json" },
  [],
  [{ destination: "gcp-pubsub", topicOrQueueArn: "projects/audit-ledger/topics/processed-events" }]
);

// HTTP Cloud Function
export const processContractEventHttp: HttpFunction = async (req, res) => {
  if (req.method !== "POST") {
    res.status(405).send("Method Not Allowed");
    return;
  }

  const event: ContractEvent = req.body;
  const result = await pipeline.processEvent(event);
  res.status(result.success ? 200 : 500).json(result);
};

// PubSub Cloud Function Trigger
export const processContractEventPubSub: EventFunction = async (cloudEvent: any) => {
  const base64Data = cloudEvent.data?.message?.data;
  if (!base64Data) return;

  const jsonStr = Buffer.from(base64Data, "base64").toString();
  const event: ContractEvent = JSON.parse(jsonStr);

  await pipeline.processEvent(event);
};
