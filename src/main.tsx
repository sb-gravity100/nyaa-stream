import { render } from "preact";
import { invoke } from "@tauri-apps/api/core";
import "@fontsource/zen-kaku-gothic-new/400.css";
import "@fontsource/zen-kaku-gothic-new/700.css";
// Display face for headings, titles and UI labels (Latin only - Japanese
// falls through to Zen Kaku).
import "@fontsource-variable/outfit";
import App from "./App";
import { WhatsNew } from "./WhatsNew";
import { installDevLogger } from "./devLogger";
import { installFullscreenHotkey } from "./fullscreen";

installDevLogger();
installFullscreenHotkey();
render(
  <>
    <App />
    <WhatsNew />
  </>,
  document.getElementById("root")!,
);

// The native splash window covers loading; once the app has painted its
// first frame, app_ready shows the main window and closes the splash.
requestAnimationFrame(() =>
  requestAnimationFrame(() => {
    console.debug("[splash] app painted, calling app_ready");
    invoke("app_ready").catch((err) => console.warn("[splash] app_ready failed", { err: String(err) }));
  }),
);
