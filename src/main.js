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
    return;
  }

  resultsEl.className = "results conflicts";
  const count = report.conflicted_files.length;

  const fileRows = report.conflicted_files.map((f) => {
    const type = f.conflict_type || "unknown";
    const displayType = formatType(type);
    const cls = badgeClass(type);
    return `<div class="conflict-row">
      <span class="conflict-badge ${cls}">${escapeHtml(displayType)}</span>
      <span class="conflict-path" title="${escapeAttr(f.path)}">${escapeHtml(f.path)}</span>
      <button class="btn-diff" data-path="${escapeAttr(f.path)}" onclick="toggleDiff(this, ${escapeAttr(JSON.stringify(report))})">▶</button>
    </div>
    <div class="diff-panel" id="diff-${escapeAttr(f.path)}" style="display:none;"></div>`;
  }).join("");

  resultsEl.innerHTML = `
    <p class="result-header conflicts">${count} conflicted file${count === 1 ? "" : "s"}</p>
    <p class="check-info">${escapeHtml(report.current_branch)} → ${escapeHtml(report.target_ref)}</p>
    ${fileRows}
    <div id="guide-panel"></div>
  `;

  loadGuide(report);
}

function formatType(type) {
  const map = {
    content: "content",
    add_add: "add/add",
    modify_delete: "modify/delete",
    delete_modify: "delete/modify",
    rename_delete: "rename/delete",
    rename_rename: "rename/rename",
    directory_file: "directory/file",
    unknown: "unknown",
  };
  return map[type] || type;
}

function badgeClass(type) {
  if (type === "content" || type === "add_add") return "badge-content";
  if (type === "modify_delete" || type === "delete_modify") return "badge-delete";
  if (type === "rename_delete" || type === "rename_rename") return "badge-rename";
  return "badge-other";
}

// Exposed globally for inline onclick
window.toggleDiff = async function toggleDiff(btn, report) {
  const path = btn.dataset.path;
  const panelId = "diff-" + path;
  const panel = document.getElementById(panelId);
  if (!panel) return;

  if (panel.style.display !== "none") {
    panel.style.display = "none";
    btn.textContent = "▶";
    return;
  }

  panel.style.display = "block";
  btn.textContent = "▼";

  if (panel.dataset.loaded) return;

  panel.innerHTML = '<div class="diff-content">Loading diff...</div>';

  try {
    const diff = await invoke("get_conflict_diff", {
      repoPath: currentRepoPath,
      currentBranch: report.current_branch,
      targetRef: report.target_ref,
      filePath: path,
    });

    if (diff && diff.trim()) {
      panel.innerHTML = `<div class="diff-content">${colorDiff(escapeHtml(diff))}</div>`;
    } else {
      panel.innerHTML = '<div class="diff-content">(no diff available)</div>';
    }
  } catch (e) {
    panel.innerHTML = `<div class="diff-content" style="color:#d44;">${escapeHtml(String(e))}</div>`;
  }

  panel.dataset.loaded = "1";
};

async function loadGuide(report) {
  const guidePanel = document.getElementById("guide-panel");
  if (!guidePanel) return;

  try {
    const guide = await invoke("get_resolution_guide", { report });

    const stepsHtml = guide.commands.map((step, i) => `
      <div class="guide-step">
        <span class="step-num">${i + 1}</span>
        <span class="step-desc">${escapeHtml(step.description)}</span>
        <code class="step-cmd" title="Click to copy" onclick="copyCmd(this)">${escapeHtml(step.command)}</code>
      </div>
    `).join("");

    guidePanel.innerHTML = `
      <div class="guide-title">Resolution Guide</div>
      <p class="guide-summary">${escapeHtml(guide.summary)}</p>
      ${stepsHtml}
    `;
  } catch (e) {
    guidePanel.innerHTML = `<p class="error-msg">${escapeHtml(String(e))}</p>`;
  }
}

window.copyCmd = async function copyCmd(el) {
  try {
    await navigator.clipboard.writeText(el.textContent);
    el.classList.add("copied");
    setTimeout(() => el.classList.remove("copied"), 1200);
  } catch (_) {}
};

function colorDiff(escapedHtml) {
  return escapedHtml
    .split("\n")
    .map((line) => {
      if (line.startsWith("+")) return `<span class="diff-add">${line}</span>`;
      if (line.startsWith("-")) return `<span class="diff-del">${line}</span>`;
      if (line.startsWith("@@")) return `<span class="diff-hunk">${line}</span>`;
      return line;
    })
    .join("\n");
}

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

function escapeAttr(str) {
  return str.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

init();
