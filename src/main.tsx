import { render } from "preact";
import App from "./App";
import { installDevLogger } from "./devLogger";

installDevLogger();
render(<App />, document.getElementById("root")!);
