import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// State
let clipboardHistory = [];
let activeFilter = "all";
let searchQuery = "";
let selectedIndex = 0;
let isPaused = false;
let settings = {
  hotkey: "Super+V",
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
const clipCountBadge = document.getElementById("clip-count-badge");
const pinnedCountChip = document.getElementById("pinned-count-chip");
const filterTabs = document.querySelectorAll(".filter-tab");
const btnTogglePause = document.getElementById("btn-toggle-pause");
const btnClose = document.getElementById("btn-close");
const btnClearAll = document.getElementById("btn-clear-all");
const btnSettings = document.getElementById("btn-settings");
const btnCloseSettings = document.getElementById("btn-close-settings");
const settingsModal = document.getElementById("settings-modal");
const toastEl = document.getElementById("toast");

// Settings elements
const settingHotkey = document.getElementById("setting-hotkey");
const settingCapacity = document.getElementById("setting-capacity");
const settingAutoPaste = document.getElementById("setting-auto-paste");
const settingFeedback = document.getElementById("setting-feedback");
const settingClearExit = document.getElementById("setting-clear-exit");

// Detect item category
function detectType(text) {
  if (!text) return "text";
  const trimmed = text.trim();
  if (/^(https?:\/\/|www\.)[^\s/$.?#].[^\s]*$/i.test(trimmed)) {
    return "link";
  }
  if (
    /(\{|\}|=>|function|const\s|let\s|var\s|class\s|import\s|def\s|pub\s|fn\s|return\s|#include|<html|<\?php)/i.test(
      trimmed
    ) ||
    trimmed.includes("\n  ") ||
    trimmed.includes("\n\t")
  ) {
    return "code";
  }
  return "text";
}

// Relative time formatter
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

// Show Toast notification
let toastTimer = null;
function showToast(message) {
  if (!settings.feedback) return;
  toastEl.textContent = message;
  toastEl.style.display = "block";
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastEl.style.display = "none";
  }, 1600);
}

// Filtered and searched items
function getFilteredItems() {
  return clipboardHistory.filter((item) => {
    // Filter tab
    if (activeFilter === "pinned" && !item.pinned) return false;
    if (activeFilter === "code" && item.type !== "code") return false;
    if (activeFilter === "link" && item.type !== "link") return false;
    if (activeFilter === "text" && item.type !== "text") return false;

    // Search query
    if (searchQuery) {
      return item.text.toLowerCase().includes(searchQuery.toLowerCase());
    }
    return true;
  });
}

