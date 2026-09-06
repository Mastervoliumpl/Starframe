import { render } from 'svelte/server';
import { expect, test } from 'vitest';
import App from './App.svelte';
import fixture from './fixtures/shell.json';

test('the Svelte shell compiles and renders its landmarks and build status', () => {
  const { body } = render(App);
  for (const landmark of fixture.landmarks) expect(body).toContain(landmark);
  expect(body).toContain(fixture.status);
});
