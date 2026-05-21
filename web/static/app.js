(function () {
  // Shortcuts overlay
  const overlay  = document.getElementById('shortcuts-overlay');
  const closeBtn = document.getElementById('shortcutsClose');

  function openShortcuts() {
    if (overlay) overlay.hidden = false;
  }

  function closeShortcuts() {
    if (overlay) overlay.hidden = true;
  }

  if (closeBtn) closeBtn.addEventListener('click', closeShortcuts);
  if (overlay)  overlay.addEventListener('click', e => { if (e.target === overlay) closeShortcuts(); });

  // Keyboard navigation for the runs table
  let focusedRow = -1;
  const rows = () => Array.from(document.querySelectorAll('.runs-table tbody tr:not(.table-empty)'));

  function focusRow(idx) {
    const all = rows();
    if (all.length === 0) return;
    if (focusedRow >= 0 && focusedRow < all.length) {
      all[focusedRow].classList.remove('row-focused');
    }
    focusedRow = Math.max(0, Math.min(idx, all.length - 1));
    const row = all[focusedRow];
    row.classList.add('row-focused');
    row.scrollIntoView({ block: 'nearest' });
  }

  function openFocusedRow() {
    const all = rows();
    if (focusedRow < 0 || focusedRow >= all.length) return;
    const link = all[focusedRow].querySelector('a');
    if (link) link.click();
  }

  function compareSelected() {
    const checked = Array.from(document.querySelectorAll('.runs-table input[type=checkbox]:checked'))
      .map(cb => cb.closest('tr'))
      .filter(Boolean)
      .map(tr => tr.dataset.runId)
      .filter(Boolean);
    if (checked.length < 2) return;
    const [a, b] = checked;
    // Navigate to the compare page in the current org/project context.
    const parts = window.location.pathname.split('/').filter(Boolean);
    if (parts.length >= 2) {
      window.location.href = `/${parts[0]}/${parts[1]}/compare?a=${a}&b=${b}`;
    }
  }

  document.addEventListener('keydown', e => {
    const tag = document.activeElement ? document.activeElement.tagName : '';
    const inputType = (document.activeElement && document.activeElement.type) || '';
    const inInput = (tag === 'INPUT' && inputType !== 'checkbox') || tag === 'TEXTAREA' || tag === 'SELECT';

    if (e.key === '?') {
      e.preventDefault();
      openShortcuts();
      return;
    }

    if (e.key === 'Escape') {
      closeShortcuts();
      if (inInput) document.activeElement.blur();
      return;
    }

    if (inInput) return;

    if (e.key === 'j') {
      e.preventDefault();
      focusRow(focusedRow + 1);
    } else if (e.key === 'k') {
      e.preventDefault();
      focusRow(focusedRow - 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      openFocusedRow();
    } else if (e.key === '/') {
      e.preventDefault();
      const filter = document.getElementById('filterInput');
      if (filter) { filter.focus(); filter.select(); }
    } else if (e.key === 'c') {
      e.preventDefault();
      compareSelected();
    }
  });

  // Show/hide compare hint when rows are checked
  const checkAll    = document.getElementById('checkAll');
  const compareHint = document.getElementById('compareHint');

  function updateCompareHint() {
    if (!compareHint) return;
    const n = document.querySelectorAll('.runs-table input[type=checkbox]:checked').length;
    compareHint.hidden = n < 2;
  }

  if (checkAll) {
    checkAll.addEventListener('change', () => {
      document.querySelectorAll('.runs-table tbody input[type=checkbox]')
        .forEach(cb => { cb.checked = checkAll.checked; });
      updateCompareHint();
    });
  }

  document.addEventListener('change', e => {
    if (e.target.closest('.runs-table') && e.target.type === 'checkbox') {
      updateCompareHint();
    }
  });
})();
