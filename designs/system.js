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
one("#agent-form").onsubmit = (event) => {
	event.preventDefault();
	notify("Saved locally");
	dialog.close();
};
const tabs = all("[role=tab]");
function selectTab(tab) {
	tabs.forEach((button) => {
		const selected = button === tab;
		button.setAttribute("aria-selected", String(selected));
		button.tabIndex = selected ? 0 : -1;
		one("#" + button.getAttribute("aria-controls")).hidden = !selected;
	});
}
tabs.forEach((tab, index) => {
	tab.onclick = () => selectTab(tab);
	tab.onkeydown = (event) => {
		if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
		event.preventDefault();
		const next =
			event.key === "Home"
				? 0
				: event.key === "End"
					? tabs.length - 1
					: (index + (event.key === "ArrowRight" ? 1 : tabs.length - 1)) % tabs.length;
		selectTab(tabs[next]);
		tabs[next].focus();
	};
});
one("#answer-send").onclick = () => {
	if (one("input[name=answer]:checked")) notify("Answer sent locally");
};
one("#message").oninput = () => {
	one("#message-send").disabled = !one("#message").value.trim();
};
one(".ds-composer").onsubmit = (event) => {
	event.preventDefault();
	const input = one("#message");
	if (!input.value.trim()) return;
	const bubble = document.createElement("div");
	bubble.className = "ds-user-message";
	bubble.textContent = input.value;
	one("#sent-messages").append(bubble);
	input.value = "";
	one("#message-send").disabled = true;
	notify("Sent locally");
	input.focus();
};
one("#new-message").onclick = () => {
	one("#sent-messages").replaceChildren();
	one("#message").value = "";
	one("#message-send").disabled = true;
	one("#message").focus();
};
one("#retry").onclick = () => notify("Retry previewed");
