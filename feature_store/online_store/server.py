"""
High-Speed Online Feature Serving API (#524)
"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import json
from urllib.parse import urlparse, parse_qs
from .redis_store import RedisOnlineStore

store = RedisOnlineStore()

class FeatureServingHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path == "/get-online-features":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length).decode("utf-8")
            data = json.loads(body)

            view_name = data.get("view_name", "submitter_behavior_v1")
            entity_keys = data.get("entities", [])

            results = {}
            for entity_id in entity_keys:
                feats = store.get_features(entity_id, view_name)
                results[entity_id] = feats or {}

            response_data = {"status": "ok", "features": results}
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(response_data).encode("utf-8"))
            return

        if self.path == "/healthz":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"status": "healthy"}).encode("utf-8"))
            return

        self.send_response(404)
        self.end_headers()

def run_server(port: int = 8000):
    server = HTTPServer(("0.0.0.0", port), FeatureServingHandler)
    print(f"Online Feature Store HTTP server listening on port {port}")
    server.serve_forever()
