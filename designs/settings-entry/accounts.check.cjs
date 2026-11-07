const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.GATHR_PLAYWRIGHT || path.resolve(__dirname, '../../ui/node_modules/playwright'));
(async () => {
  const browser = await chromium.launch({channel:'msedge', headless:true});
  const evidence = process.env.GATHR_EVIDENCE;
  if (evidence) fs.mkdirSync(evidence, {recursive:true});
  try {
    for (const width of [1440, 1024, 390]) for (const theme of ['', '#light']) {
      const page = await browser.newPage({viewport:{width, height:900}});
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      await page.goto(pathToFileURL(path.join(__dirname, 'v4.html')).href + theme);
      assert.equal(await page.locator('.account-row').count(), 2);
      assert.equal(await page.locator('#custom-path').count(), 0);
      if (evidence) await page.screenshot({path:path.join(evidence, `accounts-${width}-${theme?'light':'dark'}.png`)});
      await page.keyboard.press('Escape');
      await page.locator('#draft').fill('Keep this draft');
      await page.locator('.entry').click();
      await page.locator('#add-account').click();
      await page.locator('#account-engine').selectOption('Codex');
      await page.locator('#account-name').fill('Research');
      await page.locator('#save-account').click();
      const added = page.locator('[data-id="account-row-3"]');
      assert.match(await added.innerText(), /Codex \/ Signing in/);
      await added.getByRole('button', {name:'Cancel', exact:true}).click();
      assert.match(await added.innerText(), /Signed out/);
      await added.getByRole('button', {name:'Sign in', exact:true}).click();
      await added.getByRole('button', {name:'Simulate success'}).click();
      assert.match(await added.innerText(), /Signed in/);
      await added.getByRole('button', {name:'Rename'}).click();
      assert.equal(await page.locator('#account-engine').isDisabled(), true);
      await page.locator('#account-name').fill('Work research');
      await page.locator('#save-account').click();
      assert.match(await added.innerText(), /Work research/);
      await added.getByRole('button', {name:'Sign out'}).click();
      assert.match(await added.innerText(), /Signed out/);
      assert.match(await page.locator('[data-id="account-row-1"]').innerText(), /Signed in/);
      // Real-size names and account lists must scroll without widening the dialog.
      await page.evaluate(() => {
        for (let i=4;i<=15;i++) accountFixtures.push({id:i,name:'Regional research and release account '+i,engine:'Claude Code',signedIn:false});
        renderAccounts();
      });
      assert.equal(await page.locator('.account-row').count(), 15);
      assert.equal(await page.evaluate(() => document.querySelector('.settings-page').scrollWidth <= document.querySelector('.settings-page').clientWidth), true);
      await page.locator('[data-page="agents"]').click();
      assert.equal(await page.locator('select[name="engine"]').isVisible(), false);
      await page.locator('select[name="account"]').selectOption('3');
      assert.equal(await page.locator('select[name="engine"]').inputValue(), 'Codex');
      await page.locator('input[name="modelSearch"]').fill('model 04');
      await page.locator('.model-results button').click();
      await page.locator('select[name="effort"]').selectOption('Sample high');
      await page.locator('.agent-form button[type="submit"]').click();
      await page.locator('.entry').click();
      await page.locator('[data-page="agents"]').click();
      assert.equal(await page.locator('select[name="account"]').inputValue(), '3');
      assert.match(await page.locator('#chosen-model').innerText(), /Codex model 04/);
      await page.locator('select[name="account"]').selectOption('1');
      assert.equal(await page.locator('#chosen-model').innerText(), 'Engine default');
      assert.equal(await page.locator('select[name="effort"]').inputValue(), 'Engine default');
      await page.locator('#agent-cancel').click();
      await page.locator('.entry').click();
      await page.locator('[data-page="agents"]').click();
      assert.equal(await page.locator('select[name="account"]').inputValue(), '3');
      if (evidence) await page.screenshot({path:path.join(evidence, `agent-${width}-${theme?'light':'dark'}.png`)});
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('#draft').inputValue(), 'Keep this draft');
      assert.equal(await page.locator('.entry').evaluate(el => el === document.activeElement), true);
      assert.deepEqual(errors, []);
      await page.close();
      console.log(`Accounts and agent assignment passed at ${width}, ${theme?'light':'dark'}`);
    }
  } finally {await browser.close();}
})().catch(error => {console.error(error);process.exitCode=1;});
