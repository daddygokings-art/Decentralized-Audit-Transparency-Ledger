declare module 'crypto' {
  export function randomBytes(size: number): { toString(encoding: string): string };
  export function createHash(algorithm: string): any;
}
declare const process: {
  env: Record<string, string | undefined>;
  stdout: { write(str: string): void };
  stderr: { write(str: string): void };
  hrtime: { bigint(): bigint };
};
declare const require: any;
declare const module: any;
