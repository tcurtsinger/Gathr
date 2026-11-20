const fs = require("fs"),
	path = require("path"),
	vm = require("vm"),
	assert = require("assert/strict");
const styles = ["shell", "conversation", "panels", "responsive"]
	.map((name) => fs.readFileSync(path.join(__dirname, name + ".css"), "utf8"))
	.join("\n");
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

new vm.Script(fs.readFileSync(path.join(__dirname, "phase1-selected.js"), "utf8"));

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
	tokens.includes("--frame: #232323") && tokens.includes("--background: #1b1b1b") && tokens.includes("--card: #2c2c2c"),
	"dark surfaces: the chat darkest, the frame next, then cards (0068)",
);
assert(design.includes("There is no Everyone option"), "one named recipient, no fan-out");
assert(
	design.includes("1024 by 760px") && design.includes("200px category rail"),
	"accepted Settings geometry",
);
assert(fs.existsSync(path.join(__dirname, "fonts/LICENSE.txt")), "font licence retained");
console.log("Current system: offline assets, IDs, references, script, tokens and design boundaries passed.");

assert(tokens.includes("font-weight: 300 700"), "Font exposes requested variable weights");
assert(tokens.includes("hanken-grotesk-latin-wght-normal.woff2"), "Hanken Grotesk self-hosted");
assert(!fs.existsSync(path.join(__dirname, "fonts/public-sans-latin-wght-normal.woff2")), "Public Sans removed");

new vm.Script(fs.readFileSync(path.join(__dirname, "search/reference.js"), "utf8"));
for (const version of [1, 2]) {
 const html = fs.readFileSync(path.join(__dirname, "search/v" + version + ".html"), "utf8");
 const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map(match => match[1]);
 assert.equal(new Set(ids).size, ids.length, "Search proposal IDs unique");
 assert(html.includes("reference.js") && html.includes("../tokens.css"), "Search uses shared assets");
}
assert(system.includes('id="search-preview"'), "System contains Search");
