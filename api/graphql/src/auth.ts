export enum Role {
  Viewer = "viewer",
  Auditor = "auditor",
  Admin = "admin",
}

const ROLE_HIERARCHY: Record<Role, number> = {
  [Role.Viewer]: 1,
  [Role.Auditor]: 2,
  [Role.Admin]: 3,
};

export function hasMinRole(userRole: Role | undefined | null, requiredRole: Role): boolean {
  if (!userRole) return false;
  const userLevel = ROLE_HIERARCHY[userRole] ?? 0;
  const requiredLevel = ROLE_HIERARCHY[requiredRole];
  return userLevel >= requiredLevel;
}

export function requireRole(ctx: { role?: Role } | undefined | null, requiredRole: Role): void {
  if (!ctx || !hasMinRole(ctx.role, requiredRole)) {
    throw new Error(`Forbidden: requires ${requiredRole} role or higher`);
  }
}
