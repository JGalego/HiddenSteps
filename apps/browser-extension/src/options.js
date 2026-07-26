import { DEFAULT_PORT } from "./extract.js";

const tokenInput = document.getElementById("token");
const portInput = document.getElementById("port");
const result = document.getElementById("result");

async function load() {
  const { token, port } = await chrome.storage.local.get(["token", "port"]);
  tokenInput.value = token || "";
  portInput.value = typeof port === "number" ? port : DEFAULT_PORT;
}

async function save() {
  const token = tokenInput.value.trim();
  const port = Number.parseInt(portInput.value, 10) || DEFAULT_PORT;
  await chrome.storage.local.set({ token, port });
  result.textContent = "Saved.";
}

async function testConnection() {
  const token = tokenInput.value.trim();
  const port = Number.parseInt(portInput.value, 10) || DEFAULT_PORT;
  if (!token) {
    result.textContent = "Enter a pairing token first.";
    return;
  }
  try {
    const response = await fetch(`http://127.0.0.1:${port}/v1/status`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (response.status === 401) {
      result.textContent = "Connected, but the token was rejected — check it and try again.";
      return;
    }
    if (!response.ok) {
      result.textContent = `Unexpected response (HTTP ${response.status}).`;
      return;
    }
    const body = await response.json();
    result.textContent = `Connected. Desktop app privacy level: ${body.privacy_level}.`;
  } catch {
    result.textContent =
      "Could not reach the HiddenSteps desktop app — make sure it's running.";
  }
}

document.getElementById("save").addEventListener("click", save);
document.getElementById("test").addEventListener("click", testConnection);
load();
