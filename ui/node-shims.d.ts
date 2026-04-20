declare module "node:child_process" {
  export const execFile: any;
  export const execFileSync: any;
}

declare module "node:fs" {
  const fs: any;
  export default fs;
}

declare module "node:os" {
  const os: any;
  export default os;
}

declare module "node:path" {
  const path: any;
  export default path;
}

declare module "node:url" {
  export const fileURLToPath: any;
}

declare module "node:util" {
  export const promisify: any;
}
