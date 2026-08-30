declare module 'crypto' {
  export function createHash(algorithm: string): any;
}
declare const process: {
  env: Record<string, string | undefined>;
  argv: string[];
  exit(code?: number): never;
};
declare const require: {
  (id: string): any;
  main: any;
};
declare const module: any;
