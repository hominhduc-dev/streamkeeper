import { mkdirSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
mkdirSync('.local', { recursive: true });
const result = spawnSync('cargo', ['run', '-p', 'video-domain', '--bin', 'export-types', '--', '.local/contracts.ts'], { stdio: 'inherit' });
if (result.status !== 0) process.exit(result.status ?? 1);
const normalize = (path) => readFileSync(path, 'utf8').replace(/\s/g, '').replace(/;/g, ',');
if (normalize('.local/contracts.ts') !== normalize('packages/contracts/src/index.ts')) {
  console.error('Rust/TypeScript contracts differ. Run pnpm contracts and review the generated DTO.');
  process.exit(1);
}
console.log('Rust/TypeScript contracts match.');
