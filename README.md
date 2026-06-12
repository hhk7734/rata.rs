# Rata

Rata is a highly interactive, lightning-fast terminal-based (TUI) HTTP client. It is designed to automatically discover your OpenAPI specifications and provide a Postman-like experience directly in your terminal, complete with rich mouse support and Zellij-style shortcut hints.

## Layout Terms

The Rata UI is divided into the following primary components:

* **Collections**: The left panel. It lists all automatically discovered API endpoints from your OpenAPI specifications grouped by tags.
* **URL**: The top panel. It displays the active HTTP Method, the full requested URL, and an Examples dropdown.
* **Request**: The middle panel. It allows you to configure your API request. It is split into `Params`, `Body`, and `Headers` tabs.
* **Response**: The bottom panel. It displays the result of your API call. It features `Body` (with JSON formatting), `Headers`, and `Cookies` tabs.
* **Shortcut Bar**: The Zellij-style context-aware hint bar at the bottom of the terminal.

## Keyboard & Mouse Shortcuts

Rata supports deep mouse integration and intuitive keyboard shortcuts based on your current focus.

### Global Shortcuts (Normal Mode)

* `Ctrl+q` : Quit the application.
* `Ctrl+s` : Send the API request.
* **Mouse** : Click anywhere to focus panels, change tabs, edit fields, or open the Examples dropdown.

### Collections (Left Panel)

* `↑` / `k` : Move selection up (previous operation).
* `↓` / `j` : Move selection down (next operation).
* **Mouse Click** : Select an operation.
* **Mouse Drag (Border)** : Drag the right border to resize the Collections panel.

### URL (Top Panel)

* **Mouse Click** : Click on the URL text to enter Edit Mode.

### Request (Middle Panel)

* `↑` / `k` : Move parameter/header selection up.
* `↓` / `j` : Move parameter/header selection down.
* `Enter` : Edit the currently highlighted parameter or header value.
* `Space` : Toggle (enable/disable) the highlighted optional parameter (checking/unchecking `[x]`).
* **Mouse Click (Tabs)** : Switch between `Params`, `Body`, and `Headers` tabs.
* **Mouse Click (Row)** : Edit the clicked parameter row.
* **Mouse Drag (Border)** : Drag the top border to resize the Request panel.
* `Esc` / `Enter` (while editing) : Cancel or save your parameter edits.

### Response (Bottom Panel)

* `↑` / `k` : Scroll the response text up.
* `↓` / `j` : Scroll the response text down.
* **Mouse Wheel** : Scroll the response text.
* **Mouse Drag (Text)** : Highlight and select text. Letting go of the mouse automatically copies the selected text to your system clipboard!
* **Mouse Click (Tabs)** : Switch between `Body`, `Headers`, and `Cookies` tabs.
* **Mouse Drag (Border)** : Drag the top border to resize the Response panel height.

---
*Built for terminal lovers.*
