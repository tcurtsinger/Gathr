const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const { pathToFileURL } = require('node:url');
const { chromium } = require('playwright-core');

// Uses an already installed browser and Playwright, never downloads either.
// NODE_PATH supplies playwright-core if it is installed outside this checkout.
async function check() {
  const browser = await chromium.launch({ executablePath: process.env.BROWSER_PATH, headless: true });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    for (const file of ['chat-states/v3.html', 'diagnostics/v3.html', 'engines/v4.html'])
      for (const width of [1440, 1024]) for (const theme of ['dark', 'light']) {
      await page.setViewportSize({ width, height: 1000 });
      const url = pathToFileURL(path.join(__dirname, file)).href;
      await page.goto('about:blank');
      await page.goto(url + (theme === 'light' ? '#light' : ''));
      assert.equal(await page.locator('html').evaluate(html => html.classList.contains('light')), theme === 'light', 'requested theme');
      if (await page.locator('.settings-panel').evaluate(dialog => dialog.open)) await page.locator('#close-settings').click();
      await page.locator('#chat-preview').selectOption('Error');
      const context = `${file} ${width} ${theme}`;
      const diagnostics = page.locator('.chat-stage .notice a', { hasText: 'Diagnostics' });
      if (process.argv[2] !== 'focus') {
        await page.locator('.topbar a', { hasText: 'Diagnostics' }).click();
        await page.locator('#filter').selectOption('Stopped');
        await page.locator('#close-settings').click();
        for (const opener of [diagnostics, page.locator('.chat-stage .notice .ref')]) {
          await opener.click();
          assert.equal(await page.locator('#trace h2').count(), 1, `${context}: contextual reference must show a turn, not an empty result`);
          assert.equal(await page.locator('#trace h2').textContent(), 'Turn 46', `${context}: contextual reference must show Turn 46`);
          assert.equal(await page.locator('#filter').inputValue(), 'All', `${context}: contextual reference must clear status filter`);
          if (process.env.EVIDENCE_DIR) {
            fs.mkdirSync(process.env.EVIDENCE_DIR, { recursive: true });
            await page.screenshot({ path: path.join(process.env.EVIDENCE_DIR, `${file.split('/')[0]}-${width}-${theme}.png`) });
          }
          await page.locator('#find').fill('');
          assert.equal(await page.locator('.turnpick').count(), 48, `${context}: clearing Find restores 48 turns`);
          await page.locator('[data-turn="48"]').click();
          assert.equal(await page.locator('#trace .tools .event').count(), 50, `${context}: fifty tool calls remain available`);
          await page.locator('#filter').selectOption('Stopped');
          await page.locator('#close-settings').click();
        }
      }
      if (process.argv[2] !== 'filter') {
        for (const key of ['Close', 'Escape']) {
          await diagnostics.click();
          await page.locator('[data-category="engines"]').click();
          if (key === 'Close') await page.locator('#close-settings').click();
          else await page.keyboard.press('Escape');
          await page.waitForFunction(() => !document.querySelector('.settings-panel').open
            && document.activeElement.matches('.chat-stage .notice a'), null, { timeout: 2000 });
          assert(await diagnostics.evaluate(link => document.activeElement === link), `${context}: ${key} restores original reply link`);
        }
      }
      console.log(`PASS Settings ${process.argv[2] || 'all'}: ${context}`);
    }
    assert.deepEqual(errors, [], 'no browser errors');
  } finally { await browser.close(); }
}
check().catch(error => { console.error(error); process.exitCode = 1; });
