const express = require('express');
const bodyParser = require('body-parser');
const WebSocket = require('ws');

const app = express();
app.use(bodyParser.json());

const server = require('http').createServer(app);
const wss = new WebSocket.Server({ server });

// Map of ws -> Set of subscribed types (string). Empty set = subscribed all
const subs = new Map();

wss.on('connection', (ws) => {
  subs.set(ws, new Set());
  ws.isAlive = true;
  ws.on('pong', () => ws.isAlive = true);

  ws.on('message', (msg) => {
    try {
      const data = JSON.parse(msg);
      if (data.action === 'subscribe') {
        subs.get(ws).add(data.type);
      } else if (data.action === 'subscribe_all') {
        subs.get(ws).clear();
      } else if (data.action === 'unsubscribe') {
        subs.get(ws).delete(data.type);
      }
    } catch (e) {
      // ignore
    }
  });

  ws.on('close', () => subs.delete(ws));
});

function broadcastEvent(evt) {
  for (const [ws, set] of subs.entries()) {
    if (ws.readyState !== WebSocket.OPEN) continue;
    // if set empty => subscribed_all
    if (set.size === 0 || set.has(evt.event_type)) {
      ws.send(JSON.stringify({ type: 'event_logged', event: evt }));
    }
  }
}

// Simple HTTP emit endpoint for testing: POST /emit { event }
app.post('/emit', (req, res) => {
  const evt = req.body.event;
  if (!evt) return res.status(400).send('no event');
  broadcastEvent(evt);
  res.send('ok');
});

// Health
app.get('/health', (req, res) => res.json({ ok: true }));

// Periodic ping for connection health
setInterval(() => {
  wss.clients.forEach((ws) => {
    if (ws.isAlive === false) return ws.terminate();
    ws.isAlive = false;
    ws.ping();
  });
}, 30000);

const PORT = process.env.PORT || 4000;
server.listen(PORT, () => console.log('ws server listening', PORT));
