import { Incident, PostmortemReport } from './types';

export class PostmortemGenerator {
  public static generateTemplate(
    incident: Incident,
    investigator: string,
    executiveSummary: string,
    rootCause: string,
    fiveWhys: string[],
    actionItems: PostmortemReport['actionItems'],
    whatWentWell: string[],
    whatWentWrong: string[],
    whereWeGotLucky: string[]
  ): PostmortemReport {
    const createdAt = new Date(incident.createdAt).getTime();
    const ackAt = incident.acknowledgedAt ? new Date(incident.acknowledgedAt).getTime() : createdAt;
    const resAt = incident.resolvedAt ? new Date(incident.resolvedAt).getTime() : Date.now();

    const durationMinutes = Math.max(1, Math.round((resAt - createdAt) / (1000 * 60)));
    const timeToAcknowledgeMinutes = Math.max(0, Math.round((ackAt - createdAt) / (1000 * 60)));
    const timeToResolveMinutes = Math.max(1, Math.round((resAt - ackAt) / (1000 * 60)));

    return {
      incidentId: incident.id,
      title: incident.title,
      severity: incident.severity,
      incidentCommander: incident.commander || 'Unassigned',
      leadInvestigator: investigator,
      date: new Date().toISOString().split('T')[0],
      durationMinutes,
      timeToAcknowledgeMinutes,
      timeToResolveMinutes,
      executiveSummary,
      impactAnalysis: {
        contractEventsDropped: 0,
        financialImpactUsd: 0,
        affectedContracts: incident.contractAddress ? [incident.contractAddress] : ['Core AuditLedger'],
        affectedSubsystems: [incident.source],
      },
      rootCauseAnalysis: {
        primaryRootCause: rootCause,
        contributingFactors: [],
        fiveWhys,
      },
      timelineSummary: incident.timeline,
      actionItems,
      lessonsLearned: {
        whatWentWell,
        whatWentWrong,
        whereWeGotLucky,
      },
    };
  }

  public static formatToMarkdown(pm: PostmortemReport): string {
    return `# Blameless Postmortem: ${pm.title}

**Incident ID**: \`${pm.incidentId}\`  
**Severity**: \`${pm.severity}\`  
**Date**: ${pm.date}  
**Incident Commander**: ${pm.incidentCommander}  
**Lead Investigator**: ${pm.leadInvestigator}  

---

## 1. Metrics & Duration
- **Total Duration**: ${pm.durationMinutes} minutes
- **Mean Time to Acknowledge (MTTA)**: ${pm.timeToAcknowledgeMinutes} minutes
- **Mean Time to Resolve (MTTR)**: ${pm.timeToResolveMinutes} minutes

---

## 2. Executive Summary
${pm.executiveSummary}

---

## 3. Impact Assessment
- **Affected Subsystems**: ${pm.impactAnalysis.affectedSubsystems.join(', ')}
- **Affected Contracts**: ${pm.impactAnalysis.affectedContracts.join(', ')}
- **Dropped Events**: ${pm.impactAnalysis.contractEventsDropped}
- **Financial Impact**: $${pm.impactAnalysis.financialImpactUsd.toFixed(2)} USD

---

## 4. Root Cause Analysis (5 Whys)
**Primary Root Cause**: ${pm.rootCauseAnalysis.primaryRootCause}

### The 5 Whys:
${pm.rootCauseAnalysis.fiveWhys.map((why, idx) => `${idx + 1}. ${why}`).join('\n')}

---

## 5. Timeline of Events
| Timestamp | Event Type | Author | Note |
|---|---|---|---|
${pm.timelineSummary.map((t) => `| ${t.timestamp} | \`${t.entryType}\` | ${t.author} | ${t.message} |`).join('\n')}

---

## 6. Lessons Learned

### What Went Well
${pm.lessonsLearned.whatWentWell.map((w) => `- ${w}`).join('\n')}

### What Went Wrong
${pm.lessonsLearned.whatWentWrong.map((w) => `- ${w}`).join('\n')}

### Where We Got Lucky
${pm.lessonsLearned.whereWeGotLucky.map((w) => `- ${w}`).join('\n')}

---

## 7. Action Items
| ID | Action Item | Owner | Target Date | Status |
|---|---|---|---|---|
${pm.actionItems.map((a) => `| ${a.id} | ${a.description} | ${a.owner} | ${a.dueDate} | \`${a.status}\` |`).join('\n')}
`;
  }
}
