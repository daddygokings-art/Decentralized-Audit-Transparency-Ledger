import { TimelineItem } from './types';
import { v4 as uuidv4 } from 'uuid';

export class TimelineTracker {
  private timelines: Map<string, TimelineItem[]> = new Map();

  public addEntry(
    incidentId: string,
    entryType: TimelineItem['entryType'],
    author: string,
    message: string,
    metadata?: Record<string, unknown>
  ): TimelineItem {
    const item: TimelineItem = {
      id: uuidv4(),
      timestamp: new Date().toISOString(),
      entryType,
      author,
      message,
      metadata,
    };

    const list = this.timelines.get(incidentId) || [];
    list.push(item);
    this.timelines.set(incidentId, list);
    return item;
  }

  public getTimeline(incidentId: string): TimelineItem[] {
    return this.timelines.get(incidentId) || [];
  }
}
