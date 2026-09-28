import { render } from "preact";
import "@fontsource/zen-kaku-gothic-new/400.css";
import "@fontsource/zen-kaku-gothic-new/500.css";
import "@fontsource/zen-kaku-gothic-new/700.css";
import App from "./App";
import { installDevLogger } from "./devLogger";

installDevLogger();
render(<App />, document.getElementById("root")!);
