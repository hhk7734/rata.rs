# Rata

Rata is a highly interactive, lightning-fast terminal-based (TUI) HTTP client. It is designed to automatically discover your OpenAPI specifications and provide a Postman-like experience directly in your terminal, complete with rich mouse support, templating, and Zellij-style shortcut hints.

## Layout Terms

The Rata UI is divided into the following primary components:

* **Collections**: The left panel. It lists all automatically discovered API endpoints from your OpenAPI specifications grouped by tags.
* **URL**: The top panel. It displays the active HTTP Method, the full requested URL, and an Examples dropdown.
* **Request**: The middle panel. It allows you to configure your API request. It is split into `Params`, `Body`, and `Headers` tabs.
* **Response**: The bottom panel. It displays the result of your API call. It features `Body` (with JSON formatting), `Headers`, and `Cookies` tabs.
* **Shortcut Bar**: The Zellij-style context-aware hint bar at the bottom of the terminal.

## Features

* **Auto-Matching**: Editing the URL dynamically matches against your OpenAPI spec. If a match is found, Rata automatically populates the Request panel with the required parameters, headers, and examples!
* **Templating & Env Vars**: Rata resolves variables like `{{baseUrl}}` or `{{env:USER}}` directly from your environment or OpenAPI config.
* **Default Headers**: Standard headers like `user-agent` are automatically populated in the UI, allowing you to easily toggle or override them.
* **Rich Mouse Support**: Resize panels by dragging borders, copy text by highlighting, and interact with dropdowns seamlessly.

## Keyboard & Mouse Shortcuts

Rata supports deep mouse integration and intuitive keyboard shortcuts based on your current focus.

### Global Shortcuts

* `Ctrl+q` : Quit the application.
* `Ctrl+s` : Send the API request.
* **Mouse** : Click anywhere to focus panels, change tabs, edit fields, or open dropdowns.

### Collections (Left Panel)

* `↑` / `k` : Move selection up (previous operation).
* `↓` / `j` : Move selection down (next operation).
* **Mouse Click** : Select an operation.
* **Mouse Drag (Border)** : Drag the right border to resize the Collections panel.

### URL (Top Panel)

* **Mouse Click (URL)** : Click on the URL text to focus it. Typing instantly edits the URL.
* **Mouse Click (Method)** : Click the HTTP method (e.g. `GET ▾`) to open a dropdown and change the request method.
* **Mouse Click (Examples)** : Click the Examples dropdown (`Examples ▾`) to browse and load mock data for the current operation.

### Request (Middle Panel)

* `↑` / `k` : Move parameter/header selection up.
* `↓` / `j` : Move parameter/header selection down.
* `Space` : Toggle (enable/disable) the currently selected parameter or header.
* `Tab` : Switch focus between the Key and Value fields of a parameter row.
* `Ctrl+e` : Toggle Edit Mode for Request Params and Request Headers.
* `Ctrl+w` : Toggle line wrapping in the Request Body.
* **Mouse Click (Tabs)** : Switch between `Params`, `Body`, and `Headers` tabs.
* **Mouse Click (Row)** : Edit the clicked parameter row.
* **Mouse Drag (Border)** : Drag the top border to resize the Request panel.

### Response (Bottom Panel)

* `↑` / `k` : Scroll the response text up.
* `↓` / `j` : Scroll the response text down.
* `Ctrl+w` : Toggle line wrapping in the Response Body.
* **Mouse Wheel** : Scroll the response text.
* **Mouse Drag (Text)** : Highlight and select text. Letting go of the mouse automatically copies the selected text to your system clipboard!
* **Mouse Click (Tabs)** : Switch between `Body`, `Headers`, and `Cookies` tabs.
* **Mouse Drag (Border)** : Drag the top border to resize the Response panel height.

---
*Built for terminal lovers.*
