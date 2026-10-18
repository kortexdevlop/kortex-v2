## Overview
Kortex Flow is a visual automation editor: you place Kortex tools on a canvas as cards, connect them, and each result feeds the next. A workflow is a small versioned JSON file you can export and import.
## Usage
Pick a tool in the palette: it is placed to the right of the selected node and connected to it automatically, so you can build a chain in a few clicks. To connect differently, drag from an output (right side of a card) to an input (left side of another); Flow checks the types and refuses loops. Drag a card to move it, the background to pan, the wheel to zoom; Delete removes, Ctrl+Z and Ctrl+Y undo and redo. The selected node's properties (text, parameters, retries) are in the right-hand panel. Run it: every card shows live whether it is waiting, running, done or failed, and its result is shown in the panel.
## Frequently asked questions
### Does my data leave my device?
No. Every tool runs locally: in the browser through its embedded engine, on desktop through the installed tool. The workflow only stores its structure, positions and your texts in your device's storage.
### Which tools can I use?
This version offers tools whose required inputs are text. Tools that need files will come with file nodes.
