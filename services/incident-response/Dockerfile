# Incident-response webhook receiver for the AuditLedger runtime-security stack.
# Receives Falco/Alertmanager alerts, logs them, and forwards to external
# incident tooling (Slack / PagerDuty / generic webhook) via env config.
FROM node:20-alpine

WORKDIR /app

COPY services/incident-response/package.json services/incident-response/tsconfig.json ./
RUN npm install

COPY services/incident-response/server.ts ./

EXPOSE 8080

CMD ["npx", "ts-node", "server.ts"]
