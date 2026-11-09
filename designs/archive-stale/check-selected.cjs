const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");
const { pathToFileURL } = require("node:url");
const { chromium, webkit } = require(
	require.resolve("playwright", { paths: [path.join(__dirname, "../../ui")] }),
);
const root = path.resolve(__dirname, "../..");
assert(process.argv[2], "Pass an evidence folder outside the repository");
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
		async function open(file, width, theme) {
			await page.setViewportSize({ width, height: 900 });
			const url = pathToFileURL(path.join(root, "designs", file));
			url.searchParams.set("theme", theme);
			if (theme === "light") url.hash = "light";
			await page.goto(url.href);
			await page.evaluate(() => document.fonts.ready);
		}
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
		for (const width of [1440, 1024])
			for (const theme of ["dark", "light"]) {
				const label = width + "-" + theme;
				await open("system.html", width, theme);
				await page.locator("#archive-stale").scrollIntoViewIfNeeded();
				assert.equal(
					await page.locator("#archive-fold").evaluate((n) => n.open),
					false,
				);
				await page.locator("#archive-fold > summary").click();
				const truncation = await page
					.locator('[data-id="archived-chat-3"]')
					.evaluate((n) => ({
						overflow: n.scrollWidth > n.clientWidth,
						style: getComputedStyle(n).textOverflow,
					}));
				assert(truncation.overflow && truncation.style === "ellipsis");
				await shot("system-" + label + "-archive-stale");
				await page.locator("#archive-menu-open").click();
				await page.waitForFunction(
					() => document.activeElement.id === "archive-pause",
				);
				await fits("#archive-menu");
				await page.keyboard.press("End");
				assert.equal(
					await page
						.locator("#archive-menu [role=menuitem]")
						.last()
						.evaluate((n) => n === document.activeElement),
					true,
				);
				await page.keyboard.press("Enter");
				assert.equal(
					await page.locator("#archive-confirm-title").textContent(),
					"Delete Launch checklist?",
				);
				assert.equal(
					await page
						.locator("#archive-confirm-go")
						.getAttribute("data-variant"),
					"danger",
				);
				assert.equal(
					await page.evaluate(() => document.activeElement.id),
					"archive-confirm-cancel",
				);
				await fits("#archive-confirm");
				assert(
					(await page.locator("#archive-confirm").boundingBox()).height < 300,
					"Compact confirm fits its content",
				);
				await shot("system-" + label + "-delete-confirm");
				await page.keyboard.press("Escape");
				assert.equal(
					await page.evaluate(() => document.activeElement.id),
					"archive-menu-open",
				);
				await page.locator("#archive-menu-open").click();
				await page.locator('[data-confirm="Archive"]').click();
				assert.equal(
					await page
						.locator("#archive-confirm-go")
						.getAttribute("data-variant"),
					"primary",
				);
				await page.locator("#archive-confirm-go").click();
				assert.equal(
					await page.locator("#archive-sample-status").textContent(),
					"Archive confirmed locally",
				);
				await page.locator("#archive-general-open").click();
				await fits("#settings");
				await shot("system-" + label + "-general");
				assert.equal(
					await page.locator("#system-cancel-days").getAttribute("max"),
					"365",
				);
				await page.locator("#system-stale-days").fill("366");
				await page.locator("#system-cancel-days").focus();
				assert(await page.locator("#system-days-invalid").isVisible());
				await page.locator("#system-stale-days").fill("365");
				await page.locator("#system-cancel-days").focus();
				assert(await page.locator("#system-days-invalid").isHidden());
				await page.locator("#cancel-days-help-open").click();
				await page.waitForFunction(() =>
					document.querySelector("#cancel-days-help").matches(":popover-open"),
				);
				await fits("#cancel-days-help");
				await shot("system-" + label + "-cancel-help");
				await page.keyboard.press("Escape");
				assert(await page.locator("#settings").evaluate((n) => n.open));
				await page.keyboard.press("Escape");
				const ids = await page
					.locator("[data-id]")
					.evaluateAll((nodes) => nodes.map((n) => n.dataset.id));
				assert.equal(new Set(ids).size, ids.length);
				assert.equal(
					await page.evaluate(
						() => document.documentElement.scrollWidth > innerWidth,
					),
					false,
				);
				results.push({ page: "system", width, theme });
				await open("archive-stale/v3.html", width, theme);
				assert.equal(
					await page.locator("#archive-toggle").getAttribute("aria-expanded"),
					"false",
				);
				assert(await page.locator("#archive-open").isHidden());
				await page.locator("#archive-toggle").click();
				await shot("selected-" + label + "-archive");
				await page.locator("#tasks-open").click();
				assert(
					(await page.locator("#stale-tasks").textContent()).includes(
						"Idle 12 days",
					),
				);
				assert(
					!(await page.locator("#stale-tasks").textContent()).includes(
						"Stale 4 days",
					),
				);
				await fits("#tasks-flyout");
				await shot("selected-" + label + "-stale");
				await page.locator('[data-task="33"]').click();
				assert.equal(
					await page.locator("#task-stale").textContent(),
					"Idle 4 days",
				);
				await page.locator("#task-answer").fill("Keep the platform defaults.");
				await shot("selected-" + label + "-stale-question");
				await page.locator('[data-close="workspace"]').click();
				await page.locator("#settings-open").click();
				assert.equal(
					await page.locator("#stale-days").getAttribute("max"),
					"365",
				);
				await page.locator("#stale-days").fill("366");
				await page.locator("#cancel-days").focus();
				assert(await page.locator("#validation").isVisible());
				assert.equal(
					await page.locator("#validation").textContent(),
					"Enter whole days from 1 to 365.",
				);
				await page.locator("#stale-days").fill("365");
				await page.locator("#cancel-days").focus();
				assert(await page.locator("#validation").isHidden());
				await shot("selected-" + label + "-general");
				await page.keyboard.press("Escape");
				await page.locator("#sample-size").selectOption("large");
				await page.locator("#archive-toggle").click();
				assert.equal(
					await page.locator("#archive-content .chatrow").count(),
					63,
				);
				await page
					.locator("#archive-content .chatrow")
					.last()
					.scrollIntoViewIfNeeded();
				assert.equal(
					await page.evaluate(
						() => document.documentElement.scrollWidth > innerWidth,
					),
					false,
				);
				await shot("selected-" + label + "-large-archive");
				results.push({ page: "selected-v3", width, theme });
			}
		assert.deepEqual(errors, []);
		fs.writeFileSync(
			path.join(output, "selected-checks.json"),
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
