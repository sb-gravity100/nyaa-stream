import { render } from "preact";
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

// The inline splash (index.html) covers loading; fade it out once the app
// has painted its first frame.
const splash = document.getElementById("splash");
if (splash) {
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      console.debug("[splash] app painted, fading out");
      splash.classList.add("splash-done");
      window.setTimeout(() => splash.remove(), 500);
    }),
  );
}
