const $ = (q) => document.querySelector(q);
const variant = document.body.dataset.variant;
const now = variant === "now";
const icons = Object.fromEntries(
	[...document.querySelectorAll("svg[data-icon]")].map((n) => [
		n.dataset.icon,
		n.outerHTML,
	]),
);
const icon = (name) => icons[name] || "";
const escape = (t) =>
	String(t).replace(
		/[&<>"']/g,
		(c) =>
			({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
				c
			],
	);
let chats,
	tasks,
	active = "chat-1",
	menuChat,
	action,
	back;
let expanded = false;
let draft = "";
let drafts = {};
let selectedTask;
const initialChats = [
	{
		id: "chat-1",
		title: "Launch checklist",
		project: "Fieldnotes",
		archived: false,
	},
	{
		id: "chat-2",
		title: "Search performance",
		project: "Fieldnotes",
		archived: false,
	},
	{
		id: "chat-3",
		title: "Keyboard navigation",
		project: "Fieldnotes",
		archived: false,
	},
	{ id: "chat-4", title: "Release notes", project: "", archived: false },
	{ id: "chat-5", title: "Account setup", project: "", archived: false },
	{
		id: "chat-6",
		title: "Sidebar exploration",
		project: "Fieldnotes",
		archived: true,
	},
	{ id: "chat-7", title: "First-run copy", project: "", archived: true },
	{
		id: "chat-8",
		title:
			"Conversation layout and navigation review for the upcoming desktop release",
		project: "Fieldnotes",
		archived: true,
	},
];
const initialTasks = [
	{
		id: 31,
		title: "Pick the release headline",
		owner: "You",
		giver: "Rowan",
		chat: "chat-1",
		stale: false,
	},
	{
		id: 32,
		title: "Check the new search flow",
		owner: "You",
		giver: "Rowan",
		chat: "chat-2",
		stale: false,
	},
	{
		id: 33,
		title: "Choose the keyboard shortcuts",
		owner: "You",
		giver: "Rowan",
		chat: "chat-3",
		stale: true,
		since: "4 days",
		state: "Needs your answer",
	},
	{
		id: 34,
		title: "Review empty search results",
		owner: "Mira",
		giver: "Rowan",
		chat: "chat-2",
		stale: true,
		since: "5 days",
		state: "Blocked",
	},
	{
		id: 35,
		title: "Check the release build",
		owner: "Ellis",
		giver: "Rowan",
		chat: "chat-1",
		stale: true,
		since: "8 days",
		state: "Waiting on CI",
	},
];
function chat(id) {
	return chats.find((c) => c.id === id);
}
function row(c, archived, index) {
	return (
		'<div class="chatrow ' +
		(c.id === active ? "selected " : "") +
		(archived ? "archived-row" : c.project ? "project-chat" : "") +
		'" data-id="' +
		(archived ? "archived" : "active") +
		"-row-" +
		index +
		'">' +
		'<button class="chat-link" title="' +
		escape(c.title) +
		'" data-chat="' +
		c.id +
		'" ' +
		(c.id === active ? 'aria-current="page"' : "") +
		"><span>" +
		escape(c.title) +
		"</span></button>" +
		(now
			? ""
			: '<button class="row-more" data-menu="' +
				c.id +
				'" aria-label="More for ' +
				escape(c.title) +
				'" aria-haspopup="menu">' +
				icon("more") +
				"</button>") +
		"</div>"
	);
}
function render() {
	const live = chats.filter((c) => !c.archived);
	$("#project-chats").innerHTML = live
		.filter((c) => c.project)
		.map((c, i) => row(c, false, i + 1))
		.join("");
	$("#plain-chats").innerHTML = live
		.filter((c) => !c.project)
		.map((c, i) => row(c, false, i + 101))
		.join("");
	const archived = chats.filter((c) => c.archived);
	$("#archive-count").textContent = archived.length;
	$("#archive-content").innerHTML = archived.length
		? archived.map((c, i) => row(c, true, i + 1)).join("")
		: '<p class="empty">No archived chats</p>';
	$("#archive-content").hidden = variant !== "v1" || !expanded;
	$("#archive-toggle").setAttribute("aria-expanded", expanded);
	$("#archive-toggle-icon").innerHTML = icon(expanded ? "down" : "right");
	$("#archive-results").innerHTML = archived.length
		? archived
				.map(
					(c, i) =>
						'<div class="chatrow" data-id="archive-result-' +
						(i + 1) +
						'"><button class="task-row chat-link" data-chat="' +
						c.id +
						'" title="' +
						escape(c.title) +
						'">' +
						"<span>" +
						escape(c.title) +
						'</span></button><button class="row-more" data-menu="' +
						c.id +
						'" aria-haspopup="menu" aria-label="More for ' +
						escape(c.title) +
						'">' +
						icon("more") +
						"</button></div>" +
						'<small style="display:block;padding:0 8px 12px">' +
						escape(c.project || "Chats") +
						"</small>",
				)
				.join("")
		: '<p class="empty">No archived chats</p>';
	const current = chat(active);
	$("#chat-title").textContent = current?.title || "New chat";
	$("#chat-title").title = current?.title || "New chat";
	$("#conversation-title").textContent =
		current?.title || "What are we working on?";
	$("#archived-line").hidden = !current?.archived;
	$("#composer-wrap").hidden = !!current?.archived;
	$("#conversation").hidden = !current;
	$("#archive-state").textContent = current?.archived ? "Archived" : "";
	renderTasks();
}
function renderTasks() {
	const fresh = now
			? tasks.filter((t) => t.owner === "You")
			: tasks.filter((t) => !t.stale && t.owner === "You"),
		stale = tasks.filter((t) => t.stale);
	$("#task-count").textContent = tasks.filter((t) => t.owner === "You").length;
	$("#questions-count").textContent = fresh.length;
	$("#stale-count").textContent = stale.length;
	const make = (t) =>
		'<button class="task-row" data-task="' +
		t.id +
		'" data-id="task-row-' +
		t.id +
		'"><span>' +
		escape(t.title) +
		"</span>" +
		"<small>" +
		escape(now ? t.giver : t.owner === "You" ? "For you" : t.owner) +
		" · " +
		escape(chat(t.chat)?.title || "Chat") +
		"</small>" +
		(t.stale && !now
			? '<small class="task-meta"><span>Stale ' +
				escape(t.since) +
				"</span>" +
				(t.owner === "You" ? "" : "<span>" + escape(t.state) + "</span>") +
				"</small>"
			: "") +
		"</button>";
	$("#fresh-tasks").innerHTML =
		fresh.map(make).join("") || '<p class="empty">No questions</p>';
	$("#stale-tasks").innerHTML =
		stale.map(make).join("") || '<p class="empty">No stale tasks</p>';
	$("#stale-section").hidden = now;
}
function closePanel(id) {
	$(id).hidden = true;
}
function hideMenu() {
	if ($("#chat-menu").matches(":popover-open")) $("#chat-menu").hidePopover();
}
function openPanel(id, trigger) {
	const target = $(id);
	const wasOpen = !target.hidden;
	["#tasks-flyout", "#archive-flyout"].forEach(closePanel);
	if (!wasOpen) {
		back = trigger;
		target.hidden = false;
		target.querySelector("button").focus();
	}
}
function showMenu(id, trigger, header = false) {
	menuChat = id;
	back = trigger;
	const c = chat(id);
	if (!c) return;
	$("#menu-pause").hidden = !header || c.archived;
	$("#menu-pause-separator").hidden = !header || c.archived;
	$("#menu-archive").innerHTML =
		icon(c.archived ? "restore" : "archive") +
		(c.archived ? "Restore" : "Archive");
	const menu = $("#chat-menu");
	hideMenu();
	menu.showPopover();
	const r = trigger.getBoundingClientRect();
	menu.style.left =
		Math.max(
			8,
			Math.min(r.right - menu.offsetWidth, innerWidth - menu.offsetWidth - 8),
		) + "px";
	menu.style.top =
		Math.min(r.bottom + 8, innerHeight - menu.offsetHeight - 8) + "px";
	menu.querySelector("button:not([hidden])").focus();
}
function confirmAction(kind, id, trigger) {
	action = { kind, id };
	if (trigger) back = trigger;
	hideMenu();
	$("#confirm-title").textContent =
		kind[0].toUpperCase() +
		kind.slice(1) +
		" " +
		(kind === "cancel" ? "task" : chat(id)?.title) +
		"?";
	$("#confirm-go").textContent = kind[0].toUpperCase() + kind.slice(1);
	$("#confirm-go").className =
		"pill " + (kind === "delete" || kind === "cancel" ? "danger" : "primary");
	$("#confirm-error").hidden = true;
	$("#confirm").showModal();
	$("#confirm-cancel").focus();
}
function closeConfirm() {
	$("#confirm").close();
	back?.focus();
}
function taskDetail(id) {
	selectedTask = tasks.find((t) => t.id === Number(id));
	draft = drafts[id] || "";
	if (!selectedTask) return;
	active = selectedTask.chat;
	render();
	closePanel("#tasks-flyout");
	$("#workspace").hidden = false;
	$("#task-title").textContent = selectedTask.title;
	$("#task-source").textContent =
		selectedTask.giver + " · " + chat(active).title;
	$("#task-stale").textContent = selectedTask.stale
		? "Stale " + selectedTask.since
		: "";
	$("#task-stale").hidden = !selectedTask.stale;
	const person = selectedTask.owner === "You";
	$("#person-answer").hidden = !person;
	$("#agent-detail").hidden = person;
	$("#agent-owner").textContent = selectedTask.owner;
	$("#agent-state").textContent = selectedTask.state || "Ready";
	$("#agent-reason").textContent =
		selectedTask.state === "Waiting on CI"
			? "CI is still open."
			: "No progress recorded.";
	$("#task-send").hidden = !person;
	$("#task-restart").hidden = person;
	$("#task-restart").disabled = selectedTask.state === "Waiting on CI";
	$("#task-ready").textContent = person ? "0 ready" : "";
	$("#task-answer").value = draft;
	$("#task-send").disabled = !draft.trim();
	$("#task-title").focus();
}
function reset(scale = "normal") {
	chats = structuredClone(initialChats);
	tasks = structuredClone(initialTasks);
	if (scale === "large") {
		for (let i = 0; i < 60; i++)
			chats.push({
				id: "old-" + i,
				title: "Archived review " + (i + 1) + " for the desktop launch",
				project: i % 2 ? "Fieldnotes" : "",
				archived: true,
			});
		for (let i = 0; i < 28; i++)
			tasks.push({
				id: 100 + i,
				title: "Review the navigation change " + (i + 1),
				owner: i % 2 ? "Mira" : "You",
				giver: "Rowan",
				chat: "chat-2",
				stale: i > 13,
				since: "4 days",
				state: "Blocked",
			});
	}
	if (scale === "empty") {
		chats = chats.filter((c) => !c.archived);
		tasks = [];
	}
	active = "chat-1";
	expanded = false;
	drafts = {};
	draft = "";
	$("#task-answer").value = "";
	$("#composer-text").value = "";
	$("#workspace").hidden = true;
	$("#archive-flyout").hidden = true;
	$("#tasks-flyout").hidden = true;
	render();
}
document.addEventListener("click", (e) => {
	const b = e.target.closest("button");
	if (!b) return;
	if (b.dataset.menu) return showMenu(b.dataset.menu, b);
	if (b.dataset.chat) {
		active = b.dataset.chat;
		hideMenu();
		$("#archive-flyout").hidden = true;
		$("#sidebar").classList.remove("mobile-open");
		render();
		return;
	}
	if (b.dataset.task) return taskDetail(b.dataset.task);
	if (b.dataset.close) {
		closePanel("#" + b.dataset.close);
		back?.focus();
	}
});
$("#header-more").onclick = (e) => showMenu(active, e.currentTarget, true);
$("#menu-archive").onclick = () => {
	if (chat(menuChat).archived) {
		chat(menuChat).archived = false;
		hideMenu();
		render();
		back?.focus();
	} else confirmAction("archive", menuChat);
};
$("#menu-delete").onclick = () => confirmAction("delete", menuChat);
$("#menu-pause").onclick = () => {
	hideMenu();
	$("#run-line").hidden = !$("#run-line").hidden;
	back?.focus();
};
$("#confirm-cancel").onclick = closeConfirm;
$("#confirm").addEventListener("cancel", (e) => {
	e.preventDefault();
	closeConfirm();
});
$("#confirm-go").onclick = () => {
	if ($("#simulate-failure").checked) {
		$("#confirm-error").hidden = false;
		$("#confirm-error-text").textContent =
			action.kind[0].toUpperCase() + action.kind.slice(1) + " failed";
		return;
	}
	if (action.kind === "cancel")
		tasks = tasks.filter((t) => t.id !== selectedTask.id);
	else {
		tasks = tasks.filter((t) => t.chat !== action.id);
		if (action.kind === "archive") chat(action.id).archived = true;
		if (action.kind === "delete") {
			chats = chats.filter((c) => c.id !== action.id);
			if (active === action.id)
				active = chats.find((c) => !c.archived)?.id || "";
		}
	}
	const wasCurrent = action.id === active;
	$("#workspace").hidden = true;
	$("#confirm").close();
	render();
	if (action.kind === "archive" && wasCurrent) $("#restore-chat").focus();
	else if (action.kind === "cancel") $("#tasks-open").focus();
	else
		(
			document.querySelector('[data-chat="' + active + '"]') || $("#new-chat")
		).focus();
};
$("#restore-chat").onclick = () => {
	chat(active).archived = false;
	render();
	$("#composer-text").focus();
};
$("#archive-toggle").onclick = () => {
	expanded = !expanded;
	render();
};
$("#archive-open").onclick = (e) =>
	openPanel("#archive-flyout", e.currentTarget);
$("#tasks-open").onclick = (e) => openPanel("#tasks-flyout", e.currentTarget);
$("#theme").onclick = () => {
	document.documentElement.classList.toggle("light");
};
$("#reset").onclick = () => reset($("#sample-size").value);
$("#sample-size").onchange = (e) => reset(e.target.value);
$("#settings-open").onclick = (e) => {
	back = e.currentTarget;
	$("#settings").showModal();
};
$("#settings-close").onclick = () => {
	$("#settings").close();
	back?.focus();
};
$("#settings").addEventListener("cancel", (e) => {
	e.preventDefault();
	$("#settings").close();
	back?.focus();
});
$("#startup").onclick = (e) =>
	e.currentTarget.setAttribute(
		"aria-checked",
		e.currentTarget.getAttribute("aria-checked") !== "true",
	);
$("#task-answer").oninput = (e) => {
	draft = e.target.value;
	drafts[selectedTask.id] = draft;
	$("#task-ready").textContent = draft.trim() ? "1 ready" : "0 ready";
	$("#task-send").disabled = !draft.trim();
};
$("#task-send").onclick = () => {
	tasks = tasks.filter((t) => t.id !== selectedTask.id);
	draft = "";
	$("#workspace").hidden = true;
	render();
	$("#tasks-open").focus();
};
$("#task-cancel").onclick = () => {
	tasks = tasks.filter((t) => t.id !== selectedTask.id);
	$("#workspace").hidden = true;
	render();
	$("#tasks-open").focus();
};
$("#task-restart").onclick = () => {
	selectedTask.stale = false;
	selectedTask.state = "Ready";
	render();
	taskDetail(selectedTask.id);
};
$("#mobile-nav").onclick = () => $("#sidebar").classList.toggle("mobile-open");
$("#new-chat").onclick = () => {
	active = "";
	render();
	$("#composer-text").focus();
};
$("#help-stale-button").onclick = (e) =>
	showHelp(
		e.currentTarget,
		"Mark tasks stale after this many days without progress. Turns, messages, answers and fired waits count as progress.",
	);
$("#help-cancel-button").onclick = (e) =>
	showHelp(
		e.currentTarget,
		"Cancel agent tasks this many days after they become stale. Open waits, reviews, CI and running commands keep them open. Your questions stay open until you answer or cancel them.",
	);
function showHelp(button, text) {
	const p = $("#help");
	p.textContent = text;
	p.showPopover();
	const r = button.getBoundingClientRect();
	p.style.left =
		Math.max(20, Math.min(r.left, innerWidth - p.offsetWidth - 20)) + "px";
	p.style.top =
		Math.min(r.bottom + 6, innerHeight - p.offsetHeight - 20) + "px";
}
for (const id of ["#stale-days", "#cancel-days"])
	$(id).addEventListener("change", () => {
		const values = [$("#stale-days").value, $("#cancel-days").value];
		const valid = values.every((v) => /^\d+$/.test(v) && Number(v) >= 1);
		$("#validation").hidden = valid;
		$("#saved").textContent = valid ? "Saved" : "";
	});
document.addEventListener("keydown", (e) => {
	if (e.key === "Escape") {
		if (
			$("#confirm").open ||
			$("#settings").open ||
			$("#help").matches(":popover-open") ||
			$("#chat-menu").matches(":popover-open")
		)
			return;
		$("#tasks-flyout").hidden = true;
		$("#archive-flyout").hidden = true;
		$("#sidebar").classList.remove("mobile-open");
		back?.focus();
	}
	if (
		$("#chat-menu").matches(":popover-open") &&
		["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)
	) {
		e.preventDefault();
		const buttons = [
			...$("#chat-menu").querySelectorAll("button:not([hidden])"),
		];
		let i = buttons.indexOf(document.activeElement);
		i =
			e.key === "Home"
				? 0
				: e.key === "End"
					? buttons.length - 1
					: (i + (e.key === "ArrowDown" ? 1 : -1) + buttons.length) %
						buttons.length;
		buttons[i].focus();
	}
});
document.addEventListener("pointerdown", (e) => {
	if (
		e.target.closest(".flyout,.sidebar-actions,#archive-open,#archive-toggle")
	)
		return;
	$("#tasks-flyout").hidden = true;
	$("#archive-flyout").hidden = true;
});
document.documentElement.classList.toggle(
	"light",
	location.hash === "#light" ||
		new URLSearchParams(location.search).get("theme") === "light",
);
reset();
if (now) {
	$("#archive-group").hidden = true;
	$("#archive-open").hidden = true;
	$("#failure-control").hidden = true;
	$("#menu-archive").hidden = true;
	$("#menu-delete").hidden = true;
	$("#menu-pause-separator").hidden = true;
	$("#chat-menu")
		.querySelectorAll("hr")
		.forEach((n) => (n.hidden = true));
	$("#tasks-flyout h3").hidden = true;
} else {
	$("#archive-group").hidden = variant !== "v1";
	$("#archive-open").hidden = variant !== "v2";
}
