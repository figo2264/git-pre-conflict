const { invoke } = window.__TAURI__.core;

const branchEl = document.getElementById("current-branch");
const repoPathEl = document.getElementById("repo-path");
const browseBtn = document.getElementById("browse-btn");
const targetSelect = document.getElementById("target");
const refreshBranchesBtn = document.getElementById("refresh-branches-btn");
const checkBtn = document.getElementById("check-btn");
const resultsEl = document.getElementById("results");
const intervalInput = document.getElementById("interval");
const watchBtn = document.getElementById("watch-btn");
const watchStatusEl = document.getElementById("watch-status");
const lastCheckEl = document.getElementById("last-check");

let isWatching = false;
let currentRepoPath = null;

// --- Init ---
async function init() {
  await loadCurrentBranch();
  await loadBranches();

  // Restore watch status if the backend is still watching
  try {
    const status = await invoke("get_watch_status");
    if (status.isWatching) {
      if (status.repoPath) {
        currentRepoPath = status.repoPath;
        repoPathEl.textContent = currentRepoPath;
        repoPathEl.title = currentRepoPath;
      }
      setWatching(true, status.target);
      if (status.lastReport) {
        renderReport(status.lastReport);
      }
      // Reload branch info for the watched repo
      await loadCurrentBranch();
      await loadBranches();
    }
  } catch (_) {}
}

async function loadCurrentBranch() {
  try {
    const branch = await invoke("get_current_branch", {
      repoPath: currentRepoPath,
    });
    branchEl.textContent = branch;
    branchEl.style.color = "";
  } catch (e) {
    branchEl.textContent = "not in a git repo";
    branchEl.style.color = "#d44";
  }
}

async function loadBranches() {
  targetSelect.innerHTML = '<option value="">Loading...</option>';
  try {
    const branches = await invoke("list_branches", {
      repoPath: currentRepoPath,
    });
    targetSelect.innerHTML = "";

    if (branches.length === 0) {
      targetSelect.innerHTML = '<option value="">No branches found</option>';
      return;
    }

    for (const b of branches) {
      const opt = document.createElement("option");
      opt.value = b;
      opt.textContent = b;
      targetSelect.appendChild(opt);
    }

    // Auto-select main or master if present
    const preferred = branches.find(
      (b) => b === "main" || b === "origin/main"
    ) || branches.find(
      (b) => b === "master" || b === "origin/master"
    );
    if (preferred) {
      targetSelect.value = preferred;
    }
  } catch (e) {
    targetSelect.innerHTML = `<option value="">Error loading branches</option>`;
  }
}

// --- Browse ---
browseBtn.addEventListener("click", async () => {
  try {
    const path = await invoke("pick_directory");
    if (path) {
      currentRepoPath = path;
      repoPathEl.textContent = path;
      repoPathEl.title = path;
      await loadCurrentBranch();
      await loadBranches();
    }
  } catch (e) {
    resultsEl.className = "results conflicts";
    resultsEl.innerHTML = `<p class="error-msg">${escapeHtml(String(e))}</p>`;
  }
});

// --- Refresh branches ---
refreshBranchesBtn.addEventListener("click", () => loadBranches());

// --- Check ---
checkBtn.addEventListener("click", async () => {
  const target = targetSelect.value;
  if (!target) return;

  checkBtn.disabled = true;
  checkBtn.textContent = "Checking...";
  resultsEl.className = "results";

  try {
    const report = await invoke("check_conflicts", {
      target,
      noFetch: false,
      repoPath: currentRepoPath,
    });
    renderReport(report);
  } catch (e) {
    resultsEl.className = "results conflicts";
    resultsEl.innerHTML = `<p class="error-msg">${escapeHtml(String(e))}</p>`;
  } finally {
    checkBtn.disabled = false;
    checkBtn.textContent = "Check Now";
    lastCheckEl.textContent = "checked " + new Date().toLocaleTimeString();
  }
});

// --- Watch ---
watchBtn.addEventListener("click", async () => {
  if (isWatching) {
    try {
      await invoke("stop_watch");
    } catch (_) {}
    setWatching(false);
    return;
  }

  const target = targetSelect.value;
  if (!target) return;

  const intervalSecs = Math.max(10, parseInt(intervalInput.value, 10) || 300);

  try {
    await invoke("start_watch", {
      target,
      intervalSecs,
      repoPath: currentRepoPath,
    });
    setWatching(true, target);
  } catch (e) {
    resultsEl.className = "results conflicts";
    resultsEl.innerHTML = `<p class="error-msg">${escapeHtml(String(e))}</p>`;
  }
});

// Poll watch status to update UI
let pollTimer = null;

function setWatching(active, target) {
  isWatching = active;
  if (active) {
    watchBtn.textContent = "Stop Watching";
    watchBtn.classList.add("watching");
    watchStatusEl.textContent = `Watching: ${target}`;
    startPolling();
  } else {
    watchBtn.textContent = "Start Watching";
    watchBtn.classList.remove("watching");
    watchStatusEl.textContent = "Not watching";
    stopPolling();
  }
}

function startPolling() {
  stopPolling();
  pollTimer = setInterval(async () => {
    try {
      const status = await invoke("get_watch_status");
      if (!status.isWatching) {
        setWatching(false);
        return;
      }
      if (status.lastReport) {
        renderReport(status.lastReport);
        lastCheckEl.textContent = "updated " + new Date().toLocaleTimeString();
      }
    } catch (_) {}
  }, 3000);
}

function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

// --- Render ---
function renderReport(report) {
  if (report.conflicted_files.length === 0) {
    resultsEl.className = "results clean";
    resultsEl.innerHTML = `
      <p class="result-header clean">No conflicts detected</p>
      <p class="check-info">${escapeHtml(report.current_branch)} → ${escapeHtml(report.target_ref)}</p>
    `;
  } else {
    resultsEl.className = "results conflicts";
    const count = report.conflicted_files.length;
    const fileItems = report.conflicted_files
      .map((f) => `<li>${escapeHtml(f)}</li>`)
      .join("");
    resultsEl.innerHTML = `
      <p class="result-header conflicts">${count} conflicted file${count === 1 ? "" : "s"}</p>
      <p class="check-info">${escapeHtml(report.current_branch)} → ${escapeHtml(report.target_ref)}</p>
      <ul class="file-list">${fileItems}</ul>
    `;
  }
}

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

init();
