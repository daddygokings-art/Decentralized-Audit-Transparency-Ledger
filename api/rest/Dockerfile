FROM node:20-alpine

WORKDIR /app

COPY api/rest/package.json api/rest/tsconfig.json ./
COPY api/graphql/src ./graphql/src

RUN npm install

COPY api/rest/src ./src

RUN npm run build

EXPOSE 3002

CMD ["npm", "start"]
