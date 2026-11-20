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
})();
