const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const { pathToFileURL } = require("node:url");
const { chromium, webkit } = require(require.resolve("playwright", { paths: [path.join(__dirname, "../ui")] }));

async function check() {
	const browser = await (process.platform === "win32" ? chromium : webkit).launch({
		...(process.platform === "win32" ? { channel: "msedge" } : {}),
		headless: true,
	});
	try {
		const page = await browser.newPage();
		for (const width of [1440, 1024]) {
			for (const theme of ["dark", "light"]) {
				await page.setViewportSize({ width, height: 900 });
				const url = pathToFileURL(path.join(__dirname, "system.html"));
				url.searchParams.set("theme", theme);
				url.hash = theme === "light" ? "light" : "";
				await page.goto(url.href);
				assert.equal(await page.locator("html").evaluate((root) => root.classList.contains("light")), theme === "light");
				for (const name of ["Home", "Tasks", "Files"]) {
					const tab = page.getByRole("tab", { name, exact: true });
					await tab.click();
					const panel = page.getByRole("tabpanel", { name, exact: true });
					await page.mouse.move(0, 0);
					const resting = await panel.evaluate((node) => getComputedStyle(node).backgroundColor);
					await panel.hover();
					const hovered = await panel.evaluate((node) => getComputedStyle(node).backgroundColor);
					await page.mouse.move(0, 0);
					await tab.focus();
					await page.keyboard.press("Tab");
					const focused = await panel.evaluate((node) => ({
						active: node === document.activeElement,
						outline: getComputedStyle(node).outlineStyle,
						shadow: getComputedStyle(node).boxShadow,
						background: getComputedStyle(node).backgroundColor,
					}));
					const label = `${width} ${theme} ${name}`;
					if (process.argv[2]) {
						fs.mkdirSync(process.argv[2], { recursive: true });
						await page.screenshot({ path: path.join(process.argv[2], `panel-${width}-${theme}-${name}.png`) });
					}
					assert(focused.active, `${label}: Tab must focus the panel`);
					assert.equal(focused.outline, "none", `${label}: panel must have no focus outline`);
					assert.equal(focused.shadow, "none", `${label}: panel must have no focus shadow`);
					assert.notEqual(hovered, resting, `${label}: hover must visibly highlight the panel`);
					assert.equal(focused.background, hovered, `${label}: keyboard focus must use the hover highlight`);
					console.log(`${label}: keyboard focus and hover highlight passed`);
				}
				await page.locator("#search-open").click();
				const input = page.locator("#search-query");
				await input.fill("partial");
				const row = page.locator("#search-results .ds-search-hit").first();
				await row.hover();
				const hovered = await row.evaluate(node => getComputedStyle(node).backgroundColor);
				await page.mouse.move(0, 0);
				await input.press("ArrowDown");
				assert.equal(await row.evaluate(node => getComputedStyle(node).backgroundColor), hovered);
				assert.equal(await row.getAttribute("aria-selected"), "true");
				await input.press("Escape");

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

