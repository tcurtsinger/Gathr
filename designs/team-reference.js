(() => {
	const thread = document.querySelector("#round-thread");
	const toggle = document.querySelector("#thread-toggle");
	const summary = document.querySelector("#summary");
	const triggers = [...document.querySelectorAll('[popovertarget="summary"]')];
	const text = (id, value) => { document.getElementById(id).textContent = value; };
	let manuallySet = false;
	let anchor = triggers[0];
	toggle.addEventListener("click", () => { manuallySet = true; });
	const log = document.querySelector("#rowan-tool-log");
	for (let call = 1; call <= 32; call++) {
		log.textContent += `${call}. ${call % 2 ? "Read cursor replay fixture" : "Checked interruption boundary"}\n`;
	}
	function showResult(phase) {
		const done = phase !== "running";
		const late = phase === "late";
		document.querySelector("#round-final").hidden = !done;
		document.querySelector("#late-member").hidden = !late;
		document.querySelector("#round-finish").disabled = done;
		document.querySelector("#round-late").disabled = done;
		text("round-state", late ? "Late reply" : done ? "Finished" : "Running");
		text("final-time", late ? "11:35" : "10:03");
		text("thread-state", late ? "Sol replied at 11:32 · Rowan, Kit and Lane done"
			: done ? "Rowan, Kit and Lane replied · Sol reviewed" : "Kit and Lane working · Rowan done · Sol waiting");
		text("kit-result", done ? "All interruption cases passed. Tab, Enter and Escape keep the existing behavior."
			: "The interrupted tool result stays in its original row. I’m checking the last shortcut case now.");
		text("lane-result", done ? "Reconnect, Stop and Retry keep the partial answer and the draft at both window sizes."
			: "The partial reply stays visible after Stop. I’m checking its Retry action at the narrower window size.");
		text("summary-mira-state", done ? "Ready" : "Waiting for Kit and Lane");
		text("summary-mira-task", done ? "No assigned tasks" : "Reconcile results, then ask Sol to review");
		for (const member of ["kit", "lane"]) {
			text(`summary-${member}-state`, done ? "Ready" : "Working");
		}
		text("summary-kit-task", done ? "Interruption and shortcut checks done" : "Check interruptions and shortcuts");
		text("summary-lane-task", done ? "Reconnect and stopped checks done" : "Check reconnect and stopped states");
		text("summary-sol-state", done ? "Ready" : "Waiting for Kit");
		text("summary-sol-task", done ? "Review done" : "Review after replay checks");
		if (!manuallySet) thread.open = !done;
	}
	document.querySelector("#round-finish").onclick = () => showResult("finished");
	document.querySelector("#round-late").onclick = () => showResult("late");
	document.querySelector("#round-reset").onclick = () => {
		manuallySet = false;
		showResult("running");
	};
	document.querySelector("#thread-back").onclick = (event) => {
		event.preventDefault();
		manuallySet = true;
		thread.open = true;
		toggle.focus({ preventScroll: true });
		toggle.scrollIntoView({ block: "start" });
	};
	function placeSummary() {
		if (!summary.matches(":popover-open")) return;
		const at = anchor.id === "team-summary-open"
			? document.querySelector("#team-header").getBoundingClientRect() : anchor.getBoundingClientRect();
		summary.style.left = `${Math.max(16, Math.min(at.right - summary.offsetWidth - 8, innerWidth - summary.offsetWidth - 16))}px`;
		summary.style.top = `${Math.max(16, Math.min(at.bottom + 8, innerHeight - summary.offsetHeight - 16))}px`;
	}
	for (const trigger of triggers) {
		trigger.setAttribute("aria-expanded", "false");
		trigger.addEventListener("click", () => { anchor = trigger; });
	}
	summary.addEventListener("toggle", (event) => {
		for (const trigger of triggers) {
			trigger.setAttribute("aria-expanded", String(event.newState === "open" && trigger === anchor));
		}
		placeSummary();
	});
	window.addEventListener("resize", placeSummary);
	window.addEventListener("scroll", placeSummary, { passive: true });
})();
