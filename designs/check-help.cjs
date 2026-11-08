const assert = require("node:assert/strict");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { chromium, webkit } = require(require.resolve("playwright", { paths: [path.join(__dirname, "../ui")] }));

async function check() {
	const browser = await (process.platform === "win32" ? chromium : webkit).launch({
		...(process.platform === "win32" ? { channel: "msedge" } : {}),
		headless: true,
	});
	try {
		const page = await browser.newPage();
		for (const [width, height] of [[1440, 900], [1024, 900], [1440, 600], [1024, 600]]) {
			for (const theme of ["dark", "light"]) {
				await page.setViewportSize({ width, height });
				const url = pathToFileURL(path.join(__dirname, "system.html"));
				url.searchParams.set("theme", theme);
				url.hash = theme === "light" ? "light" : "";
				await page.goto(url.href);
				assert.equal(await page.locator("html").evaluate((root) => root.classList.contains("light")), theme === "light");
				const trigger = page.getByRole("button", { name: "About approvals" });
				await trigger.focus();
				await page.keyboard.press("Enter");
				await page.waitForFunction(() => document.querySelector("#help").matches(":popover-open"));
				// Popover placement runs in its toggle event, after the open state changes.
				await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
				const at = await trigger.boundingBox();
				const panel = await page.locator("#help").boundingBox();
				const label = `${width}x${height} ${theme}`;
				assert(panel, `${label}: Help must be visible`);
				assert(
					Math.abs(panel.y - at.y - at.height - 6) < 1 || Math.abs(panel.y + panel.height + 6 - at.y) < 1,
					`${label}: Help must sit 6px above or below its trigger; trigger=${JSON.stringify(at)}, Help=${JSON.stringify(panel)}`,
				);
				assert(panel.x >= 20 && panel.x + panel.width <= width - 20, `${label}: Help must stay inside the viewport`);
				assert.equal(await trigger.getAttribute("aria-expanded"), "true", `${label}: the trigger must report open`);
				await page.keyboard.press("Escape");
				assert.equal(await page.locator("#help").isVisible(), false, `${label}: Escape must close Help`);
				await page.waitForFunction(() => document.querySelector('[popovertarget="help"]').getAttribute("aria-expanded") === "false");
				assert(await trigger.evaluate((button) => button === document.activeElement), `${label}: focus must return to the ?`);
				console.log(`${label}: Help placement, Escape and focus passed`);
			}
		}
	} finally {
		await browser.close();
	}
}

check().catch((error) => {
	console.error(error);
	process.exitCode = 1;
});
