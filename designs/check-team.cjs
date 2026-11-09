const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const { pathToFileURL } = require("node:url");
const { chromium, webkit } = require(require.resolve("playwright", { paths: [path.join(__dirname, "../ui")] }));

async function check() {
	const output = process.argv[2] && path.resolve(process.argv[2]);
	if (output) {
		const relative = path.relative(path.resolve(__dirname, ".."), output);
		assert(relative.startsWith("..") || path.isAbsolute(relative), "Evidence must be outside the repository");
		fs.mkdirSync(output, { recursive: true });
	}
	const browser = await (process.platform === "win32" ? chromium : webkit).launch({
		...(process.platform === "win32" ? { channel: "msedge" } : {}),
		headless: true,
	});
	const results = [];
	const errors = [];
	try {
		const page = await browser.newPage();
		page.on("pageerror", (error) => errors.push(error.message));
		async function shot(name) {
			if (output) await page.screenshot({ path: path.join(output, `${name}.png`) });
		}
		for (const width of [1440, 1024]) {
			for (const theme of ["dark", "light"]) {
				const label = `${width}-${theme}`;
				await page.setViewportSize({ width, height: 900 });
				const url = pathToFileURL(path.join(__dirname, "system.html"));
				url.searchParams.set("theme", theme);
				url.hash = theme === "light" ? "light" : "team-message";
				await page.goto(url.href);
				await page.evaluate(() => document.fonts.ready);
				await page.locator("#team-message").evaluate((node) => node.scrollIntoView({ block: "start" }));
				assert.equal(await page.locator("html").evaluate((node) => node.classList.contains("light")), theme === "light");
				const thread = page.locator("#round-thread");
				const toggle = page.locator("#thread-toggle");
				const draft = page.getByRole("textbox", { name: "Message", exact: true });
				const recipient = page.getByRole("combobox", { name: "Recipient" });
				const isOpen = () => thread.evaluate((node) => node.open);
				const reset = () => page.getByRole("button", { name: "Reset", exact: true }).click();
				assert(await isOpen(), `${label}: a running thread starts open`);
				const geometry = await thread.evaluate((node) => {
					const replies = node.querySelector(".ds-thread-replies");
					return { height: replies.clientHeight, scroll: replies.scrollHeight, overflow: getComputedStyle(replies).overflowY };
				});
				assert(geometry.height > 900, `${label}: long work must grow beyond a viewport`);
				assert.equal(geometry.height, geometry.scroll, `${label}: replies must have no inner scroll area`);
				assert.equal(geometry.overflow, "visible", `${label}: replies grow in the page`);
				assert.equal(await recipient.inputValue(), "Mira");
				assert.equal(await recipient.locator("option").count(), 5);
				assert.equal(await recipient.locator("option", { hasText: "Everyone" }).count(), 0);
				await shot(`running-${label}`);
				await page.getByText("32 tool calls", { exact: true }).click();
				assert.equal((await page.locator("#rowan-tool-log").innerText()).trim().split("\n").length, 32);
				await shot(`tools-${label}`);
				await page.getByText("32 tool calls", { exact: true }).click();
				await draft.fill("Keep this draft while I read the team’s work.");
				await recipient.selectOption("Lane");
				await page.getByRole("button", { name: "Finish", exact: true }).click();
				assert.equal(await isOpen(), false, `${label}: untouched thread folds on finish`);
				assert(await page.locator("#round-final").isVisible());
				assert.equal(await recipient.inputValue(), "Lane");
				assert.equal(await draft.inputValue(), "Keep this draft while I read the team’s work.");
				await page.locator("#team-message").evaluate((node) => node.scrollIntoView({ block: "start" }));
				await shot(`finished-${label}`);
				await page.locator(".ds-composer").scrollIntoViewIfNeeded();
				await shot(`finished-bottom-${label}`);
				await toggle.focus();
				await page.keyboard.press("Enter");
				assert(await isOpen(), `${label}: keyboard opens the thread`);
				await page.keyboard.press("Enter");
				assert.equal(await isOpen(), false, `${label}: keyboard closes the thread`);
				await reset();
				await toggle.click();
				const mainReply = page.locator('[data-id="direct-reply"]');
				await mainReply.scrollIntoViewIfNeeded();
				const readingTop = await mainReply.evaluate((node) => node.getBoundingClientRect().top);
				await page.getByRole("button", { name: "Late reply", exact: true }).dispatchEvent("click");
				assert.equal(await mainReply.evaluate((node) => node.getBoundingClientRect().top), readingTop,
					`${label}: late update must preserve the reading position`);
				assert.equal(await isOpen(), false, `${label}: late reply respects manual close`);
				assert((await page.locator("#thread-state").innerText()).includes("11:32"));
				assert(await page.locator("#late-member").evaluate((node) => node.closest("details").id === "round-thread"));
				assert(await page.locator("#round-final").evaluate((node) =>
					Boolean(document.querySelector('[data-id="direct-reply"]').compareDocumentPosition(node) & Node.DOCUMENT_POSITION_FOLLOWING)));
				assert.equal(await page.locator('[data-id="status-without-thread"] details').count(), 0);
				assert.equal(await page.locator('[data-id="direct-reply"] details').count(), 0);
				await page.locator("#round-final").scrollIntoViewIfNeeded();
				await shot(`late-bottom-${label}`);
				await page.locator("#thread-back").focus();
				await page.keyboard.press("Enter");
				assert(await isOpen(), `${label}: backlink opens its original thread`);
				assert(await page.locator("#late-member").isVisible(), `${label}: late reply is readable in the original thread`);
				assert(await toggle.evaluate((node) => node === document.activeElement), `${label}: backlink focuses disclosure`);
				assert.equal(await draft.inputValue(), "Keep this draft while I read the team’s work.");
				await shot(`late-thread-${label}`);
				await reset();
				await toggle.click();
				await toggle.click();
				await page.getByRole("button", { name: "Finish", exact: true }).click();
				assert(await isOpen(), `${label}: manual open wins over automatic fold`);
				await reset();
				const trigger = page.locator("#team-summary-open");
				await page.locator("#team-message").evaluate((node) => node.scrollIntoView({ block: "start" }));
				await trigger.focus();
				await page.keyboard.press("Enter");
				const summary = page.locator("#summary");
				await page.waitForFunction(() => document.querySelector("#summary").style.left !== "");
				assert(await summary.evaluate((node) => node.matches(":popover-open")));
				assert.equal(await summary.locator(".ds-agent").count(), 5);
				const placement = await summary.evaluate((node) => {
					const at = node.getBoundingClientRect();
					const header = document.querySelector("#team-header").getBoundingClientRect();
					return { left: at.left, right: at.right, top: at.top, bottom: at.bottom, headerBottom: header.bottom };
				});
				assert(placement.left >= 16 && placement.right <= width - 16 && placement.bottom <= 884, `${label}: Summary fits`);
				assert.equal(placement.top, placement.headerBottom + 8, `${label}: Summary anchors below header`);
				await summary.locator("summary").first().click();
				assert(await page.getByText("Reconcile results, then ask Sol to review", { exact: true }).isVisible());
				await shot(`summary-${label}`);
				await page.keyboard.press("Escape");
				await page.waitForFunction(() => !document.querySelector("#summary").matches(":popover-open"));
				assert(await trigger.evaluate((node) => node === document.activeElement), `${label}: Escape returns Summary focus`);
				assert.equal(await recipient.inputValue(), "Lane");
				for (const target of [trigger, toggle, page.locator("#thread-back")]) {
					if (!(await target.isVisible())) continue;
					await target.hover();
					const hover = await target.evaluate((node) => getComputedStyle(node).backgroundColor);
					await page.mouse.move(0, 0);
					await target.focus();
					const focus = await target.evaluate((node) => ({
						background: getComputedStyle(node).backgroundColor,
						outline: getComputedStyle(node).outlineStyle,
						shadow: getComputedStyle(node).boxShadow,
					}));
					assert.equal(focus.background, hover, `${label}: focus equals hover`);
					assert.equal(focus.outline, "none", `${label}: no focus outline`);
					assert.equal(focus.shadow, "none", `${label}: no focus ring shadow`);
				}
				assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${label}: no page overflow`);
				await draft.fill("Keep <b>this</b> as text.");
				await page.getByRole("button", { name: "Send", exact: true }).last().click();
				assert.equal(await page.locator("#team-sent-messages").innerText(), "Keep <b>this</b> as text.");
				assert.equal(await page.locator("#team-sent-messages b").count(), 0);
				assert.equal(await draft.inputValue(), "");
				await page.getByRole("button", { name: "New", exact: true }).click();
				assert.equal(await page.locator("#team-sent-messages").innerText(), "");
				results.push({ width, theme, geometry, placement, interactions: "passed" });
				console.log(`${label}: thread states, manual choice, tools, draft, Summary and keyboard passed`);
			}
		}
		assert.deepEqual(errors, [], "No browser errors");
		if (output) fs.writeFileSync(path.join(output, "checks.json"), JSON.stringify({ results, errors }, null, 2));
	} finally {
		await browser.close();
	}
}
check().catch((error) => { console.error(error); process.exitCode = 1; });
