import { ForecastResult, TelemetryPoint } from '../types';

export class MLForecaster {
  private alpha = 0.35; // Level smoothing coefficient
  private beta = 0.15; // Trend smoothing coefficient
  private gamma = 0.25; // Seasonality smoothing coefficient
  private seasonLength = 24; // 24-hour periodic daily cycle

  public forecast(history: TelemetryPoint[], horizonMinutes: number = 15): ForecastResult {
    if (history.length < 5) {
      const last = history[history.length - 1]?.tps || 10;
      return {
        horizonMinutes,
        predictedTps: last,
        upperConfidenceTps: last * 1.3,
        lowerConfidenceTps: Math.max(0, last * 0.7),
        trend: 'STABLE',
        seasonalFactor: 1.0,
        anomalyScore: 0.05,
        timestamp: new Date().toISOString(),
      };
    }

    // Extract TPS series
    const series = history.map((h) => h.tps);
    let level = series[0];
    let trend = (series[series.length - 1] - series[0]) / series.length;
    const seasonal = new Array(this.seasonLength).fill(1.0);

    // Holt-Winters Additive / Multiplicative Estimation
    for (let i = 0; i < series.length; i++) {
      const val = series[i];
      const sIdx = i % this.seasonLength;
      const prevLevel = level;
      const prevTrend = trend;

      level = this.alpha * (val / (seasonal[sIdx] || 1.0)) + (1 - this.alpha) * (prevLevel + prevTrend);
      trend = this.beta * (level - prevLevel) + (1 - this.beta) * prevTrend;
      seasonal[sIdx] = this.gamma * (val / (level || 1.0)) + (1 - this.gamma) * seasonal[sIdx];
    }

    // Compute steps ahead based on 5-minute bucket intervals
    const stepsAhead = Math.max(1, Math.round(horizonMinutes / 5));
    const targetSeasonalIdx = (series.length + stepsAhead) % this.seasonLength;
    const rawForecast = (level + stepsAhead * trend) * (seasonal[targetSeasonalIdx] || 1.0);
    const predictedTps = Math.max(1, Math.round(rawForecast));

    // Volatility and Anomaly confidence intervals
    const variance = series.reduce((acc, v) => acc + Math.pow(v - (level || 1), 2), 0) / series.length;
    const stdDev = Math.sqrt(variance);
    const z = 1.96; // 95% confidence interval

    const upperConfidenceTps = Math.round(predictedTps + z * stdDev * Math.sqrt(stepsAhead));
    const lowerConfidenceTps = Math.max(0, Math.round(predictedTps - z * stdDev * Math.sqrt(stepsAhead)));

    let trendLabel: ForecastResult['trend'] = 'STABLE';
    if (trend > 0.5) trendLabel = 'INCREASING';
    else if (trend < -0.5) trendLabel = 'DECREASING';

    const latest = series[series.length - 1];
    const anomalyScore = Math.min(1.0, Math.abs(latest - predictedTps) / (stdDev * 3 || 1));

    return {
      horizonMinutes,
      predictedTps,
      upperConfidenceTps,
      lowerConfidenceTps,
      trend: trendLabel,
      seasonalFactor: seasonal[targetSeasonalIdx] || 1.0,
      anomalyScore,
      timestamp: new Date().toISOString(),
    };
  }
}
