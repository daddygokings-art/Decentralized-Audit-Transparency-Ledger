import { OnCallShift, OnCallUser } from './types';

export class OnCallScheduler {
  private shifts: Map<string, OnCallShift[]> = new Map();
  private users: Map<string, OnCallUser> = new Map();

  constructor() {
    this.seedDefaultRoster();
  }

  private seedDefaultRoster() {
    const primaryEng: OnCallUser = {
      id: 'eng-101',
      name: 'Alice Vanguard',
      email: 'alice.v@example.com',
      phone: '+1-555-0101',
      timezone: 'America/New_York',
      role: 'PRIMARY',
    };

    const secondaryEng: OnCallUser = {
      id: 'eng-102',
      name: 'Bob Horizon',
      email: 'bob.h@example.com',
      phone: '+1-555-0102',
      timezone: 'Europe/London',
      role: 'SECONDARY',
    };

    const leadEng: OnCallUser = {
      id: 'eng-103',
      name: 'Carol Sentinel',
      email: 'carol.s@example.com',
      phone: '+1-555-0103',
      timezone: 'Asia/Singapore',
      role: 'LEAD',
    };

    this.users.set(primaryEng.id, primaryEng);
    this.users.set(secondaryEng.id, secondaryEng);
    this.users.set(leadEng.id, leadEng);

    const defaultShift: OnCallShift = {
      id: 'shift-core-001',
      team: 'audit-ledger-core',
      primary: primaryEng,
      secondary: secondaryEng,
      startTime: new Date(Date.now() - 3 * 86400000).toISOString(),
      endTime: new Date(Date.now() + 4 * 86400000).toISOString(),
      rotationType: 'WEEKLY',
    };

    this.shifts.set('audit-ledger-core', [defaultShift]);
  }

  public getCurrentShift(team: string = 'audit-ledger-core'): OnCallShift | undefined {
    const teamShifts = this.shifts.get(team);
    if (!teamShifts || teamShifts.length === 0) return undefined;
    const now = new Date();
    return teamShifts.find((s) => new Date(s.startTime) <= now && now <= new Date(s.endTime)) || teamShifts[0];
  }

  public getActivePrimary(team: string = 'audit-ledger-core'): OnCallUser | undefined {
    return this.getCurrentShift(team)?.primary;
  }

  public getActiveSecondary(team: string = 'audit-ledger-core'): OnCallUser | undefined {
    return this.getCurrentShift(team)?.secondary;
  }

  public setShiftOverride(team: string, primaryUser: OnCallUser, durationHours: number = 8): OnCallShift {
    const now = new Date();
    const endTime = new Date(now.getTime() + durationHours * 3600000);
    const existing = this.getCurrentShift(team);

    const overrideShift: OnCallShift = {
      id: `override-${Date.now()}`,
      team,
      primary: primaryUser,
      secondary: existing ? existing.secondary : primaryUser,
      startTime: now.toISOString(),
      endTime: endTime.toISOString(),
      rotationType: 'DAILY',
    };

    const list = this.shifts.get(team) || [];
    list.unshift(overrideShift);
    this.shifts.set(team, list);
    return overrideShift;
  }

  public getAllUsers(): OnCallUser[] {
    return Array.from(this.users.values());
  }
}
