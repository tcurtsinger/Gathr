(() => {
	const get = (selector) => document.querySelector(selector);
	const text = (id, value) => {
		document.getElementById(id).textContent = value;
	};
	const summary = get("#summary");
	const triggers = [...document.querySelectorAll('[popovertarget="summary"]')];
	let anchor = triggers[0];
	const toggle = get("#split-toggle");
	const team = get("#team-column");
	const supervisor = get("#supervisor-scroll");
	let positions = [0, 0];
	function setSplit(open) {
		positions = [
			supervisor.scrollTop,
			team.hidden ? positions[1] : team.scrollTop,
		];
		team.hidden = !open;
		get("#supervisor-label").hidden = !open;
		toggle.setAttribute("aria-pressed", String(open));
		toggle.setAttribute(
			"aria-label",
			open ? "Hide team chat" : "Show team chat",
		);
		toggle.title = toggle.getAttribute("aria-label");
		if (open) {
			get("#team-unread").hidden = true;
			get("#late-unread").hidden = true;
		}
		supervisor.scrollTop = positions[0];
		team.scrollTop = positions[1];
	}
	toggle.onclick = () => setSplit(team.hidden);
	get("#waiting-on-team").onclick = () => setSplit(true);
	const log = get("#rowan-tool-log");
	for (let call = 1; call <= 32; call++) {
		log.textContent += `${call}. ${call % 2 ? "Read cursor replay fixture" : "Checked interruption boundary"}\n`;
	}
	// Invented fixture records, not a production data adapter. No prose parsing or timers.
	const tasks = [...get("#agent-plan").querySelectorAll("li")].map((row) => ({
		owner: row.dataset.owner.toLowerCase(),
		state: row.dataset.state,
		row,
	}));
	const initial = tasks.map((task) => task.state);
	function showResult(phase) {
		const done = phase === "finished" || phase === "late";
		const outcome = ["blocked", "failed", "stopped"].includes(phase)
			? phase
			: null;
		const late = phase === "late";
		get("#round-final").hidden = !done;
		get("#late-member").hidden = !late;
		get("#waiting-on-team").hidden = done;
		get("#team-working").hidden = done;
		get("#round-finish").disabled = done;
		get("#round-late").disabled = late;
		text(
			"round-state",
			late
				? "Late reply"
				: done
					? "Finished"
					: outcome
						? outcome[0].toUpperCase() + outcome.slice(1)
						: "Running",
		);
		text("final-time", late ? "11:35" : "10:03");
		text(
			"plan-state",
			done
				? "Done"
				: outcome
					? `Lane working, 1 ${outcome}`
					: "Kit and Lane working",
		);
		text(
			"kit-result",
			done
				? "All interruption cases passed. Tab, Enter and Escape keep the existing behavior."
				: "The interrupted tool result stays in its original row. I’m checking the last shortcut case now.",
		);
		text(
			"lane-result",
			done
				? "Reconnect, Stop and Retry keep the partial answer and the draft at both window sizes."
				: "The partial reply stays visible after Stop. I’m checking its Retry action at the narrower window size.",
		);
		tasks.forEach((task, index) => {
			task.state = done
				? "done"
				: task.owner === "kit" && outcome
					? outcome
					: initial[index];
			task.row.dataset.state = task.state;
			const item = get(`#summary-${task.owner}-task`);
			item.dataset.state = task.state;
			const count = task.state === "done" ? 1 : 0;
			text(`summary-${task.owner}-count`, `${count}/1`);
			get(`[data-agent="${task.owner}"] progress`).value = count;
			get(`#summary-${task.owner}-state`).dataset.state = task.state;

			text(
				`summary-${task.owner}-state`,
				task.state === "active" ? "Working" : "Ready",
			);
			get(".ds-segments").children[index].dataset.state = task.state;
		});
		// Only recorded fixture detail is shown, and only while its step is active.
		text(
			"step-kit-detail",
			done
				? ""
				: outcome === "blocked"
					? "Waiting for the replay fixture"
					: outcome === "failed"
						? "Replay command failed (ref sample-02)"
						: outcome === "stopped"
							? ""
							: "Checked Tab, Enter and Escape",
		);
		text("step-lane-detail", "");
		for (const task of tasks) {
			text(
				`step-${task.owner}-outcome`,
				["blocked", "failed", "stopped"].includes(task.state)
					? task.state[0].toUpperCase() + task.state.slice(1)
					: "",
			);
		}
		text(
			"summary-mira-state",
			done
				? "Ready"
				: outcome
					? "Waiting for Lane and Sol since 09:15"
					: "Waiting for Kit, Lane and Sol since 09:15",
		);
		get("#waiting-on-team").title = done
			? ""
			: outcome
				? "Mira is waiting for Lane and Sol, since 09:15"
				: "Mira is waiting for Kit, Lane and Sol, since 09:15";
		for (const workers of document.querySelectorAll(".ds-workers")) {
			workers.setAttribute("aria-label", outcome ? "Lane" : "Kit and Lane");
			workers.innerHTML = outcome ? "<i>L</i>" : "<i>K</i><i>L</i>";
		}
		const count = tasks.filter((task) => task.state === "done").length;
		text("plan-count", `${count}/${tasks.length}`);
		get(".ds-segments").setAttribute("aria-valuenow", String(count));
		get("#plan-toggle .ds-spinner").hidden = done;
		get("#plan-done").hidden = !done;
		if (late && team.hidden) {
			get("#team-unread").hidden = false;
			get("#late-unread").hidden = false;
		}
	}
	for (const state of ["blocked", "failed", "stopped"]) {
		get(`#round-${state}`).onclick = () => showResult(state);
	}
	get("#round-finish").onclick = () => showResult("finished");
	get("#round-late").onclick = () => showResult("late");
	get("#round-reset").onclick = () => {
		showResult("running");
		get("#team-unread").hidden = true;
		get("#late-unread").hidden = true;
	};
	get("#agent-action-close").onclick = () =>
		get("#agent-actions").hidePopover();
	showResult("running");
	function placeSummary() {
		if (!summary.matches(":popover-open")) return;
		const at =
			anchor.id === "team-summary-open"
				? document.querySelector("#team-header").getBoundingClientRect()
				: anchor.getBoundingClientRect();
		const size = summary.getBoundingClientRect();
		summary.style.left = `${Math.max(16, Math.min(at.right - size.width - 8, innerWidth - size.width - 16))}px`;
		summary.style.top = `${Math.max(16, Math.min(at.bottom + 8, innerHeight - size.height - 16))}px`;
	}
	for (const trigger of triggers) {
		trigger.setAttribute("aria-expanded", "false");
		trigger.addEventListener("click", () => {
			anchor = trigger;
		});
	}
	summary.addEventListener("toggle", (event) => {
		for (const trigger of triggers) {
			trigger.setAttribute(
				"aria-expanded",
				String(event.newState === "open" && trigger === anchor),
			);
		}
		placeSummary();
	});
	new ResizeObserver(placeSummary).observe(summary);
	window.addEventListener("resize", placeSummary);
	window.addEventListener("scroll", placeSummary, { passive: true });
})();
