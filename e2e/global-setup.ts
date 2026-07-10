import { ChildProcess, spawn } from 'node:child_process';
import { cpSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Spawn the REAL shipped binary against a throwaway copy of the example
// episode. Managed here (not playwright's webServer) so the fixture dir
// provably exists before the server starts.

let server: ChildProcess;

export default async function globalSetup() {
  const episode = join(tmpdir(), 'videoeditor-record-e2e');
  rmSync(episode, { recursive: true, force: true });
  cpSync(join(__dirname, '..', 'examples', 'hello-bench'), episode, { recursive: true });

  const env = { ...process.env };
  // hermetic + deterministic: level coaching only — no ElevenLabs STT
  // calls, and no local whisper either (it hallucinates words on the
  // fake mic's tone, which would make coaching output flaky)
  delete env.ELEVENLABS_API_KEY;
  delete env.WHISPER_MODEL;

  const bin = join(__dirname, '..', 'target', 'debug', 'videoeditor');
  server = spawn(bin, ['record', episode, '--port', '4901', '--no-open'], {
    env,
    stdio: 'inherit',
  });
  server.on('exit', (code) => {
    if (code !== null && code !== 0) console.error(`recorder server exited with ${code}`);
  });

  for (let i = 0; i < 100; i++) {
    try {
      const res = await fetch('http://127.0.0.1:4901/api/episode');
      if (res.ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error('recorder server never became ready on :4901');
}

export async function teardown() {
  server?.kill();
}
