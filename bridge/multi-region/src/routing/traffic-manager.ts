import { RegionHealth, RegionIdentifier, RegionalNodeConfig, TrafficRoutingDecision } from '../types';

export class GlobalTrafficManager {
  private nodes: Map<RegionIdentifier, RegionalNodeConfig> = new Map();

  constructor() {
    this.seedDefaultNodes();
  }

  private seedDefaultNodes() {
    const usNode: RegionalNodeConfig = {
      region: 'us-east-1',
      endpointUrl: 'https://us-east.audit-ledger.io',
      isPrimary: true,
      health: 'HEALTHY',
      lastHeartbeat: new Date().toISOString(),
      processedLedgerSeq: 104500,
      stateRootHash: '0xus_east_stateroot_hash_98231023',
      trafficWeight: 60,
      latencyMs: 15,
    };

    const euNode: RegionalNodeConfig = {
      region: 'eu-central-1',
      endpointUrl: 'https://eu-central.audit-ledger.io',
      isPrimary: false,
      health: 'HEALTHY',
      lastHeartbeat: new Date().toISOString(),
      processedLedgerSeq: 104498,
      stateRootHash: '0xeu_central_stateroot_hash_98231023',
      trafficWeight: 30,
      latencyMs: 22,
    };

    const apNode: RegionalNodeConfig = {
      region: 'ap-southeast-1',
      endpointUrl: 'https://ap-southeast.audit-ledger.io',
      isPrimary: false,
      health: 'HEALTHY',
      lastHeartbeat: new Date().toISOString(),
      processedLedgerSeq: 104495,
      stateRootHash: '0xap_southeast_stateroot_hash_98231023',
      trafficWeight: 10,
      latencyMs: 38,
    };

    this.nodes.set(usNode.region, usNode);
    this.nodes.set(euNode.region, euNode);
    this.nodes.set(apNode.region, apNode);
  }

  public routeClient(clientIp: string, clientCountry: string): TrafficRoutingDecision {
    const isEu = ['DE', 'FR', 'GB', 'NL', 'SE', 'IT', 'ES'].includes(clientCountry.toUpperCase());
    const isAp = ['SG', 'JP', 'KR', 'AU', 'IN', 'SG'].includes(clientCountry.toUpperCase());

    let targetRegion: RegionIdentifier = 'us-east-1';
    let routingReason: TrafficRoutingDecision['routingReason'] = 'GEO_PROXIMITY';

    if (isEu) targetRegion = 'eu-central-1';
    else if (isAp) targetRegion = 'ap-southeast-1';

    const targetNode = this.nodes.get(targetRegion);
    if (!targetNode || targetNode.health !== 'HEALTHY') {
      // Failover routing to primary or any healthy node
      const healthyNode = Array.from(this.nodes.values()).find((n) => n.health === 'HEALTHY');
      targetRegion = healthyNode ? healthyNode.region : 'us-east-1';
      routingReason = 'FAILOVER_BACKUP';
    }

    const node = this.nodes.get(targetRegion);
    return {
      clientIp,
      clientCountry,
      routedRegion: targetRegion,
      routingReason,
      estimatedLatencyMs: node ? node.latencyMs : 25,
    };
  }

  public setNodeHealth(region: RegionIdentifier, health: RegionHealth) {
    const node = this.nodes.get(region);
    if (node) {
      node.health = health;
      node.lastHeartbeat = new Date().toISOString();
    }
  }

  public getNode(region: RegionIdentifier): RegionalNodeConfig | undefined {
    return this.nodes.get(region);
  }

  public getAllNodes(): RegionalNodeConfig[] {
    return Array.from(this.nodes.values());
  }

  public getPrimaryNode(): RegionalNodeConfig | undefined {
    return Array.from(this.nodes.values()).find((n) => n.isPrimary);
  }
}
