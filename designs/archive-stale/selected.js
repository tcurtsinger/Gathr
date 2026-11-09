(() => {
	const baseRender = renderTasks;
	const idleCopy = () => {
		document
			.querySelectorAll(".task-meta > span, #task-stale")
			.forEach((node) => {
				node.textContent = node.textContent.replace(/^Stale (?=\d)/, "Idle ");
			});
	};
	renderTasks = () => {
		baseRender();
		idleCopy();
	};
	const baseDetail = taskDetail;
	taskDetail = (id) => {
		baseDetail(id);
		idleCopy();
	};
	initialTasks.find((task) => task.id === 35).since = "12 days";
	["#stale-days", "#cancel-days"].forEach((id) => {
		const input = document.querySelector(id);
		input.max = "365";
		input.addEventListener("change", () => {
			const inputs = ["#stale-days", "#cancel-days"].map((name) =>
				document.querySelector(name),
			);
			const valid = inputs.every(
				(node) =>
					/^\d+$/.test(node.value) &&
					Number(node.value) >= 1 &&
					Number(node.value) <= 365,
			);
			document.querySelector("#validation").hidden = valid;
			document.querySelector("#validation").textContent =
				"Enter whole days from 1 to 365.";
			document.querySelector("#saved").textContent = valid ? "Saved" : "";
		});
	});
	reset();
})();
