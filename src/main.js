import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// State
let clipboardHistory = [];
let activeFilter = "all";
let searchQuery = "";
let selectedIndex = 0;
let isPaused = false;
let isRecordingHotkey = false;

let settings = {
  hotkey: "Win+V",
  capacity: 100,
  autoPaste: true,
  feedback: true,
  clearOnExit: false,
};

// DOM Elements
const clipboardListEl = document.getElementById("clipboard-list");
const emptyStateEl = document.getElementById("empty-state");
const searchInputEl = document.getElementById("search-input");
const clearSearchBtn = document.getElementById("btn-clear-search");
const pinnedCountBadge = document.getElementById("pinned-count-badge");
const filterPills = document.querySelectorAll(".filter-pills .pill");
const btnClearAll = document.getElementById("btn-clear-all");
const btnTogglePause = document.getElementById("btn-toggle-pause");
const iconPause = document.getElementById("icon-pause");
const btnSettings = document.getElementById("btn-settings");
const settingsPanel = document.getElementById("settings-panel");
const btnBackSettings = document.getElementById("btn-back-settings");
const btnCloseSettings = document.getElementById("btn-close-settings");
const toastEl = document.getElementById("toast");

// Settings DOM Elements
const settingHotkeyPreset = document.getElementById("setting-hotkey-preset");
const customHotkeyBox = document.getElementById("custom-hotkey-box");
const customHotkeyInput = document.getElementById("custom-hotkey-input");
const btnRecordHotkey = document.getElementById("btn-record-hotkey");
const settingCapacity = document.getElementById("setting-capacity");
const settingAutoPaste = document.getElementById("setting-auto-paste");
const settingFeedback = document.getElementById("setting-feedback");
const settingClearExit = document.getElementById("setting-clear-exit");
const btnDangerClear = document.getElementById("btn-danger-clear");

