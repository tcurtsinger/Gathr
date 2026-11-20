const one = (selector) => document.querySelector(selector);
const all = (selector) => [...document.querySelectorAll(selector)];
const notify = (value) => {
	one("#status").textContent = value;
};
if (location.hash === "#light") document.documentElement.classList.add("light");
one("#theme").onclick = () => {
	const light = document.documentElement.classList.toggle("light");
	history.replaceState(null, "", light ? "#light" : location.pathname);
};
one("#preview-save").onclick = () => notify("Saved locally");
one("#preview-cancel").onclick = () => notify("Cancelled");
const help = one("#help");
const helpButton = one('[popovertarget="help"]');
help.addEventListener("toggle", (event) => {
	const shown = event.newState === "open";
	helpButton.setAttribute("aria-expanded", String(shown));
	if (!shown) return;
	const at = helpButton.getBoundingClientRect();
	const below = at.bottom + 6 + help.offsetHeight + 20 <= innerHeight;
	help.style.left = `${Math.max(20, Math.min(at.left, innerWidth - help.offsetWidth - 20))}px`;
	help.style.top = `${Math.max(20, below ? at.bottom + 6 : at.top - help.offsetHeight - 6)}px`;
});
const dialog = one("#settings");
function showCategory(name) {
	all("[data-section]").forEach((button) => {
		if (button.dataset.section === name) button.setAttribute("aria-current", "page");
		else button.removeAttribute("aria-current");
	});
	all("[data-settings]").forEach((panel) => {
		panel.hidden = panel.dataset.settings !== name;
	});
	one("#settings-heading").textContent = name;
}
function openSettings(category) {
	showCategory(category);
	dialog.showModal();
}
one("#settings-open").onclick = () => openSettings("General");
one("#settings-close").onclick = () => dialog.close();
one("#diagnostics-open").onclick = () => openSettings("Diagnostics");
all("[data-section]").forEach((button) => {
	button.onclick = () => showCategory(button.dataset.section);
});
one("#startup").onclick = (event) => {
	const button = event.currentTarget;
	button.setAttribute("aria-checked", String(button.getAttribute("aria-checked") !== "true"));
};
one("#add-account").onclick = () => notify("Account action previewed");
all("#agent-form, #project-form").forEach((form) => {
	form.onsubmit = (event) => {
		event.preventDefault();
		notify("Saved locally");
		dialog.close();
	};
});
one("#retry").onclick = () => notify("Retry previewed");
