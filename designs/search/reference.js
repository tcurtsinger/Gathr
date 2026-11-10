(() => {
	const one = (s) => document.querySelector(s),
		fly = one("#navigation-flyout"),
		input = one("#search-query");
	if (!fly) return;
	const chats = [
		{ title: "Reconnect streaming", archived: false },
		{
			title:
				"Reconnect streaming notes and keyboard shortcut review from the archived desktop rollout",
			archived: true,
		},
	];
	const messages = [
		{
			title: chats[0].title,
			who: "Mira",
			time: "09:52",
			text: "The partial reply stays visible after reconnect. Retry keeps your draft too.",
		},
		{
			title: chats[1].title,
			who: "You",
			time: "Yesterday",
			text: "Keep reconnect and keyboard shortcuts consistent. Preserve the partial answer.",
			archived: true,
		},
		{
			title: "Review keyboard shortcuts",
			who: "Lane",
			time: "Monday",
			text: "Reconnect replay passed with a saved draft and a stopped partial answer.",
		},
	];
	let active = -1,
		rows = [],
		trigger = one("#search-open"),
		mode = "search";
	const mark = (parent, text) => {
		const words = input.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
		let at = 0;
		const pattern = /[\p{L}\p{N}_]+/gu;
		for (const match of text.matchAll(pattern)) {
			if (!words.some((w) => match[0].toLowerCase().startsWith(w))) continue;
			parent.append(document.createTextNode(text.slice(at, match.index)));
			const m = document.createElement("mark");
			m.textContent = match[0];
			parent.append(m);
			at = match.index + match[0].length;
		}
		parent.append(document.createTextNode(text.slice(at)));
	};
	function select(index) {
		active = index;
		rows.forEach((row, i) =>
			row.setAttribute("aria-selected", String(i === index)),
		);
		if (rows[index]) {
			input.setAttribute("aria-activedescendant", rows[index].id);
			rows[index].scrollIntoView({ block: "nearest" });
		} else input.removeAttribute("aria-activedescendant");
	}
	function openHit(hit) {
		one("#search-chat-title").textContent = hit.title;
		one("#search-chat-archived").hidden = !hit.archived;
		one("#search-target-text").textContent =
			hit.text || "Chat title match. Open at the latest message.";
		one("#search-target strong").textContent = hit.who || "Mira";
		one("#search-open-status").textContent = hit.archived
			? "Archived chat. Read only."
			: hit.text
				? "Search result"
				: "Chat opened";
		one("#search-latest").hidden = !hit.text;
		fly.hidePopover();
		const target = one("#search-target");
		target.removeAttribute("data-found");
		void target.offsetWidth;
		if (hit.text) target.setAttribute("data-found", "");
		target.scrollIntoView({ block: "center" });
		one("#search-destination").focus();
	}
	function render() {
		const out = one("#search-results"),
			feedback = one("#search-feedback");
		out.replaceChildren();
		rows = [];
		select(-1);
		feedback.textContent = "";
		const state = one("#search-state").value,
			q = input.value.trim();
		if (state === "loading") {
			feedback.textContent = "Searching…";
			return;
		}
		if (state === "error") {
			feedback.textContent = "Search failed. Ref: sample-search-01";
			const retry = document.createElement("button");
			retry.className = "ds-button";
			retry.textContent = "Retry";
			retry.onclick = () => {
				one("#search-state").value = "results";
				render();
				input.focus();
			};
			feedback.append(" ", retry);
			return;
		}
		if (!q) {
			feedback.textContent = "Search chats and messages";
			return;
		}
		const terms = q.toLowerCase().split(/\s+/);
		const results = [
			[
				"Chats",
				chats.filter((h) => h.title.toLowerCase().includes(q.toLowerCase())),
			],
			[
				"Messages",
				messages.filter((h) =>
					terms.every((t) =>
						(h.text.toLowerCase().match(/[\p{L}\p{N}_]+/gu) || []).some((w) =>
							w.startsWith(t),
						),
					),
				),
			],
		];
		if (state === "none") results.forEach((g) => (g[1] = []));
		for (const [name, hits] of results) {
			if (!hits.length) continue;
			const group = document.createElement("div");
			group.setAttribute("role", "group");
			group.setAttribute("aria-label", name);
			const heading = document.createElement("p");
			heading.className = "ds-search-group";
			heading.textContent = name;
			group.append(heading);
			for (const hit of hits) {
				const row = document.createElement("button");
				row.className = "ds-search-hit";
				row.id = "search-hit-" + rows.length;
				row.dataset.id = row.id;
				row.setAttribute("role", "option");
				row.setAttribute("aria-selected", "false");
				row.tabIndex = -1;
				const title = document.createElement("span");
				title.className = "ds-search-hit-title";
				mark(title, hit.title);
				row.append(title);
				if (hit.text) {
					const snippet = document.createElement("span");
					snippet.className = "ds-search-snippet";
					mark(snippet, hit.text);
					row.append(snippet);
				}
				const meta = document.createElement("small");
				meta.textContent = hit.text ? hit.who + " · " + hit.time : "Chat";
				if (hit.archived) {
					const label = document.createElement("span");
					label.className = "ds-search-archived";
					label.textContent = "Archived";
					meta.append(label);
				}
				row.append(meta);
				row.onclick = () => openHit(hit);
				rows.push(row);
				group.append(row);
			}
			out.append(group);
		}
		if (!rows.length) feedback.textContent = "No matches";
	}
	function position() {
		const box = one(".ds-search-nav").getBoundingClientRect();
		const width = Math.min(340, innerWidth - 32);
		fly.style.left = Math.min(box.right + 8, innerWidth - width - 16) + "px";
		fly.style.top =
			Math.max(16, Math.min(box.top + 40, innerHeight - 300)) + "px";
		fly.style.maxHeight = innerHeight - parseFloat(fly.style.top) - 16 + "px";
	}
	function open(which, button) {
		trigger = button;
		mode = which;
		const search = which === "search";
		one("#flyout-close").title = search ? "Close Search" : "Close Tasks";
		one("#flyout-title").textContent = search ? "Search" : "Tasks";
		one("#flyout-close").setAttribute(
			"aria-label",
			search ? "Close Search" : "Close Tasks",
		);
		one("#search-content").hidden = !search;
		one("#tasks-flyout-content").hidden = search;
		fly.dataset.layout = one("#search-layout").value;
		position();
		if (!fly.matches(":popover-open")) fly.showPopover();
		render();
		(search ? input : one("#flyout-close")).focus();
	}
	for (const [id, which] of [
		["#search-open", "search"],
		["#tasks-flyout-open", "tasks"],
	])
		one(id).onclick = (e) => {
			e.preventDefault();
			const button = one(id);
			if (fly.matches(":popover-open") && mode === which) fly.hidePopover();
			else open(which, button);
		};
	fly.addEventListener("toggle", () => {
		for (const id of ["#search-open", "#tasks-flyout-open"])
			one(id).setAttribute(
				"aria-expanded",
				String(fly.matches(":popover-open") && one(id) === trigger),
			);
		if (!fly.matches(":popover-open")) trigger.focus();
	});
	one("#flyout-close").onclick = () => fly.hidePopover();
	input.oninput = render;
	input.onkeydown = (e) => {
		if (!["ArrowDown", "ArrowUp", "Enter"].includes(e.key)) return;
		e.preventDefault();
		if (e.key === "Enter") {
			if (rows[active]) rows[active].click();
			return;
		}
		if (rows.length)
			select(
				(active + (e.key === "ArrowDown" ? 1 : rows.length - 1) + rows.length) %
					rows.length,
			);
	};
	one("#search-state").onchange = () => {
		const value = one("#search-state").value;
		input.value =
			value === "empty" ? "" : value === "none" ? "nomatch" : "reconnect";
		open("search", one("#search-open"));
	};
	one("#search-layout").onchange = () => {
		fly.dataset.layout = one("#search-layout").value;
	};
	one("#search-new").onclick = () => {
		one("#search-chat-title").textContent = "New chat";
		one("#search-target-text").textContent = "What are we working on?";
	};
	one("#search-latest").onclick = () => {
		one("#search-target").removeAttribute("data-found");
		one("#search-latest").hidden = true;
		one("#search-open-status").textContent = "Latest message";
	};
	one("#search-task-example")?.addEventListener("click", () => {
		fly.hidePopover();
		one("#search-chat-title").textContent = "Reconnect streaming";
		one("#search-target-text").textContent = "Keep the partial reply?";
		one("#search-open-status").textContent = "Tasks preview";
	});
	addEventListener("resize", () => {
		if (fly.matches(":popover-open")) position();
	});
	addEventListener("keydown", (e) => {
		if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
			e.preventDefault();
			open("search", one("#search-open"));
		}
	});
	const version = document.body.dataset.searchVersion;
	if (version) one("#search-layout").value = version;
	input.value = "reconnect";
	render();
})();
