(() => {
	const get = (selector) => document.querySelector(selector);
	const menu = get("#archive-menu");
	const opener = get("#archive-menu-open");
	const confirm = get("#archive-confirm");
	let action;
	menu.addEventListener("toggle", (event) => {
		if (event.newState !== "open") return;
		menu.querySelector("button").focus();
		const at = opener.getBoundingClientRect();
		menu.style.left =
			Math.max(
				8,
				Math.min(
					at.right - menu.offsetWidth,
					innerWidth - menu.offsetWidth - 8,
				),
			) + "px";
		menu.style.top =
			Math.max(
				8,
				Math.min(at.bottom + 8, innerHeight - menu.offsetHeight - 8),
			) + "px";
	});
	menu.addEventListener("keydown", (event) => {
		if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
		event.preventDefault();
		const items = [...menu.querySelectorAll("button")];
		const current = items.indexOf(document.activeElement);
		const next =
			event.key === "Home"
				? 0
				: event.key === "End"
					? items.length - 1
					: (current + (event.key === "ArrowDown" ? 1 : items.length - 1)) %
						items.length;
		items[next].focus();
	});
	menu.querySelectorAll("[data-confirm]").forEach((button) => {
		button.onclick = () => {
			action = button.dataset.confirm;
			menu.hidePopover();
			get("#archive-confirm-title").textContent = action + " Launch checklist?";
			get("#archive-confirm-go").textContent = action;
			get("#archive-confirm-go").dataset.variant =
				action === "Delete" ? "danger" : "primary";
			confirm.showModal();
			get("#archive-confirm-cancel").focus();
		};
	});
	const close = () => {
		confirm.close();
		opener.focus();
	};
	get("#archive-confirm-cancel").onclick = close;
	confirm.addEventListener("cancel", (event) => {
		event.preventDefault();
		close();
	});
	get("#archive-confirm-go").onclick = () => {
		get("#archive-sample-status").textContent = action + " confirmed locally";
		close();
	};
	get("#archive-pause").onclick = () => {
		menu.hidePopover();
		opener.focus();
	};
	get("#archive-restore").onclick = () => {
		get("#archive-sample-status").textContent = "Restore previewed locally";
	};
	get("#archive-general-open").onclick = () => openSettings("General");
	for (const [id, button] of [
		["stale-days-help", "stale-days-help-open"],
		["cancel-days-help", "cancel-days-help-open"],
	]) {
		const panel = get("#" + id);
		panel.addEventListener("toggle", (event) => {
			const shown = event.newState === "open";
			get("#" + button).setAttribute("aria-expanded", String(shown));
			if (!shown) return;
			const at = get("#" + button).getBoundingClientRect();
			panel.style.left =
				Math.max(20, Math.min(at.left, innerWidth - panel.offsetWidth - 20)) +
				"px";
			panel.style.top =
				Math.max(
					20,
					Math.min(at.bottom + 6, innerHeight - panel.offsetHeight - 20),
				) + "px";
		});
	}
	["#system-stale-days", "#system-cancel-days"].forEach((id) => {
		const input = get(id);
		const save = () => {
			const valid = ["#system-stale-days", "#system-cancel-days"].every(
				(name) => {
					const value = get(name).value;
					return (
						/^\d+$/.test(value) && Number(value) >= 1 && Number(value) <= 365
					);
				},
			);
			get("#system-days-invalid").hidden = valid;
			notify(valid ? "Saved locally" : "");
		};
		input.onchange = save;
		input.onkeydown = (event) => {
			if (event.key === "Enter") {
				event.preventDefault();
				save();
			}
		};
	});
})();
