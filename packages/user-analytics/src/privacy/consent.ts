import { ConsentCategory, ConsentPreferences } from '../types';

export class ConsentManager {
  private preferences: Map<string, ConsentPreferences> = new Map();

  /**
   * Evaluates incoming request headers for Do Not Track (DNT) or Global Privacy Control (GPC).
   */
  public static isDntOrGpcEnabled(headers: Record<string, string | undefined>): boolean {
    const dnt = headers['dnt'] || headers['DNT'];
    const gpc = headers['sec-gpc'] || headers['Sec-GPC'];
    return dnt === '1' || gpc === '1';
  }

  public setConsent(
    anonymousId: string,
    optedIn: boolean,
    categories: ConsentCategory[] = ['necessary', 'analytics'],
    dntHeaderHonored = false
  ): ConsentPreferences {
    const pref: ConsentPreferences = {
      anonymousId,
      optedIn,
      categories: optedIn ? categories : ['necessary'],
      dntHeaderHonored,
      updatedAt: new Date().toISOString(),
    };
    this.preferences.set(anonymousId, pref);
    return pref;
  }

  public getConsent(anonymousId: string): ConsentPreferences | undefined {
    return this.preferences.get(anonymousId);
  }

  public hasConsent(anonymousId: string, category: ConsentCategory = 'analytics'): boolean {
    if (category === 'necessary') return true;
    const pref = this.preferences.get(anonymousId);
    if (!pref) {
      // Default: opt-in required for analytics under strict privacy mode
      return false;
    }
    return pref.optedIn && pref.categories.includes(category);
  }

  public optOut(anonymousId: string): void {
    this.setConsent(anonymousId, false, ['necessary']);
  }

  public deleteConsent(anonymousId: string): boolean {
    return this.preferences.delete(anonymousId);
  }

  public getStats(): { totalUsers: number; optedIn: number; optedOut: number; optInRatePct: number } {
    let optedIn = 0;
    let optedOut = 0;
    for (const p of this.preferences.values()) {
      if (p.optedIn) optedIn++;
      else optedOut++;
    }
    const totalUsers = this.preferences.size;
    const optInRatePct = totalUsers > 0 ? Number(((optedIn / totalUsers) * 100).toFixed(2)) : 0;
    return { totalUsers, optedIn, optedOut, optInRatePct };
  }
}
