const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const { pathToFileURL } = require("node:url");
const { chromium, webkit } = require(
	require.resolve("playwright", { paths: [path.join(__dirname, "../ui")] }),
);

async function check() {
	const output = process.argv[2] && path.resolve(process.argv[2]);
	if (output) {
		const relative = path.relative(path.resolve(__dirname, ".."), output);
		assert(
			relative.startsWith("..") || path.isAbsolute(relative),
			"Evidence must be outside the repository",
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
	const results = [];
	const errors = [];
	try {
		const page = await browser.newPage();
		page.on("pageerror", (error) => errors.push(error.message));
		const shot = async (name) => {
			if (output)
				await page.screenshot({ path: path.join(output, `${name}.png`) });
		};
		for (const width of [1440, 1024]) {
			for (const theme of ["dark", "light"]) {
				const label = `${width}-${theme}`;
				await page.setViewportSize({ width, height: 900 });
				const url = pathToFileURL(path.join(__dirname, "system.html"));
				url.searchParams.set("theme", theme);
				if (theme === "light") url.hash = "light";
				await page.goto(url.href);
				await page.evaluate(() => document.fonts.ready);
				await page
					.locator("#team-message")
					.evaluate((node) => node.scrollIntoView({ block: "start" }));
				assert.equal(
					await page
						.locator("html")
						.evaluate((node) => node.classList.contains("light")),
					theme === "light",
				);
				const toggle = page.locator("#split-toggle");
				const team = page.locator("#team-column");
				const main = page.locator("#supervisor-scroll");
				const draft = page.getByRole("textbox", {
					name: "Message",
					exact: true,
				});
				const recipient = page.getByRole("combobox", { name: "Recipient" });
				const scrollTop = (node) => node.evaluate((el) => el.scrollTop);
				const geometry = await team.evaluate((node) => ({
					width: node.getBoundingClientRect().width,
					height: node.clientHeight,
					scroll: node.scrollHeight,
					overflow: getComputedStyle(node).overflowY,
				}));
				const left = await page.locator(".ds-supervisor").boundingBox();
				assert(
					Math.abs(left.width - geometry.width) <= 1,
					`${label}: split is 50/50`,
				);
				assert(
					geometry.scroll > geometry.height,
					`${label}: long team reply scrolls independently`,
				);
				assert.equal(geometry.overflow, "auto");
				assert.equal(await recipient.inputValue(), "Mira");
				assert.equal(await recipient.locator("option").count(), 5);
				assert.equal(
					await recipient.locator("option", { hasText: "Everyone" }).count(),
					0,
				);
				assert.equal(
					await page.locator("#round-thread, #thread-back").count(),
					0,
					"inline thread removed",
				);
				assert.equal(
					await page.locator("#team-column [data-id=direct-reply]").count(),
					0,
					"direct exchange stays in main",
				);
				assert.equal(await page.locator("#plan-count").innerText(), "1/4");
				const indent = await page
					.locator("[data-id=thread-rowan] > p:not(.ds-assignment)")
					.first()
					.evaluate(
						(node) =>
							node.getBoundingClientRect().left -
							node.parentElement.getBoundingClientRect().left,
					);
				assert.equal(indent, 34, "body aligns with speaker name");
				const clearance = await team.evaluate((node) => ({
					labelBottom: node
						.querySelector(".ds-column-label")
						.getBoundingClientRect().bottom,
					assignmentTop: node
						.querySelector(".ds-assignment")
						.getBoundingClientRect().top,
				}));
				assert(
					clearance.assignmentTop >= clearance.labelBottom,
					`${label}: first assignment clears sticky label; got ${JSON.stringify(clearance)}`,
				);
				await shot(`running-top-${label}`);
				await main.evaluate((node) => {
					node.scrollTop = node.scrollHeight;
				});
				const pill = await page.locator("#waiting-on-team").evaluate((node) => {
					const rect = node.getBoundingClientRect();
					const viewport = node
						.closest(".ds-column-scroll")
						.getBoundingClientRect();
					const plan = document
						.querySelector("#agent-plan")
						.getBoundingClientRect();
					return {
						top: rect.top,
						bottom: rect.bottom,
						viewportTop: viewport.top,
						viewportBottom: viewport.bottom,
						planTop: plan.top,
					};
				});
				assert(
					pill.top >= pill.viewportTop && pill.bottom <= pill.viewportBottom,
					`${label}: waiting pill is fully visible at latest message; got ${JSON.stringify(pill)}`,
				);
				assert(
					pill.bottom <= pill.planTop,
					`${label}: waiting pill is above plan bar`,
				);
				await shot(`running-${label}`);
				await page.getByText("32 tool calls", { exact: true }).click();
				assert.equal(
					(await page.locator("#rowan-tool-log").innerText()).trim().split("\n")
						.length,
					32,
				);
				await page.getByText("32 tool calls", { exact: true }).click();
				await draft.fill("Keep this draft while I read the team’s work.");
				await recipient.selectOption("Lane");
				await team.evaluate((node) => {
					node.scrollTop = 250;
				});
				const reading = await scrollTop(main);
				assert.equal(await scrollTop(team), 250);
				assert.equal(
					await scrollTop(main),
					reading,
					"team scroll leaves main reading position",
				);
				await toggle.focus();
				await page.keyboard.press("Enter");
				assert(
					await team.evaluate((node) => node.hidden),
					"keyboard hides split",
				);
				await page.keyboard.press("Enter");
				assert.equal(
					await team.evaluate((node) => node.hidden),
					false,
					"keyboard shows split",
				);
				assert.equal(await scrollTop(team), 250, "split preserves team scroll");
				await toggle.click();
				await page.locator("#round-late").click();
				assert(
					await page.locator("#team-unread").isVisible(),
					"late work marks a closed split",
				);
				assert.equal(
					await team.evaluate((node) => node.hidden),
					true,
					"late work does not open split",
				);
				await toggle.click();
				assert.equal(
					await page.locator("#team-unread").isVisible(),
					false,
					"opening clears header dot",
				);
				assert.equal(
					await page.locator("#late-unread").isVisible(),
					false,
					"opening marks fixture reply seen",
				);
				assert.equal(
					await scrollTop(team),
					250,
					"late work preserves team reading position",
				);
				assert(
					await page
						.locator("#late-member")
						.evaluate((node) => node.closest("#team-column") !== null),
				);
				assert.equal(await page.locator("#plan-count").innerText(), "4/4");
				assert.equal(await page.locator("#waiting-on-team").isVisible(), false);
				assert.equal(await page.locator("#team-working").isVisible(), false);
				assert.equal(await recipient.inputValue(), "Lane");
				assert.equal(
					await draft.inputValue(),
					"Keep this draft while I read the team’s work.",
				);
				await shot(`late-${label}`);
				await page.locator("#round-reset").click();
				await toggle.click();
				await page.locator("#waiting-on-team").click();
				assert.equal(
					await team.evaluate((node) => node.hidden),
					false,
					"waiting indicator opens split",
				);
				await page.locator("#plan-toggle").click();
				assert.equal(
					await page.locator("#agent-plan").evaluate((node) => node.open),
					true,
				);
				assert.equal(await page.locator(".ds-plan-steps li").count(), 4);
				assert.equal(
					await page.locator("#step-kit-detail").innerText(),
					"Checked Tab, Enter and Escape",
				);
				await shot(`plan-${label}`);
				for (const state of ["blocked", "failed", "stopped"]) {
					await page.locator(`#round-${state}`).click();
					assert.equal(
						await page.locator("#plan-count").innerText(),
						"1/4",
						"unsuccessful outcomes do not count done",
					);
					assert.equal(
						await page.locator("#step-kit-outcome").innerText(),
						state[0].toUpperCase() + state.slice(1),
					);
					assert.equal(
						await page.locator(`[data-owner="Kit"] .ds-step-icon`).isVisible(),
						false,
						"outcome has no working spinner",
					);
					assert.equal(
						await page
							.locator(`#waiting-on-team .ds-workers`)
							.getAttribute("aria-label"),
						"Lane",
					);
					await shot(`${state}-${label}`);
				}
				await page.locator("#round-reset").click();
				await page.locator("#plan-toggle").click();
				const trigger = page.locator("#team-summary-open");
				await trigger.focus();
				await page.keyboard.press("Enter");
				const summary = page.locator("#summary");
				await page.waitForFunction(
					() => document.querySelector("#summary").style.left !== "",
				);
				assert(await summary.evaluate((node) => node.matches(":popover-open")));
				assert.equal(await summary.locator(".ds-agent").count(), 5);
				assert.equal(
					await page.locator("#summary-rowan-count").innerText(),
					"1/1",
				);
				assert.equal(
					await page.locator("#summary-kit-count").innerText(),
					"0/1",
				);
				await shot(`summary-collapsed-${label}`);
				for (const agent of await summary.locator(".ds-agent summary").all())
					await agent.click();
				await shot(`summary-${label}`);
				await page.setViewportSize({ width, height: 600 });
				await page.locator("#summary-sol-task").scrollIntoViewIfNeeded();
				const last = await page.locator("#summary-sol-task").boundingBox();
				assert(
					last.y >= 16 && last.y + last.height <= 584,
					"last assignment reachable in short window",
				);
				await shot(`summary-short-${label}`);
				await page.keyboard.press("Escape");
				await page.waitForFunction(
					() => !document.querySelector("#summary").matches(":popover-open"),
				);
				assert(
					await trigger.evaluate((node) => node === document.activeElement),
					"Escape returns Summary focus",
				);
				await page.setViewportSize({ width, height: 900 });
				await page.locator("#round-finish").click();
				assert.equal(
					await page.locator(".ds-segments").getAttribute("aria-valuenow"),
					"4",
				);
				await shot(`finished-${label}`);
				await page.locator("#archive-fold summary").click();
				const lid = await page
					.locator("#archive-fold .ds-archive-lid")
					.evaluate((node) => getComputedStyle(node).transform);
				// Motion is checked under reduced motion below, avoiding any transition timing dependency.
				assert(
					await page.locator("#archive-fold").evaluate((node) => node.open),
				);
				for (const target of [toggle, page.locator("#plan-toggle")]) {
					await target.hover();
					const hover = await target.evaluate(
						(node) => getComputedStyle(node).backgroundColor,
					);
					await page.mouse.move(0, 0);
					await target.focus();
					await page.keyboard.press("Tab");
					await page.keyboard.press("Shift+Tab");
					const focus = await target.evaluate((node) => ({
						background: getComputedStyle(node).backgroundColor,
						outline: getComputedStyle(node).outlineStyle,
						shadow: getComputedStyle(node).boxShadow,
					}));
					assert.equal(focus.background, hover, "focus equals hover");
					assert.equal(focus.outline, "none");
					assert.equal(focus.shadow, "none");
				}
				await page.emulateMedia({ reducedMotion: "reduce" });
				const motion = await page
					.locator("#archive-fold .ds-archive-lid")
					.evaluate((node) => ({
						transition: getComputedStyle(node).transitionDuration,
						transform: getComputedStyle(node).transform,
					}));
				assert.equal(motion.transition, "0s");
				assert.notEqual(motion.transform, "none", "expanded archive lifts lid");
				await page.locator("#round-reset").click();
				assert.equal(
					await page
						.locator("#team-working .ds-spinner")
						.evaluate((node) => getComputedStyle(node).animationName),
					"none",
				);
				await page.emulateMedia({ reducedMotion: "no-preference" });
				assert(
					await page.evaluate(
						() => document.documentElement.scrollWidth <= innerWidth,
					),
					"no page overflow",
				);
				await draft.fill("Keep <b>this</b> as text.");
				await page
					.getByRole("button", { name: "Send", exact: true })
					.last()
					.click();
				assert.equal(
					await page.locator("#team-sent-messages").innerText(),
					"Keep <b>this</b> as text.",
				);
				assert.equal(await page.locator("#team-sent-messages b").count(), 0);
				assert.equal(await draft.inputValue(), "");
				await page.getByRole("button", { name: "New", exact: true }).click();
				assert.equal(await page.locator("#team-sent-messages").innerText(), "");
				results.push({
					width,
					theme,
					geometry,
					clearance,
					pill,
					lid,
					interactions: "passed",
				});
				console.log(
					`${label}: split, scroll, late/unread, plan, Summary, draft and keyboard passed`,
				);
			}
		}
		assert.deepEqual(errors, [], "No browser errors");
		if (output)
			fs.writeFileSync(
				path.join(output, "checks.json"),
				JSON.stringify({ results, errors }, null, 2),
			);
	} finally {
		await browser.close();
	}
}
check().catch((error) => {
	console.error(error);
	process.exitCode = 1;
});
