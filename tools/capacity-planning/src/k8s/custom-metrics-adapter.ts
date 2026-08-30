import client from 'prom-client';

export class CustomMetricsAdapter {
  public predictedTpsGauge: client.Gauge<string>;
  public recommendedReplicasGauge: client.Gauge<string>;
  public capacityHeadroomGauge: client.Gauge<string>;
  public estimatedMonthlyCostGauge: client.Gauge<string>;

  constructor(register: client.Registry) {
    this.predictedTpsGauge = new client.Gauge({
      name: 'audit_ledger_predicted_tps',
      help: 'ML-forecasted contract event TPS for the next 15-minute horizon',
      registers: [register],
    });

    this.recommendedReplicasGauge = new client.Gauge({
      name: 'audit_ledger_recommended_replicas',
      help: 'Proactive HPA recommended replica count for bridge and API pods',
      registers: [register],
    });

    this.capacityHeadroomGauge = new client.Gauge({
      name: 'audit_ledger_capacity_headroom_percent',
      help: 'Remaining storage and throughput capacity headroom percentage',
      registers: [register],
    });

    this.estimatedMonthlyCostGauge = new client.Gauge({
      name: 'audit_ledger_estimated_monthly_cost_usd',
      help: 'Projected monthly Kubernetes cluster compute cost in USD',
      registers: [register],
    });
  }

  public updateMetrics(predictedTps: number, recommendedReplicas: number, headroom: number, monthlyCost: number) {
    this.predictedTpsGauge.set(predictedTps);
    this.recommendedReplicasGauge.set(recommendedReplicas);
    this.capacityHeadroomGauge.set(headroom);
    this.estimatedMonthlyCostGauge.set(monthlyCost);
  }
}
