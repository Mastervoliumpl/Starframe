import { expect } from '@playwright/test';
import { createConnection } from 'node:net';

/** @param {number} port */
export async function waitForDebugPortRelease(port) {
  await expect
    .poll(
      () =>
        new Promise((accept, reject) => {
          const socket = createConnection({ host: '127.0.0.1', port });
          socket.once('connect', () => {
            socket.destroy();
            accept(true);
          });
          socket.once('error', (error) => {
            socket.destroy();
            if ('code' in error && error.code === 'ECONNREFUSED') accept(false);
            else reject(error);
          });
          socket.setTimeout(1000, () => {
            socket.destroy();
            reject(new Error('Debug port probe timed out.'));
          });
        }),
      {
        timeout: 10000,
        message: `Waiting for debug port ${port} to stop listening`,
      },
    )
    .toBe(false);
}
