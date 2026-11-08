const fs = require("fs"),
	path = require("path"),
	vm = require("vm"),
	assert = require("assert/strict");
const styles = ["shell", "conversation", "panels", "responsive"]
	.map((name) => fs.readFileSync(path.join(__dirname, name + ".css"), "utf8"))
	.join("\n");
const pages = [
	["chat", 14],
	["single-chat", 12],
	["new-chat", 12],
	["start-chat", 13],
];
for (const [d, v] of pages) {
	const file = path.join(__dirname, d, "v" + v + ".html"),
		h = fs.readFileSync(file, "utf8");
	const ids = [...h.matchAll(/data-id="([^"]+)"/g)].map((x) => x[1]);
	assert.equal(new Set(ids).size, ids.length, "unique data IDs");
	assert.equal((h.match(/<h1/g) || []).length, 1);
	assert(
		!/fetch\(|Sample workspace|>↑<|>＋<|<span class="path">Gathr/.test(h),
		"local content and polished icons",
	);
	assert(h.includes("../tokens.css") && h.includes("../components.css"), "shared styles");
	if (d === "chat" || d === "single-chat") assert.equal((h.match(/data-id="message-/g) || []).length, 42);
	for (const m of h.matchAll(/<script>([\s\S]*?)<\/script>/g)) new vm.Script(m[1]);
	for (const m of h.matchAll(/(?:href|location.href)=['"]([^'"#]+\.html)/g))
		assert(fs.existsSync(path.resolve(path.dirname(file), m[1])), m[1]);
}
assert(styles.includes(".choose .agentcheck:checked"), "picker styling");
console.log("Four historical shell pages: IDs, scripts, links and fixtures passed.");
assert(/\.chatrow\[hidden\]\s*\{\s*display:\s*none/.test(styles), "search hides flex rows");
new vm.Script(fs.readFileSync(path.join(__dirname, "navigation.js"), "utf8"));
new vm.Script(fs.readFileSync(path.join(__dirname, "phase1-prototype.js"), "utf8"));
new vm.Script(fs.readFileSync(path.join(__dirname, "settings-overlay.js"), "utf8"));
for (const folder of ["diagnostics", "engines", "chat-states"]) {
	for (const version of folder === "engines" ? [1, 2, 3, 4] : [1, 2, 3]) {
		const html = fs.readFileSync(path.join(__dirname, folder, `v${version}.html`), "utf8");
		const ids = [...html.matchAll(/data-id="([^"]+)"/g)].map((match) => match[1]);
		assert.equal(new Set(ids).size, ids.length, folder + ": unique IDs");
		assert.equal((html.match(/<h1/g) || []).length, 1);
		assert(html.includes("../tokens.css") && html.includes("../components.css"), "shared styles");
	}
}
console.log("Seven Phase 1 options: shared styles, IDs and script syntax passed.");
assert(/background:\s*var\(--backdrop\)/.test(styles), "shared backdrop");
for (const [d, v] of pages) {
	const h = fs.readFileSync(path.join(__dirname, d, "v" + v + ".html"), "utf8");
	assert(
		h.includes('src="../navigation.js"') && h.includes('data-id="sidebar-reveal"'),
		"shared navigation reveal",
	);
}

new vm.Script(fs.readFileSync(path.join(__dirname, "phase1-selected.js"), "utf8"));

for (const [folder, version] of pages) {
	const html = fs.readFileSync(path.join(__dirname, folder, `v${version}.html`), "utf8");
	assert(!html.includes('aria-label="Model and effort"'), "composer selects an agent only");
}
new vm.Script(fs.readFileSync(path.join(__dirname, "agent-editor.js"), "utf8"));

const system = fs.readFileSync(path.join(__dirname, "system.html"), "utf8");
const tokens = fs.readFileSync(path.join(__dirname, "tokens.css"), "utf8");
const design = fs.readFileSync(path.join(__dirname, "../DESIGN.md"), "utf8");
const systemIds = [...system.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1]);
const designIds = [...system.matchAll(/data-id="([^"]+)"/g)].map((match) => match[1]);
assert.equal(new Set(systemIds).size, systemIds.length, "system: unique HTML IDs");
assert.equal(new Set(designIds).size, designIds.length, "system: unique design IDs");
assert.equal((system.match(/<h1\b/g) || []).length, 1, "system: one main heading");
for (const match of system.matchAll(/(?:href|src)="([^"#]+)"/g)) {
	assert(!/^https?:/.test(match[1]), "system: offline assets only");
	assert(fs.existsSync(path.resolve(__dirname, match[1])), "system asset: " + match[1]);
}
for (const match of system.matchAll(/aria-(?:controls|labelledby|describedby)="([^"]+)"/g)) {
	for (const id of match[1].split(" ")) assert(systemIds.includes(id), "reference exists: " + id);
}
new vm.Script(fs.readFileSync(path.join(__dirname, "system.js"), "utf8"));
assert(tokens.includes("--muted: var(--muted-foreground)"), "legacy muted is readable text");
assert(tokens.includes("--accent: var(--attention)"), "legacy accent remains text");
assert(
	tokens.includes("--background: #181818") && tokens.includes("--card: #202020"),
	"prototype dark surfaces",
);
assert(design.includes("Team-message layout: pending"), "no unselected agent arrangement");
assert(design.includes("There is no Everyone option"), "one named recipient, no fan-out");
assert(
	design.includes("1024 by 760px") && design.includes("200px category rail"),
	"accepted Settings geometry",
);
assert(fs.existsSync(path.join(__dirname, "fonts/LICENSE.txt")), "font licence retained");
console.log("Current system: offline assets, IDs, references, script, tokens and design boundaries passed.");
