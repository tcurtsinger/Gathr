document.querySelector('.startup-switch').addEventListener('click', (event) => {
  const control = event.currentTarget;
  control.setAttribute('aria-checked', String(control.getAttribute('aria-checked') !== 'true'));
});