// Relative time formatting
function formatTimeAgo(timestamp) {
  const diffSec = Math.floor((Date.now() - timestamp) / 1000);
  if (diffSec < 10) return "just now";
  if (diffSec < 60) return `${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHour = Math.floor(diffMin / 60);
  if (diffHour < 24) return `${diffHour}h ago`;
  const diffDay = Math.floor(diffHour / 24);
  return `${diffDay}d ago`;
}

// Toast notification
let toastTimer = null;
function showToast(message) {
  if (!settings.feedback) return;
  toastEl.textContent = message;
  toastEl.style.display = "block";
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastEl.style.display = "none";
  }, 1400);
}

// Filter and search
function getFilteredItems() {
  return clipboardHistory.filter((item) => {
    if (activeFilter === "pinned" && !item.pinned) return false;
    if (searchQuery) {
      return item.text.toLowerCase().includes(searchQuery.toLowerCase());
    }
    return true;
  });
}

// Render Clipboard List
function renderList() {
  const items = getFilteredItems();
  const pinnedCount = clipboardHistory.filter((i) => i.pinned).length;
  pinnedCountBadge.textContent = pinnedCount;

  if (items.length === 0) {
    clipboardListEl.innerHTML = "";
    emptyStateEl.style.display = "flex";
    if (searchQuery) {
      document.getElementById("empty-title").textContent = "No matching items";
      document.getElementById("empty-subtitle").textContent = `No clipboard entries match "${searchQuery}"`;
    } else if (activeFilter === "pinned") {
      document.getElementById("empty-title").textContent = "No pinned snippets";
      document.getElementById("empty-subtitle").textContent = "Click 📌 on any snippet to keep it permanently saved.";
    } else {
      document.getElementById("empty-title").textContent = "Your clipboard is empty";
      document.getElementById("empty-subtitle").textContent = "Copy text anywhere and it will instantly appear here.";
    }
    return;
  }

  emptyStateEl.style.display = "none";

  if (selectedIndex >= items.length) {
    selectedIndex = Math.max(0, items.length - 1);
  }

  clipboardListEl.innerHTML = items
    .map((item, index) => {
      const isSelected = index === selectedIndex;
      const lines = item.text.split("\n").length;
      const chars = item.text.length;
      const previewStats = lines > 1 ? `${lines} lines • ${chars} chars` : `${chars} chars`;
      const isCode = item.type === "code";

      return `
        <div class="clip-card ${item.pinned ? "pinned" : ""} ${isSelected ? "selected" : ""}"
             data-id="${item.id}" data-index="${index}">
          <div class="card-header-row">
            <div class="card-tags">
              <span class="type-badge ${item.type}">${item.type}</span>
            </div>
            <div class="card-actions-hover">
              <button class="card-btn pin-btn ${item.pinned ? "active" : ""}" 
                      title="${item.pinned ? "Unpin snippet" : "Pin snippet"}" 
                      data-action="pin" data-id="${item.id}">
                📌
              </button>
              <button class="card-btn delete-btn" 
                      title="Delete snippet" 
                      data-action="delete" data-id="${item.id}">
                ✕
              </button>
            </div>
          </div>
          <div class="card-snippet ${isCode ? "code-view" : ""}">${escapeHtml(item.text)}</div>
          <div class="card-meta-row">
            <span>${previewStats}</span>
            <span class="time-ago">${formatTimeAgo(item.timestamp)}</span>
          </div>
        </div>
      `;
    })
    .join("");

  const selectedEl = clipboardListEl.querySelector(".clip-card.selected");
  if (selectedEl) {
    selectedEl.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }
}

function escapeHtml(str) {
  if (!str) return "";
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

// Select Item: Copy, Auto-Paste & Close flyout immediately!
async function selectItem(id) {
  try {
    await invoke("copy_item", { id, autoPaste: settings.autoPaste });
    showToast("Copied to clipboard!");
  } catch (err) {
    console.error("Failed to copy item:", err);
  }
}

// Delete item
async function deleteItem(id) {
  try {
    await invoke("delete_item", { id });
    clipboardHistory = clipboardHistory.filter((i) => i.id !== id);
    renderList();
  } catch (err) {
    console.error("Failed to delete item:", err);
  }
}

// Toggle Pin
async function togglePin(id) {
  try {
    const isPinned = await invoke("toggle_pin", { id });
    const item = clipboardHistory.find((i) => i.id === id);
    if (item) {
      item.pinned = isPinned;
      sortHistory();
      renderList();
    }
  } catch (err) {
    console.error("Failed to toggle pin:", err);
  }
}

function sortHistory() {
  clipboardHistory.sort((a, b) => {
    if (a.pinned && !b.pinned) return -1;
    if (!a.pinned && b.pinned) return 1;
    return b.timestamp - a.timestamp;
  });
}

// Load History from Rust backend
async function loadHistory() {
  try {
    const history = await invoke("get_history");
    clipboardHistory = history || [];
    sortHistory();
    renderList();
  } catch (err) {
    console.error("Failed to load history:", err);
  }
}

// Load Settings from Rust backend
async function loadSettings() {
  try {
    const backendSettings = await invoke("get_settings");
    if (backendSettings) {
      settings = { ...settings, ...backendSettings };
    }
  } catch {
    const saved = localStorage.getItem("clipstore_settings");
    if (saved) {
      try {
        settings = { ...settings, ...JSON.parse(saved) };
      } catch (e) {
        console.error("Error parsing settings:", e);
      }
    }
  }
  updateSettingsUI();
}

function updateSettingsUI() {
  // Check if preset matches
  const knownPresets = [
    "Win+V",
    "CommandOrControl+Shift+V",
    "Alt+V",
    "Win+Shift+V",
    "CommandOrControl+Alt+V",
  ];

  if (knownPresets.includes(settings.hotkey)) {
    settingHotkeyPreset.value = settings.hotkey;
    customHotkeyBox.style.display = "none";
  } else {
    settingHotkeyPreset.value = "custom";
    customHotkeyBox.style.display = "flex";
    customHotkeyInput.value = settings.hotkey;
  }

  settingCapacity.value = settings.capacity.toString();
  settingAutoPaste.checked = settings.autoPaste;
  settingFeedback.checked = settings.feedback;
  settingClearExit.checked = settings.clearOnExit;
}

// Save settings to backend
async function saveCurrentSettings() {
  localStorage.setItem("clipstore_settings", JSON.stringify(settings));
  try {
    await invoke("save_settings", { settings });
    showToast("Settings updated");
  } catch (err) {
    console.error("Failed to save settings:", err);
  }
}

// Handle preset change
settingHotkeyPreset.addEventListener("change", () => {
  const val = settingHotkeyPreset.value;
  if (val === "custom") {
    customHotkeyBox.style.display = "flex";
    customHotkeyInput.value = settings.hotkey || "Win+V";
  } else {
    customHotkeyBox.style.display = "none";
    settings.hotkey = val;
    saveCurrentSettings();
  }
});

// Interactive Hotkey Recorder
btnRecordHotkey.addEventListener("click", () => {
  isRecordingHotkey = true;
  btnRecordHotkey.textContent = "Press keys...";
  btnRecordHotkey.classList.add("active");
  customHotkeyInput.classList.add("recording");
  customHotkeyInput.value = "Recording shortcut...";
  customHotkeyInput.focus();
});

customHotkeyInput.addEventListener("keydown", (e) => {
  if (!isRecordingHotkey) return;
  e.preventDefault();
  e.stopPropagation();

  if (e.key === "Escape") {
    isRecordingHotkey = false;
    btnRecordHotkey.textContent = "Record";
    btnRecordHotkey.classList.remove("active");
    customHotkeyInput.classList.remove("recording");
    customHotkeyInput.value = settings.hotkey;
    return;
  }

  const parts = [];
  if (e.metaKey || e.key === "Meta" || e.key === "OS") parts.push("Win");
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");

  const ignored = ["Control", "Shift", "Alt", "Meta", "OS"];
  if (!ignored.includes(e.key)) {
    let keyName = e.key.toUpperCase();
    if (e.code === "Space") keyName = "Space";
    if (e.code === "Insert") keyName = "Insert";
    parts.push(keyName);

    if (parts.length > 1) {
      const combo = parts.join("+");
      customHotkeyInput.value = combo;
      settings.hotkey = combo;
      isRecordingHotkey = false;
      btnRecordHotkey.textContent = "Record";
      btnRecordHotkey.classList.remove("active");
      customHotkeyInput.classList.remove("recording");
      saveCurrentSettings();
      showToast(`Shortcut set to ${combo}`);
    }
  }
});

// Setting changes
settingCapacity.addEventListener("change", () => {
  settings.capacity = parseInt(settingCapacity.value, 10);
  saveCurrentSettings();
});

settingAutoPaste.addEventListener("change", () => {
  settings.autoPaste = settingAutoPaste.checked;
  saveCurrentSettings();
});

settingFeedback.addEventListener("change", () => {
  settings.feedback = settingFeedback.checked;
  saveCurrentSettings();
});

settingClearExit.addEventListener("change", () => {
  settings.clearOnExit = settingClearExit.checked;
  saveCurrentSettings();
});

// Danger clear
btnDangerClear.addEventListener("click", async () => {
  if (clipboardHistory.length === 0) return;
  const confirmed = confirm("Are you sure you want to clear all unpinned items?");
  if (confirmed) {
    try {
      await invoke("clear_all_history");
      clipboardHistory = clipboardHistory.filter((i) => i.pinned);
      renderList();
      showToast("History cleared");
    } catch (err) {
      console.error("Failed to clear history:", err);
    }
  }
});

// Event Listeners for List
clipboardListEl.addEventListener("click", (e) => {
  const pinBtn = e.target.closest('[data-action="pin"]');
  if (pinBtn) {
    e.stopPropagation();
    togglePin(pinBtn.dataset.id);
    return;
  }

  const deleteBtn = e.target.closest('[data-action="delete"]');
  if (deleteBtn) {
    e.stopPropagation();
    deleteItem(deleteBtn.dataset.id);
    return;
  }

  const card = e.target.closest(".clip-card");
  if (card) {
    selectItem(card.dataset.id);
  }
});

// Search input
searchInputEl.addEventListener("input", (e) => {
  searchQuery = e.target.value;
  clearSearchBtn.style.display = searchQuery ? "flex" : "none";
  selectedIndex = 0;
  renderList();
});

clearSearchBtn.addEventListener("click", () => {
  searchInputEl.value = "";
  searchQuery = "";
  clearSearchBtn.style.display = "none";
  selectedIndex = 0;
  renderList();
  searchInputEl.focus();
});

// Filter pills
filterPills.forEach((pill) => {
  pill.addEventListener("click", () => {
    filterPills.forEach((p) => p.classList.remove("active"));
    pill.classList.add("active");
    activeFilter = pill.dataset.filter;
    selectedIndex = 0;
    renderList();
  });
});

// Clear all unpinned button
btnClearAll.addEventListener("click", async () => {
  if (clipboardHistory.length === 0) return;
  const confirmed = confirm("Clear unpinned history?");
  if (confirmed) {
    try {
      await invoke("clear_all_history");
      clipboardHistory = clipboardHistory.filter((i) => i.pinned);
      renderList();
      showToast("History cleared");
    } catch (err) {
      console.error("Failed to clear:", err);
    }
  }
});

// Toggle pause monitoring
btnTogglePause.addEventListener("click", async () => {
  try {
    isPaused = await invoke("toggle_pause");
    btnTogglePause.title = isPaused ? "Monitoring Paused (Click to resume)" : "Monitoring Active";
    btnTogglePause.style.color = isPaused ? "var(--danger)" : "var(--text-muted)";
    showToast(isPaused ? "Monitoring paused" : "Monitoring active");
  } catch (err) {
    console.error("Failed to toggle pause:", err);
  }
});

// Settings Open & Close
btnSettings.addEventListener("click", () => {
  settingsPanel.style.display = "flex";
});

btnBackSettings.addEventListener("click", () => {
  settingsPanel.style.display = "none";
});

btnCloseSettings.addEventListener("click", () => {
  settingsPanel.style.display = "none";
});

// Global Keyboard Navigation
window.addEventListener("keydown", async (e) => {
  // If recording hotkey, let the recorder handle it
  if (isRecordingHotkey) return;

  // Escape key: close settings or hide flyout window immediately
  if (e.key === "Escape") {
    if (settingsPanel.style.display === "flex") {
      settingsPanel.style.display = "none";
      return;
    }
    await invoke("hide_window");
    return;
  }

  // Ctrl+F or Cmd+F focuses search bar
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
    e.preventDefault();
    searchInputEl.focus();
    searchInputEl.select();
    return;
  }

  // If search input is focused and user presses Down, move focus to list
  if (document.activeElement === searchInputEl) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      searchInputEl.blur();
      clipboardListEl.focus();
      selectedIndex = 0;
      renderList();
    }
    return;
  }

  const items = getFilteredItems();
  if (items.length === 0) return;

  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex = (selectedIndex + 1) % items.length;
    renderList();
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex = (selectedIndex - 1 + items.length) % items.length;
    renderList();
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (items[selectedIndex]) {
      selectItem(items[selectedIndex].id);
    }
  } else if (e.key === "Delete" || e.key === "Backspace") {
    if (document.activeElement !== searchInputEl && items[selectedIndex]) {
      e.preventDefault();
      deleteItem(items[selectedIndex].id);
    }
  } else if (e.key === "p" || e.key === "P") {
    if (document.activeElement !== searchInputEl && items[selectedIndex]) {
      e.preventDefault();
      togglePin(items[selectedIndex].id);
    }
  }
});

// Window Opened event from backend: reset selection, focus search/list
listen("window-opened", () => {
  settingsPanel.style.display = "none";
  selectedIndex = 0;
  if (searchInputEl) {
    searchInputEl.value = "";
    searchQuery = "";
    clearSearchBtn.style.display = "none";
  }
  renderList();
  setTimeout(() => {
    if (clipboardListEl) clipboardListEl.focus();
  }, 50);
});

// Background clipboard event listeners
listen("clipboard-changed", (event) => {
  const newItem = event.payload;
  if (!newItem || !newItem.text) return;

  const existingIdx = clipboardHistory.findIndex((i) => i.text === newItem.text);
  if (existingIdx !== -1) {
    clipboardHistory[existingIdx].timestamp = newItem.timestamp || Date.now();
  } else {
    clipboardHistory.unshift(newItem);
    if (clipboardHistory.length > settings.capacity) {
      const oldestUnpinnedIdx = clipboardHistory.findLastIndex((i) => !i.pinned);
      if (oldestUnpinnedIdx !== -1) {
        clipboardHistory.splice(oldestUnpinnedIdx, 1);
      }
    }
  }
  sortHistory();
  renderList();
});

listen("clipboard-refresh", () => {
  loadHistory();
});

// Periodic timestamp update
setInterval(() => {
  document.querySelectorAll(".clip-card").forEach((card) => {
    const id = card.dataset.id;
    const item = clipboardHistory.find((i) => i.id === id);
    if (item) {
      const timeEl = card.querySelector(".time-ago");
      if (timeEl) timeEl.textContent = formatTimeAgo(item.timestamp);
    }
  });
}, 20000);

// Initialize app
async function init() {
  await loadSettings();
  await loadHistory();
}

init();