// Render UI
function renderList() {
  const items = getFilteredItems();
  const pinnedCount = clipboardHistory.filter((i) => i.pinned).length;
  clipCountBadge.textContent = `${clipboardHistory.length} items`;
  pinnedCountChip.textContent = pinnedCount;

  if (items.length === 0) {
    clipboardListEl.innerHTML = "";
    emptyStateEl.style.display = "flex";
    if (searchQuery) {
      document.getElementById("empty-title").textContent = "No matching items";
      document.getElementById("empty-subtitle").textContent =
        `No clipboard entries matched "${searchQuery}"`;
    } else if (activeFilter === "pinned") {
      document.getElementById("empty-title").textContent = "No pinned items";
      document.getElementById("empty-subtitle").textContent =
        "Click the pin icon on any item to keep it permanently saved.";
    } else {
      document.getElementById("empty-title").textContent = "Clipboard is empty";
      document.getElementById("empty-subtitle").textContent =
        "Copy text or snippets anywhere and they will appear here.";
    }
    return;
  }

  emptyStateEl.style.display = "none";

  // Clamp selected index
  if (selectedIndex >= items.length) {
    selectedIndex = Math.max(0, items.length - 1);
  }

  clipboardListEl.innerHTML = items
    .map((item, index) => {
      const isSelected = index === selectedIndex;
      const lines = item.text.split("\n").length;
      const chars = item.text.length;
      const previewLines = lines > 1 ? `${lines} lines` : `${chars} chars`;

      return `
        <div class="clip-card ${item.pinned ? "pinned" : ""} ${isSelected ? "selected" : ""}" 
             data-id="${item.id}" data-index="${index}">
          <div class="card-header">
            <div class="card-header-left">
              <span class="type-tag ${item.type}">${item.type}</span>
              <span class="time-ago">${formatTimeAgo(item.timestamp)}</span>
            </div>
            <div class="card-actions">
              <button class="action-btn pin-btn ${item.pinned ? "active" : ""}" 
                      title="${item.pinned ? "Unpin item" : "Pin item"}" 
                      data-action="pin" data-id="${item.id}">
                📌
              </button>
              <button class="action-btn delete-btn" title="Delete item" data-action="delete" data-id="${item.id}">
                ✕
              </button>
            </div>
          </div>
          <div class="card-content ${item.type === "code" ? "code-content" : ""}">${escapeHtml(item.text)}</div>
          <div class="card-footer">
            <div class="card-meta">
              <span>${previewLines}</span>
            </div>
            <span class="copy-hint">Click or Enter to paste</span>
          </div>
        </div>
      `;
    })
    .join("");

  // Scroll active item into view
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

// Select & Copy Action
async function selectItem(id) {
  try {
    await invoke("copy_item", { id, autoPaste: settings.autoPaste });
    showToast("Copied to clipboard!");
    // Hide window after copy
    await invoke("hide_window");
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
    const updated = await invoke("toggle_pin", { id });
    const item = clipboardHistory.find((i) => i.id === id);
    if (item) {
      item.pinned = !item.pinned;
      // Re-sort: pinned items on top
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
    clipboardHistory = (history || []).map((item) => ({
      ...item,
      type: item.type || detectType(item.text),
    }));
    sortHistory();
    renderList();
  } catch (err) {
    console.error("Failed to load history:", err);
  }
}

// Event Listeners for DOM
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

// Search handling
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

// Filter tabs
filterTabs.forEach((tab) => {
  tab.addEventListener("click", () => {
    filterTabs.forEach((t) => t.classList.remove("active"));
    tab.classList.add("active");
    activeFilter = tab.dataset.filter;
    selectedIndex = 0;
    renderList();
  });
});

// Clear all history
btnClearAll.addEventListener("click", async () => {
  if (clipboardHistory.length === 0) return;
  const confirmed = confirm("Are you sure you want to clear all unpinned clipboard history?");
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

// Toggle Pause monitoring
btnTogglePause.addEventListener("click", async () => {
  try {
    isPaused = await invoke("toggle_pause");
    btnTogglePause.title = isPaused ? "Monitoring Paused (Click to resume)" : "Monitoring Active";
    btnTogglePause.style.color = isPaused ? "var(--danger)" : "var(--text-secondary)";
    showToast(isPaused ? "Clipboard monitoring paused" : "Clipboard monitoring active");
  } catch (err) {
    console.error("Failed to toggle pause:", err);
  }
});

// Close button
btnClose.addEventListener("click", async () => {
  await invoke("hide_window");
});

// Settings Modal
btnSettings.addEventListener("click", () => {
  settingsModal.style.display = "flex";
});

btnCloseSettings.addEventListener("click", () => {
  settingsModal.style.display = "none";
});

settingsModal.addEventListener("click", (e) => {
  if (e.target === settingsModal) {
    settingsModal.style.display = "none";
  }
});

// Save settings changes
function handleSettingChange() {
  settings.hotkey = settingHotkey.value;
  settings.capacity = parseInt(settingCapacity.value, 10);
  settings.autoPaste = settingAutoPaste.checked;
  settings.feedback = settingFeedback.checked;
  settings.clearOnExit = settingClearExit.checked;

  localStorage.setItem("clipstore_settings", JSON.stringify(settings));
  invoke("save_settings", { settings });
}

settingHotkey.addEventListener("change", handleSettingChange);
settingCapacity.addEventListener("change", handleSettingChange);
settingAutoPaste.addEventListener("change", handleSettingChange);
settingFeedback.addEventListener("change", handleSettingChange);
settingClearExit.addEventListener("change", handleSettingChange);

function loadSavedSettings() {
  const saved = localStorage.getItem("clipstore_settings");
  if (saved) {
    try {
      settings = { ...settings, ...JSON.parse(saved) };
      settingHotkey.value = settings.hotkey;
      settingCapacity.value = settings.capacity.toString();
      settingAutoPaste.checked = settings.autoPaste;
      settingFeedback.checked = settings.feedback;
      settingClearExit.checked = settings.clearOnExit;
    } catch (e) {
      console.error("Error parsing settings:", e);
    }
  }
}

// Global Keyboard Navigation
window.addEventListener("keydown", async (e) => {
  // Esc closes modal or window
  if (e.key === "Escape") {
    if (settingsModal.style.display === "flex") {
      settingsModal.style.display = "none";
      return;
    }
    await invoke("hide_window");
    return;
  }

  // Ctrl+F focuses search
  if ((e.ctrlKey || e.metaKey) && e.key === "f") {
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
    if (items[selectedIndex]) {
      e.preventDefault();
      deleteItem(items[selectedIndex].id);
    }
  } else if (e.key === "p" || e.key === "P") {
    if (items[selectedIndex]) {
      e.preventDefault();
      togglePin(items[selectedIndex].id);
    }
  }
});

// Periodic time update
setInterval(() => {
  document.querySelectorAll(".clip-card").forEach((card) => {
    const id = card.dataset.id;
    const item = clipboardHistory.find((i) => i.id === id);
    if (item) {
      const timeEl = card.querySelector(".time-ago");
      if (timeEl) timeEl.textContent = formatTimeAgo(item.timestamp);
    }
  });
}, 30000);

// Initialize
async function init() {
  loadSavedSettings();
  await loadHistory();

  // Listen for backend new clipboard events
  try {
    await listen("clipboard-changed", (event) => {
      const newItem = event.payload;
      if (!newItem || !newItem.text) return;

      newItem.type = newItem.type || detectType(newItem.text);

      // Check if item already exists (deduplication)
      const existingIndex = clipboardHistory.findIndex((i) => i.text === newItem.text);
      if (existingIndex !== -1) {
        const existing = clipboardHistory[existingIndex];
        existing.timestamp = newItem.timestamp || Date.now();
      } else {
        clipboardHistory.unshift(newItem);
        if (clipboardHistory.length > settings.capacity) {
          // Evict oldest unpinned
          const unpinned = clipboardHistory.filter((i) => !i.pinned);
          if (unpinned.length > 0) {
            const oldestUnpinned = unpinned[unpinned.length - 1];
            clipboardHistory = clipboardHistory.filter((i) => i.id !== oldestUnpinned.id);
          }
        }
      }
      sortHistory();
      renderList();
    });
  } catch (err) {
    console.warn("Could not attach clipboard-changed listener:", err);
  }
}

init();
