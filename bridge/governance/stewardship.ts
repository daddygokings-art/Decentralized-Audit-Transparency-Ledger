/**
 * Data Stewardship Workflow Manager
 *
 * Coordinates data access requests, schema alteration reviews,
 * policy changes, and steward audit logs.
 */

export interface StewardshipChangeRequest {
  id: string;
  assetId: string;
  requester: string;
  type: "ACCESS_GRANT" | "SCHEMA_CHANGE" | "RETENTION_OVERRIDE" | "CLASSIFICATION_UPDATE";
  description: string;
  status: "PENDING" | "APPROVED" | "REJECTED";
  reviewedBy?: string;
  reviewNotes?: string;
  createdAt: number;
  reviewedAt?: number;
}

export class StewardshipWorkflowManager {
  private requests = new Map<string, StewardshipChangeRequest>();

  public createRequest(params: {
    assetId: string;
    requester: string;
    type: StewardshipChangeRequest["type"];
    description: string;
  }): StewardshipChangeRequest {
    const id = `req-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
    const request: StewardshipChangeRequest = {
      id,
      assetId: params.assetId,
      requester: params.requester,
      type: params.type,
      description: params.description,
      status: "PENDING",
      createdAt: Date.now(),
    };

    this.requests.set(id, request);
    return request;
  }

  public reviewRequest(
    id: string,
    steward: string,
    approved: boolean,
    notes: string = ""
  ): StewardshipChangeRequest {
    const request = this.requests.get(id);
    if (!request) {
      throw new Error(`Stewardship request '${id}' not found`);
    }

    request.status = approved ? "APPROVED" : "REJECTED";
    request.reviewedBy = steward;
    request.reviewNotes = notes;
    request.reviewedAt = Date.now();

    return request;
  }

  public listRequests(status?: StewardshipChangeRequest["status"]): StewardshipChangeRequest[] {
    const all = Array.from(this.requests.values());
    if (status) return all.filter((r) => r.status === status);
    return all;
  }
}
