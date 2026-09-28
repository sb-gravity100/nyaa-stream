import { render } from "preact";
import "@fontsource/zen-kaku-gothic-new/400.css";
import "@fontsource/zen-kaku-gothic-new/500.css";
import "@fontsource/zen-kaku-gothic-new/700.css";
// Display face for headings, titles and UI labels (Latin only - Japanese
// falls through to Zen Kaku).
import "@fontsource-variable/outfit";
import App from "./App";
import { installDevLogger } from "./devLogger";
import { installFullscreenHotkey } from "./fullscreen";

installDevLogger();
installFullscreenHotkey();
render(<App />, document.getElementById("root")!);
