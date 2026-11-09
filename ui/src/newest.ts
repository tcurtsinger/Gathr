import { useState } from 'react';

/**
 * Requests to the service where the newest wins: the answer of one that started before a later one
 * never settles, so a slow, older answer can't overwrite what a newer change showed. A value known
 * now (a live update) counts too: pass it as `Promise.resolve(value)`.
 */
export function newestWins() {
  let latest = 0;
  return <T>(request: Promise<T>): Promise<T> => {
    const n = ++latest;
    const stale = new Promise<T>(() => {});
    return request.then(
      (value) => (n === latest ? value : stale),
      (error) => (n === latest ? Promise.reject(error) : stale),
    );
  };
}

/** One `newestWins` for a component's life. */
export function useNewestWins() {
  return useState(newestWins)[0];
}
