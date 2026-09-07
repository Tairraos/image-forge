import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { readSessions, updateSession, writeSession } from '../../src/api/localStore.js';

vi.mock('../../src/api/blob.js', () => ({ isLocalDev: () => false }));

beforeEach(() => {
  const values = new Map();
  vi.stubGlobal('localStorage', {
    getItem: (key) => values.get(key) || null,
    setItem: (key, value) => values.set(key, value),
  });
});
afterEach(() => vi.unstubAllGlobals());

it('同一会话的并发读改写保留双方消息，失败后仍可继续保存', async () => {
  await writeSession({ id: 'session', messages: [] });
  let release;
  const gate = new Promise((resolve) => {
    release = resolve;
  });
  const first = updateSession('session', async (session) => {
    await gate;
    session.messages.push({ id: 'reply' });
    return session;
  });
  const second = updateSession('session', (session) => {
    session.messages.push({ id: 'image' });
    return session;
  });
  release();
  await Promise.all([first, second]);
  expect((await readSessions())[0].messages.map((message) => message.id)).toEqual([
    'reply',
    'image',
  ]);

  await expect(
    updateSession('session', () => {
      throw new Error('保存失败');
    })
  ).rejects.toThrow('保存失败');
  await updateSession('session', (session) => ({ ...session, title: '恢复保存' }));
  expect((await readSessions())[0].title).toBe('恢复保存');
});
