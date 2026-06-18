/* @refresh reload */
import { render } from "solid-js/web";
import App from "./App";
import ContainerWindow from "./components/ContainerWindow";
import type { Source } from "./api";
import "./theme.css";

const root = document.getElementById("root") as HTMLElement;
const params = new URLSearchParams(location.search);

if (params.get("window") === "container") {
  // A container pop-out window: render just that container's grid.
  const source = (params.get("source") || "inventory") as Source;
  const owner = params.get("owner") || "";
  const label = params.get("label") || "Container";
  render(() => <ContainerWindow source={source} ownerId={owner} label={label} />, root);
} else {
  render(() => <App />, root);
}
