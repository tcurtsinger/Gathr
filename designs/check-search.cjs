const assert = require("node:assert/strict"),
	fs = require("node:fs"),
	path = require("node:path");
const { pathToFileURL } = require("node:url");
const { chromium, webkit } = require(
	require.resolve("playwright", {
		paths: [
			path.join(__dirname, "../ui"),
			...(process.env.NODE_PATH || "").split(path.delimiter),
		],
	}),
);
async function check() {
	const output = process.argv[2];
	if (output) {
		assert(path.isAbsolute(output), "absolute evidence folder");
		assert(
			!path
				.resolve(output)
				.startsWith(path.resolve(__dirname, "..") + path.sep),
			"evidence outside repo",
		);
		fs.mkdirSync(output, { recursive: true });
	}
	const browser = await (process.platform === "win32"
		? chromium
		: webkit
	).launch({
		...(process.platform === "win32" ? { channel: "msedge" } : {}),
		headless: true,
	});
	const results = [],
		errors = [];
	try {
		const page = await browser.newPage();
		page.on("pageerror", (e) => errors.push(e.message));
		for (const file of ["system.html", "search/v1.html", "search/v2.html"])
			for (const width of [1440, 1024, 390])
				for (const theme of ["dark", "light"]) {
					await page.setViewportSize({ width, height: 900 });
					const url = pathToFileURL(path.join(__dirname, file));
					if (theme === "light") url.hash = "light";
					url.searchParams.set("theme", theme);
					await page.goto(url.href);
					await page.evaluate(() => document.fonts.ready);
					const label =
						file.replaceAll("/", "-").replace(".html", "") +
						"-" +
						width +
						"-" +
						theme;
					await page.locator("#search-preview").scrollIntoViewIfNeeded();
					await page.locator("#search-open").click();
					const fly = page.locator("#navigation-flyout"),
						input = page.locator("#search-query");
					await page.waitForTimeout(50);
					assert(
						await fly.evaluate((n) => n.matches(":popover-open")),
						label + " opens",
					);
					assert(
						await input.evaluate((n) => n === document.activeElement),
						label + " input autofocus",
					);
					assert.equal(await input.inputValue(), "reconnect");
					assert(
						(await page
							.locator(".ds-search-archived")
							.filter({ hasText: "Archived" })
							.count()) > 0,
					);
					const bounds = await fly.boundingBox();
					assert(
						bounds.x >= 0 && bounds.x + bounds.width <= width + 1,
						label + " flyout fits",
					);
					assert(bounds.width <= 340, label + " max 340");
					assert.equal(
						await page.locator("#search-results [role=group]").count(),
						2,
					);
					await input.press("ArrowDown");
					assert.equal(
						await input.getAttribute("aria-activedescendant"),
						"search-hit-0",
					);
					await input.press("ArrowDown");
					assert.equal(
						await input.getAttribute("aria-activedescendant"),
						"search-hit-1",
					);
					await input.press("ArrowDown");
					assert.equal(
						await input.getAttribute("aria-activedescendant"),
						"search-hit-2",
					);
					if (output)
						await page.screenshot({
							path: path.join(output, label + "-results.png"),
						});
					await input.press("Enter");
					assert(
						!(await fly.evaluate((n) => n.matches(":popover-open"))),
						label + " result closes",
					);
					assert.equal(
						await page.locator("#search-open-status").textContent(),
						"Search result",
					);
					await page.locator("#search-open").click();
					assert.equal(await input.inputValue(), "reconnect");
					await input.press("Escape");
					await page.waitForTimeout(50);
					assert(
						await page
							.locator("#search-open")
							.evaluate((n) => n === document.activeElement),
						label + " Escape restores trigger",
					);
					await page.locator("#search-open").click();
					if (width < 600) await page.locator("#flyout-close").click();
					await page.locator("#tasks-flyout-open").click();
					assert(await page.locator("#search-content").isHidden());
					assert(await page.locator("#tasks-flyout-content").isVisible());
					await page
						.getByRole("button", { name: "Close Tasks", exact: true })
						.click();
					for (const [state, text] of [
						["empty", "Search chats and messages"],
						["none", "No matches"],
						["loading", "Searching…"],
						["error", "Search failed. Ref: sample-search-01"],
					]) {
						await page.locator("#search-state").selectOption(state);
						assert(
							(await page.locator("#search-feedback").textContent()).includes(
								text,
							),
							label + " " + state,
						);
						if (output && width === 1024)
							await page.screenshot({
								path: path.join(output, label + "-" + state + ".png"),
							});
						await page.locator("#flyout-close").click();
					}
					await page.locator("#search-state").selectOption("error");
					await page
						.locator("#search-feedback")
						.getByRole("button", { name: "Retry", exact: true })
						.click();
					assert.equal(
						await page.locator("#search-results [role=group]").count(),
						2,
					);
					await page.locator("#search-results .ds-search-hit").nth(3).click();
					assert.equal(
						await page.locator("#search-chat-archived").isVisible(),
						true,
						label + " archived destination labelled",
					);
					assert.equal(
						await page.locator("#search-open-status").textContent(),
						"Archived chat. Read only.",
					);
					await page.keyboard.press(
						process.platform === "darwin" ? "Meta+k" : "Control+k",
					);
					assert(
						await input.evaluate((n) => n === document.activeElement),
						label + " shortcut focuses input",
					);
					await input.fill("partial");
					const row = page.locator(".ds-search-hit").first();
					await page.mouse.move(0, 0);
					const rest = await row.evaluate(
						(n) => getComputedStyle(n).backgroundColor,
					);
					await row.hover();
					const hover = await row.evaluate(
						(n) => getComputedStyle(n).backgroundColor,
					);
					await page.mouse.move(0, 0);
					await input.press("ArrowDown");
					const selected = await row.evaluate((n) => ({
						bg: getComputedStyle(n).backgroundColor,
						outline: getComputedStyle(n).outlineStyle,
						shadow: getComputedStyle(n).boxShadow,
					}));
					assert.notEqual(rest, hover);
					assert.equal(selected.bg, hover);
					assert.equal(selected.outline, "none");
					assert.equal(selected.shadow, "none");
					await input.fill("zzzz");
					assert.equal(
						await page.locator("#search-feedback").textContent(),
						"No matches",
					);
					await page.locator("#flyout-close").click();
					results.push({ page: file, width, theme, passed: true });
					console.log(label + " passed");
				}
		assert.deepEqual(errors, [], "no page errors");
		if (output)
			fs.writeFileSync(
				path.join(output, "results.json"),
				JSON.stringify(results, null, 2),
			);
	} finally {
		await browser.close();
	}
}
check().catch((e) => {
	console.error(e);
	process.exitCode = 1;
});
