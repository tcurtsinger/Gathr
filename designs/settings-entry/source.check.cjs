const assert = require('node:assert/strict');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { chromium } = require(process.env.GATHR_PLAYWRIGHT || path.resolve(__dirname, '../../ui/node_modules/playwright'));

(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  try {
    for (const version of ['now', 'v1', 'v2', 'v3']) {
      for (const width of [1440, 1024, 390]) {
        for (const theme of ['', '#light']) {
          const page = await browser.newPage({ viewport: { width, height: 900 } });
          await page.goto(pathToFileURL(path.join(__dirname, `${version}.html`)).href + theme);
          if (width >= 1024) {
            await page.locator('#search-toggle').click();
            await page.locator('#chat-search').fill('keyboard');
            assert.equal(await page.locator('.chatrow:visible').count(), 1, `${version}: Search filters chats`);
            await page.locator('#chat-search').fill('');
            assert.equal(await page.locator('.chatrow:visible').count(), 3);
          }
          await page.locator(version === 'now' ? '.old-entry' : '[data-id="settings-entry"]').click();
          if (version === 'v2') await page.getByRole('button', { name: 'Settings', exact: true }).click();
          assert.equal(await page.locator('#settings').isVisible(), true);
          if (process.env.GATHR_EVIDENCE) {
            const name = `${version}-${width}-${theme ? 'light' : 'dark'}.png`;
            await page.screenshot({ path: path.join(process.env.GATHR_EVIDENCE, name) });
          }
          await page.keyboard.press('Escape');
          assert.equal(await page.locator('#settings').isVisible(), false);
          await page.close();
        }
      }
    }
    console.log('Four pages: Search at desktop widths, Settings and Escape at 1440/1024/390, dark/light passed');
    for (const version of [3, 2]) {
      for (const width of [1440, 1024, 390]) for (const theme of ['', '#light']) {
      const page = await browser.newPage({ viewport: { width, height: 900 } });
      await page.goto(pathToFileURL(path.join(__dirname, `v${version}.html`)).href + theme);
      await page.locator('[data-id="settings-entry"]').click();
      if (version === 2) await page.getByRole('button', { name: 'Settings', exact: true }).click();
      if (version === 3) {
        await page.locator('summary').filter({ hasText: 'Engine source' }).click();
        await page.locator('#custom-choice').check();
      } else await page.locator('#custom-open').click();
      await page.locator('#custom-path').fill('C:\\sample\\claude.exe');
      await page.locator('#save-path').click();
      assert.equal(await page.locator('.engine-heading span').textContent(), 'Custom engine');
      if (version === 2) {
        assert.match(await page.locator('.setting-row').filter({ hasText: 'Engine source' }).innerText(), /Custom/);
        await page.locator('#custom-path').fill('');
        await page.locator('#save-path').click();
        assert.match(await page.locator('.setting-row').filter({ hasText: 'Engine source' }).innerText(), /Gathr/);
      } else await page.locator('#managed-choice').check();
      assert.equal(await page.locator('.engine-heading span').textContent(), 'Managed by Gathr');
      assert.equal(await page.locator('#engine-status').textContent(), 'Up to date');
      assert.equal(await page.locator('#update').isEnabled(), true);
      if (process.env.GATHR_EVIDENCE) {
        const name = `v${version}-restored-${width}-${theme ? 'light' : 'dark'}.png`;
        await page.screenshot({ path: path.join(process.env.GATHR_EVIDENCE, name) });
      }
      console.log(`Option ${version}: custom source and return to managed passed`);
      await page.close();
      }
    }
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
