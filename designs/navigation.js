// Use the same navigation at every width. Native popovers provide Escape,
// light dismissal and keyboard focus return on narrow windows.
const navigation = document.querySelector('#navigation');
const narrowNavigation = matchMedia('(max-width:767px)');
function resizeNavigation() {
  if (narrowNavigation.matches) navigation.setAttribute('popover', 'auto');else navigation.removeAttribute('popover');
}
narrowNavigation.addEventListener('change', resizeNavigation);
resizeNavigation();
