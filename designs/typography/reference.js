(() => {
	const split = document.querySelector(".ds-split-body");
	const layout = document.createElement("div");
	layout.className = "type-chat-and-workspace";
	split.before(layout);
	layout.append(split);
	const workspace = document.createElement("aside");
	workspace.className = "type-workspace";
	workspace.id = "type-workspace";
	workspace.dataset.id = "type-workspace";
	workspace.setAttribute("aria-label", "Workspace");
	workspace.append(document.querySelector(".ds-workspace"));
	workspace.hidden = true;
	layout.append(workspace);
	const button = document.createElement("button");
	button.className = "ds-button";
	button.id = "type-workspace-toggle";
	button.dataset.id = button.id;
	button.textContent = "Workspace";
	button.setAttribute("aria-expanded", "false");
	button.setAttribute("aria-controls", workspace.id);
	document.querySelector(".ds-team-header .ds-row").append(button);
	const close = document.createElement("button");
	close.className = "ds-button";
	close.dataset.icon = "";
	close.id = "type-workspace-close";
	close.dataset.id = close.id;
	close.title = "Close workspace";
	close.setAttribute("aria-label", "Close workspace");
	close.innerHTML =
		'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12"/></svg>';
	workspace.querySelector(".ds-tabstrip").append(close);
	function setOpen(open) {
		workspace.hidden = !open;
		button.setAttribute("aria-expanded", String(open));
	}
	button.onclick = () => setOpen(workspace.hidden);
	close.onclick = () => {
		setOpen(false);
		button.focus();
	};
	workspace.addEventListener("keydown", (event) => {
		if (event.key === "Escape") {
			setOpen(false);
			button.focus();
		}
	});
	const scroll = document.querySelector("#supervisor-scroll");
	document.fonts.ready.then(() =>
		requestAnimationFrame(() => {
			scroll.scrollTop = scroll.scrollHeight;
		}),
	);
	const nav = document.createElement("button");
	nav.className = "ds-button type-nav-toggle";
	nav.id = "type-nav-toggle";
	nav.dataset.id = nav.id;
	nav.textContent = "Navigation";
	nav.setAttribute("aria-expanded", "false");
	document.querySelector(".ds-header nav").prepend(nav);
	function setNav(open) {
		document.body.classList.toggle("type-nav-open", open);
		nav.setAttribute("aria-expanded", String(open));
	}
	nav.onclick = () =>
		setNav(!document.body.classList.contains("type-nav-open"));
	document.querySelector(".ds-navigation").addEventListener("keydown", (e) => {
		if (e.key === "Escape") {
			setNav(false);
			nav.focus();
		}
	});
})();
