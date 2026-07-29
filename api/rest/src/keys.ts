import { randomBytes } from "crypto";

export type Role = "viewer" | "auditor" | "admin";

export interface ApiKeyRecord {
  key: string;
  name: string;
  createdAt: number;
  active: boolean;
  role: Role;
}

const keys = new Map<string, ApiKeyRecord>();

const ENV_KEY = process.env.API_KEY;
if (ENV_KEY) {
  keys.set(ENV_KEY, {
    key: ENV_KEY,
    name: "env-default",
    createdAt: Date.now(),
    active: true,
    role: (process.env.API_KEY_ROLE as Role) ?? "admin",
  });
}

export function generateKey(name: string, role: Role = "viewer"): ApiKeyRecord {
  const key = "alg_" + randomBytes(32).toString("hex");
  const record: ApiKeyRecord = {
    key,
    name,
    createdAt: Date.now(),
    active: true,
    role,
  };
  keys.set(key, record);
  return record;
}

export function validateKey(key: string): ApiKeyRecord | null {
  const record = keys.get(key);
  if (record && record.active) return record;
  return null;
}

export function rotateKey(oldKey: string): ApiKeyRecord | null {
  const old = keys.get(oldKey);
  if (!old || !old.active) return null;
  old.active = false;
  return generateKey(old.name + "-rotated", old.role);
}

export function revokeKey(key: string): boolean {
  const record = keys.get(key);
  if (!record) return false;
  record.active = false;
  return true;
}

export function listKeys(): ApiKeyRecord[] {
  return Array.from(keys.values());
}
