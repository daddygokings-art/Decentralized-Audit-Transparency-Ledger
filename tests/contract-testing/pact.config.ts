import * as path from 'path';

export const pactConfig = {
  // Provider configuration
  providerName: 'AuditLedgerRestAPI',
  providerPort: parseInt(process.env.PROVIDER_PORT || '3002', 10),
  providerHost: process.env.PROVIDER_HOST || '127.0.0.1',
  get providerBaseUrl() {
    return `http://${this.providerHost}:${this.providerPort}`;
  },

  // Consumers
  consumers: {
    webUI: 'AuditLedgerWebUI',
    sdk: 'AuditLedgerSDK',
    bridgeRelayer: 'BridgeRelayer'
  },

  // Directory paths
  pactDir: path.resolve(__dirname, 'pacts'),
  logDir: path.resolve(__dirname, 'logs'),

  // Pact Broker Settings
  broker: {
    pactBrokerUrl: process.env.PACT_BROKER_BASE_URL || process.env.PACT_BROKER_URL || 'http://localhost:9292',
    pactBrokerToken: process.env.PACT_BROKER_TOKEN || '',
    pactBrokerUsername: process.env.PACT_BROKER_USERNAME || '',
    pactBrokerPassword: process.env.PACT_BROKER_PASSWORD || '',
    publishVerificationResult: process.env.CI === 'true',
  },

  // Git / CI version metadata
  version: {
    consumerVersion: process.env.GIT_COMMIT || process.env.GITHUB_SHA || `1.0.0-${Date.now()}`,
    providerVersion: process.env.GIT_COMMIT || process.env.GITHUB_SHA || `1.0.0-${Date.now()}`,
    branch: process.env.GIT_BRANCH || process.env.GITHUB_REF_NAME || 'master',
    environment: process.env.TARGET_ENV || 'staging'
  }
};
