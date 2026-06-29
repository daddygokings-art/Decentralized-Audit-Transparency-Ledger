FROM node:20-alpine

WORKDIR /app

# Copy relayer source
COPY index.ts .
COPY package.json* tsconfig.json* ./

# Install dependencies (if package.json exists)
RUN if [ -f package.json ]; then npm ci --only=production; else npm init -y && npm install --save typescript ts-node @types/node; fi

# Build TypeScript
RUN npx tsc index.ts --outDir . --declaration || npm install -g ts-node && ts-node index.ts --version || true

# Health check (Issue #145): relayer exposes health endpoint on port 8080
HEALTHCHECK --interval=15s --timeout=5s --retries=3 \
  CMD node -e "require('http').get('http://localhost:8080/healthz', (res) => { if (res.statusCode === 200 || res.statusCode === 503) process.exit(0); process.exit(1); }).on('error', () => process.exit(1))"

# Run relayer
CMD ["node", "--loader", "ts-node/esm", "index.ts"]
