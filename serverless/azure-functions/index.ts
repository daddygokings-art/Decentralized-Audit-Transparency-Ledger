/**
 * Azure Functions Event Processor (#522)
 *
 * Implements HTTP Trigger and Azure Event Grid / Service Bus processing.
 */

import { AzureFunction, Context, HttpRequest } from "@azure/functions";
import { ServerlessEventPipeline } from "../core/pipeline";
import { ContractEvent } from "../core/types";

const pipeline = new ServerlessEventPipeline(
  { targetFormat: "json" },
  [],
  [{ destination: "azure-servicebus", topicOrQueueArn: "processed-events-queue" }]
);

const httpTrigger: AzureFunction = async function (context: Context, req: HttpRequest): Promise<void> {
  const event: ContractEvent = req.body;
  if (!event || !event.eventType) {
    context.res = {
      status: 400,
      body: { error: "Invalid contract event payload" },
    };
    return;
  }

  const result = await pipeline.processEvent(event);
  context.res = {
    status: result.success ? 200 : 500,
    body: result,
  };
};

export default httpTrigger;
