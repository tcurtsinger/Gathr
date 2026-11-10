const assert = require("node:assert/strict"),
	fs = require("node:fs"),
	path = require("node:path");
const { pathToFileURL } = require("node:url");
const { chromium, webkit } = require(
	require.resolve("playwright", { paths: [path.join(__dirname, "../ui")] }),
);
async function check() {
	const output = process.argv[2];
	if (output) {
		assert(path.isAbsolute(output));
		assert(
			!path
				.resolve(output)
				.startsWith(path.resolve(__dirname, "..") + path.sep),
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
		const shot = async (name) => {
			await page
				.locator("#supervisor-scroll")
				.evaluate((n) => (n.scrollTop = n.scrollHeight));
			if (output)
				await page.screenshot({ path: path.join(output, name + ".png") });
		};
		for (const width of [1440, 1024, 390])
			for (const theme of ["dark", "light"]) {
				const label = width + "-" + theme;
				await page.setViewportSize({ width, height: 900 });
				const url = pathToFileURL(path.join(__dirname, "typography/v1.html"));
				url.searchParams.set("theme", theme);
				if (theme === "light") url.hash = "light";
				await page.goto(url.href);
				await page.evaluate(() => document.fonts.ready);
				const font = await page.evaluate(() => {
					const s = getComputedStyle(document.body);
					return {
						family: s.fontFamily,
						size: s.fontSize,
						weight: s.fontWeight,
						line: s.lineHeight,
						faces: [...document.fonts].map((f) => ({
							family: f.family,
							status: f.status,
							weight: f.weight,
						})),
					};
				});
				assert(
					font.family.includes("Hanken Grotesk Variable"),
					label + " family",
				);
				assert.equal(font.size, "15px");
				assert.equal(font.weight, "450");
				assert.equal(font.line, "24px");
				assert(
					font.faces.some(
						(f) =>
							f.family.includes("Hanken Grotesk") &&
							f.status === "loaded" &&
							f.weight === "300 700",
					),
					label + " font loaded",
				);
				assert.equal(
					await page
						.locator("#message")
						.evaluate((n) => getComputedStyle(n).fontSize),
					"16px",
				);
				const baseline = await page
					.locator("#round-handoff .ds-speaker")
					.evaluate((header) => {
						const label = header.querySelector(".ds-speaker-label"),
							name = header.querySelector(".ds-speaker-name"),
							role = header.querySelector(".ds-role"),
							avatar = header.querySelector(".ds-avatar");
						const markers = [name, role].map((n) => {
							const m = document.createElement("span");
							m.style.cssText =
								"display:inline-block;width:0;height:0;vertical-align:baseline";
							n.append(m);
							return m;
						});
						const y = markers.map((n) => n.getBoundingClientRect().top);
						markers.forEach((n) => n.remove());
						const a = avatar.getBoundingClientRect(),
							b = label.getBoundingClientRect();
						return {
							delta: Math.abs(y[0] - y[1]),
							align: getComputedStyle(label).alignItems,
							gap: getComputedStyle(label).gap,
							avatarDelta: Math.abs((a.top + a.bottom - b.top - b.bottom) / 2),
							name: getComputedStyle(name).fontSize,
							role: getComputedStyle(role).fontSize,
						};
					});
				assert.equal(baseline.align, "baseline");
				assert.equal(baseline.gap, "8px");
				if (width >= 1024) {
					assert(baseline.delta < 1, label + " shared baseline");
					assert(baseline.avatarDelta < 1, label + " avatar centred");
				}
				assert.equal(baseline.name, "14px");
				assert.equal(baseline.role, "12px");
				const overflow = async (selectors) =>
					page.evaluate(
						(selectors) =>
							selectors.flatMap((s) =>
								[...document.querySelectorAll(s)]
									.filter(
										(n) =>
											n.getClientRects().length &&
											n.scrollWidth > n.clientWidth + 1,
									)
									.map((n) => ({
										selector: s,
										width: n.clientWidth,
										scroll: n.scrollWidth,
									})),
							),
						selectors,
					);
				assert.deepEqual(
					await overflow([
						"body",
						".ds-team-header",
						".ds-split-body",
						".ds-supervisor",
						".ds-compose-dock",
						"#agent-plan",
						".ds-plan-steps",
						"#plan-toggle",
					]),
					[],
					label + " split overflow",
				);
				await shot(label + "-split");
				const pill = await page.locator("#waiting-on-team").boundingBox();
				const viewport = await page.locator("#supervisor-scroll").boundingBox();
				assert(
					pill.y >= viewport.y &&
						pill.y + pill.height <= viewport.y + viewport.height + 1,
					label + " wait pill visible above dock",
				);
				await page.locator("#agent-plan").evaluate((n) => (n.open = true));
				assert.deepEqual(
					await overflow(["#agent-plan", ".ds-plan-steps"]),
					[],
					label + " expanded plan overflow",
				);
				await page.locator("#type-workspace-toggle").click();
				assert(await page.locator("#type-workspace").isVisible());
				assert.deepEqual(
					await overflow(["#type-workspace", ".ds-tabstrip", "#pane-home"]),
					[],
					label + " workspace overflow",
				);
				await shot(label + "-workspace");
				await page.locator("#tab-tasks").click();
				await page.locator("#type-workspace-close").click();
				await page.locator("#team-summary-open").click();
				await page
					.locator(".ds-agent[data-agent=kit]")
					.evaluate((n) => (n.open = true));
				assert.deepEqual(
					await overflow([
						"#summary",
						".ds-agent",
						".ds-agent summary",
						".ds-todos",
					]),
					[],
					label + " Summary overflow",
				);
				await shot(label + "-summary");
				await page.keyboard.press("Escape");
				await page.locator("#split-toggle").click();
				assert(await page.locator("#team-column").isHidden());
				const thread = await page
					.locator("#supervisor-scroll .ds-thread-column")
					.evaluate((n) => n.getBoundingClientRect().width);
				assert(thread <= 832.1, label + " 52rem maximum");
				await shot(label + "-single");
				results.push({ width, theme, font, baseline, passed: true });
				console.log(label + " typography passed");
			}
		assert.deepEqual(errors, [], "no script errors");
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
