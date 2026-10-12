"use strict";

for (const snippet of document.querySelectorAll("pre[aria-label]")) {
  const code = snippet.querySelector("code");
  if (!code) continue;

  const tools = document.createElement("div");
  tools.className = "snippet-tools";
  const status = document.createElement("span");
  status.className = "copy-status";
  status.setAttribute("role", "status");
  const button = document.createElement("button");
  button.type = "button";
  button.className = "copy-button";
  button.textContent = "Copy commands";
  button.setAttribute("aria-label", `Copy commands: ${snippet.getAttribute("aria-label")}`);

  button.addEventListener("click", async () => {
    button.disabled = true;
    status.textContent = "";
    try {
      await navigator.clipboard.writeText(code.textContent);
      status.textContent = "Copied to clipboard.";
    } catch {
      status.textContent = "Copy unavailable. Select the commands and copy them manually.";
    } finally {
      button.disabled = false;
    }
  });

  tools.append(status, button);
  snippet.before(tools);
  snippet.classList.add("copy-snippet");
}
