const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { chromium, webkit } = require(
	require.resolve("playwright", { paths: [path.join(__dirname, "../../ui")] }),
);
const root = path.resolve(__dirname, "../..");
assert(
	process.argv[2],
	"Pass an absolute evidence folder outside the repository",
);
const output = path.resolve(process.argv[2]);
assert(
	path.relative(root, output).startsWith(".."),
	"Evidence must stay outside the repository",
);
fs.mkdirSync(output, { recursive: true });
async function run() {
	const browser = await (process.platform === "win32"
		? chromium
		: webkit
	).launch({
		...(process.platform === "win32" ? { channel: "msedge" } : {}),
		headless: true,
	});
	const errors = [];
	const results = [];
	try {
		const page = await browser.newPage();
		page.on("pageerror", (e) => errors.push(e.message));
		const shot = (name) =>
			page.screenshot({ path: path.join(output, name + ".png") });
		async function fits(selector) {
			const r = await page.locator(selector).boundingBox();
			assert(
				r &&
					r.x >= 0 &&
					r.y >= 0 &&
					r.x + r.width <= page.viewportSize().width + 1 &&
					r.y + r.height <= page.viewportSize().height + 1,
				selector + " fits the viewport",
			);
		}
		for (const version of ["now", "v1", "v2"]) {
			for (const width of [1440, 1024])
				for (const theme of ["dark", "light"]) {
					await page.setViewportSize({ width, height: 900 });
					const url = pathToFileURL(
						path.join(root, "designs/archive-stale", version + ".html"),
					);
					url.searchParams.set("theme", theme);
					await page.goto(url.href);
					await page.evaluate(() => document.fonts.ready);
					const label = version + "-" + width + "-" + theme;
					await shot(label + "-shell");
					if (version === "now") {
						await page.locator("#tasks-open").click();
						await shot(label + "-tasks");
						await page.keyboard.press("Escape");
						await page.locator("#settings-open").click();
						await shot(label + "-general");
						await page.keyboard.press("Escape");
						results.push({ version, width, theme, current: true });
						continue;
					}
					await page
						.locator("#composer-text")
						.fill("Keep this draft while I inspect the archive.");
					await page.locator('[data-menu="chat-1"]').first().click();
					await fits("#chat-menu");
					await shot(label + "-row-menu");
					await page
						.getByRole("menuitem", { name: "Archive", exact: true })
						.click();
					assert.equal(
						await page.evaluate(() => document.activeElement.id),
						"confirm-cancel",
					);
					await shot(label + "-archive-confirm");
					await page.locator("#confirm-cancel").click();
					assert.equal(
						await page.locator("#composer-text").inputValue(),
						"Keep this draft while I inspect the archive.",
					);
					await page.locator("#header-more").click();
					await page
						.getByRole("menuitem", { name: "Delete", exact: true })
						.click();
					await shot(label + "-delete-confirm");
					await page.evaluate(
						() => (document.querySelector("#simulate-failure").checked = true),
					);
					await page.locator("#confirm-go").click();
					assert.equal(
						await page.locator("#confirm-error-text").textContent(),
						"Delete failed",
					);
					await shot(label + "-failure");
					await page.keyboard.press("Escape");
					await page.locator("#simulate-failure").uncheck();
					await page.locator("#tasks-open").click();
					await fits("#tasks-flyout");
					assert.equal(await page.locator("#stale-tasks .task-row").count(), 3);
					await shot(label + "-stale-group");
					await page.locator('[data-task="34"]').click();
					await fits("#workspace");
					assert.equal(
						await page.locator("#agent-owner").textContent(),
						"Mira",
					);
					await shot(label + "-stale-agent");
					await page.locator('[data-close="workspace"]').click();
					await page.locator("#tasks-open").click();
					await page.locator('[data-task="33"]').click();
					await page.locator("#task-answer").fill("Use the platform defaults.");
					await shot(label + "-stale-question");
					await page.locator('[data-close="workspace"]').click();
					await page.locator("#settings-open").click();
					await fits("#settings");
					await shot(label + "-general");
					await page.locator("#help-cancel-button").click();
					assert(
						(await page.locator("#help").textContent()).includes(
							"after they become stale",
						),
					);
					await fits("#help");
					await shot(label + "-days-help");
					await page.keyboard.press("Escape");
					assert(await page.locator("#settings").evaluate((n) => n.open));
					await page.locator("#stale-days").fill("0");
					await page.locator("#cancel-days").focus();
					assert(await page.locator("#validation").isVisible());
					await page.locator("#stale-days").fill("4");
					await page.locator("#cancel-days").focus();
					assert.equal(await page.locator("#saved").textContent(), "Saved");
					await page.keyboard.press("Escape");
					await page.locator("#reset").click();
					await page.locator("#header-more").click();
					await page
						.getByRole("menuitem", { name: "Archive", exact: true })
						.click();
					await page.locator("#confirm-go").click();
					assert(await page.locator("#archived-line").isVisible());
					assert(await page.locator("#composer-wrap").isHidden());
					await shot(label + "-archived-chat");
					await page.locator("#restore-chat").click();
					assert(await page.locator("#composer-wrap").isVisible());
					await page.locator("#sample-size").selectOption("large");
					if (version === "v1") {
						await page.locator("#archive-toggle").click();
						assert.equal(
							await page.locator("#archive-content .chatrow").count(),
							63,
						);
						await page
							.locator("#archive-content .chatrow")
							.last()
							.scrollIntoViewIfNeeded();
						await shot(label + "-large-archive");
					} else {
						await page.locator("#archive-open").click();
						await fits("#archive-flyout");
						assert.equal(
							await page.locator("#archive-results .chatrow").count(),
							63,
						);
						await page
							.locator("#archive-results .chatrow")
							.last()
							.scrollIntoViewIfNeeded();
						await shot(label + "-large-archive");
					}
					await page.keyboard.press("Escape");
					await page.locator("#tasks-open").click();
					await page
						.locator("#stale-tasks .task-row")
						.last()
						.scrollIntoViewIfNeeded();
					await shot(label + "-large-stale");
					await page.keyboard.press("Escape");
					await page.locator("#sample-size").selectOption("empty");
					await page.locator("#tasks-open").click();
					await shot(label + "-empty");
					assert.equal(
						await page.evaluate(
							() => document.documentElement.scrollWidth > innerWidth,
						),
						false,
					);
					const ids = await page
						.locator("[data-id]")
						.evaluateAll((nodes) => nodes.map((n) => n.dataset.id));
					assert.equal(new Set(ids).size, ids.length, "Unique design IDs");
					results.push({
						version,
						width,
						theme,
						flows:
							"archive/delete confirm, failure, stale routing, question, settings, help, validation, restore, large lists, empty",
					});
				}
		}
		for (const version of ["v1", "v2"]) {
			await page.setViewportSize({ width: 390, height: 844 });
			await page.goto(
				pathToFileURL(
					path.join(root, "designs/archive-stale", version + ".html"),
				).href,
			);
			await page.locator("#mobile-nav").click();
			await page.locator("#tasks-open").click();
			await fits("#tasks-flyout");
			await shot(version + "-390-dark-tasks");
			await page.keyboard.press("Escape");
			await page.locator("#mobile-nav").click();
			await page.locator("#settings-open").click();
			await fits("#settings");
			await shot(version + "-390-dark-general");
			assert.equal(
				await page.evaluate(
					() => document.documentElement.scrollWidth > innerWidth,
				),
				false,
			);
		}
		assert.deepEqual(errors, []);
		fs.writeFileSync(
			path.join(output, "checks.json"),
			JSON.stringify({ results, errors }, null, 2),
		);
		console.log(JSON.stringify({ matrices: results.length, errors, output }));
	} finally {
		await browser.close();
	}
}
run().catch((e) => {
	console.error(e);
	process.exitCode = 1;
});
