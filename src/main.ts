import { mount } from "svelte";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/controls.css";
import App from "./App.svelte";
import { initTheme } from "./lib/theme";

initTheme();

export default mount(App, { target: document.getElementById("app")! });

// Solo en desarrollo: la ventana de Tauri no recarga con F5 ni Ctrl+R por sí sola.
if (import.meta.env.DEV) {
  window.addEventListener("keydown", (e) => {
    if (e.key === "F5" || (e.ctrlKey && e.key.toLowerCase() === "r")) {
      e.preventDefault();
      location.reload();
    }
  });
}
