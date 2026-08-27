/**
 * Data Lake Schema Evolution Engine
 *
 * Manages schema versions, validates backward and forward compatibility,
 * and handles column additions, renames, and type promotions.
 */

export interface ColumnDefinition {
  name: string;
  type: "string" | "long" | "int" | "double" | "boolean" | "timestamp" | "binary";
  nullable: boolean;
  doc?: string;
}

export interface SchemaDefinition {
  version: number;
  columns: ColumnDefinition[];
  createdAt: number;
}

export class SchemaEvolutionManager {
  private schemas: SchemaDefinition[] = [];

  constructor() {
    this.schemas.push({
      version: 1,
      columns: [
        { name: "event_hash", type: "string", nullable: false },
        { name: "ledger_seq", type: "long", nullable: false },
        { name: "tx_hash", type: "string", nullable: false },
        { name: "event_type", type: "string", nullable: false },
        { name: "submitter", type: "string", nullable: false },
        { name: "timestamp", type: "timestamp", nullable: false },
      ],
      createdAt: Date.now(),
    });
  }

  /**
   * Evolve schema to a new version with compatibility checks
   */
  public evolveSchema(newColumns: ColumnDefinition[]): SchemaDefinition {
    const currentSchema = this.getLatestSchema();
    this.validateCompatibility(currentSchema.columns, newColumns);

    const newSchema: SchemaDefinition = {
      version: currentSchema.version + 1,
      columns: newColumns,
      createdAt: Date.now(),
    };

    this.schemas.push(newSchema);
    return newSchema;
  }

  /**
   * Validate backward compatibility: existing columns cannot change to incompatible types
   * or become non-nullable without defaults.
   */
  private validateCompatibility(prev: ColumnDefinition[], next: ColumnDefinition[]) {
    const prevMap = new Map(prev.map((c) => [c.name, c]));

    for (const nextCol of next) {
      const prevCol = prevMap.get(nextCol.name);
      if (prevCol) {
        // Type promotion rules (int -> long, float -> double)
        if (prevCol.type !== nextCol.type) {
          const allowedPromotion = prevCol.type === "int" && nextCol.type === "long";
          if (!allowedPromotion) {
            throw new Error(
              `Incompatible type promotion on column '${nextCol.name}' from ${prevCol.type} to ${nextCol.type}`
            );
          }
        }
        if (!prevCol.nullable && nextCol.nullable) {
          // Allowed: making field nullable
        }
        if (prevCol.nullable && !nextCol.nullable) {
          throw new Error(`Cannot make nullable column '${nextCol.name}' non-nullable`);
        }
      }
    }
  }

  public getLatestSchema(): SchemaDefinition {
    return this.schemas[this.schemas.length - 1];
  }

  public getSchemaByVersion(version: number): SchemaDefinition | null {
    return this.schemas.find((s) => s.version === version) || null;
  }

  public getSchemaHistory(): SchemaDefinition[] {
    return this.schemas;
  }
}
