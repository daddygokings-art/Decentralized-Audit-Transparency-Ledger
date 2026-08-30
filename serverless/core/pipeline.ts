/**
 * Composite Serverless Pipeline Orchestrator (#522)
 *
 * Integrates Transform -> Enrich -> Filter -> Route into a unified pipeline.
 */

import { ContractEvent, FilterRule, ProcessingResult, RouteTarget, TransformationRule } from "./types";
import { EventTransformer } from "./transformer";
import { EventEnricher } from "./enricher";
import { EventFilter } from "./filter";
import { EventRouter } from "./router";

export class ServerlessEventPipeline {
  private transformationRule: TransformationRule;
  private filterRules: FilterRule[];
  private routeTargets: RouteTarget[];

  constructor(
    transformationRule: TransformationRule = { targetFormat: "json" },
    filterRules: FilterRule[] = [],
    routeTargets: RouteTarget[] = []
  ) {
    this.transformationRule = transformationRule;
    this.filterRules = filterRules;
    this.routeTargets = routeTargets;
  }

  public async processEvent(event: ContractEvent): Promise<ProcessingResult> {
    const startTime = Date.now();

    try {
      // 1. Filtering
      const filterRes = EventFilter.evaluate(event, this.filterRules);
      if (!filterRes.pass) {
        return {
          success: true,
          eventId: event.id || String(event.index),
          dropped: true,
          dropReason: filterRes.reason,
          durationMs: Date.now() - startTime,
        };
      }

      // 2. Enrichment
      const enriched = await EventEnricher.enrich(event);

      // 3. Transformation
      const transformed = EventTransformer.transform(enriched, this.transformationRule);

      // 4. Routing
      const { dispatched } = await EventRouter.dispatch(transformed, this.routeTargets);

      return {
        success: true,
        eventId: event.id || String(event.index),
        transformedData: transformed,
        routedDestinations: dispatched,
        durationMs: Date.now() - startTime,
      };
    } catch (err: any) {
      return {
        success: false,
        eventId: event.id || String(event.index),
        error: err.message,
        durationMs: Date.now() - startTime,
      };
    }
  }
}
