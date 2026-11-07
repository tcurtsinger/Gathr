(() => {
  const toggle = document.querySelector('#search-toggle');
  const input = document.querySelector('#chat-search');
  toggle.onclick = () => {
    input.hidden = !input.hidden;
    toggle.setAttribute('aria-expanded', String(!input.hidden));
    if (!input.hidden) input.focus();
  };
  input.oninput = () => {
    const query = input.value.trim().toLowerCase();
    document.querySelectorAll('.chatrow').forEach(row => {
      row.hidden = !row.textContent.toLowerCase().includes(query);
    });
  };
})();
