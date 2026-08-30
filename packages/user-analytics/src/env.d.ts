declare module 'crypto' {
  export function createHmac(algorithm: string, key: string): any;
  export function randomBytes(size: number): { toString(encoding: string): string };
  export function createHash(algorithm: string): any;
}
declare const process: {
  env: Record<string, string | undefined>;
};
declare const require: any;
declare const module: any;
