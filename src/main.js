import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

// Svelte 5: components are not classes — `new App(...)` throws and leaves a
// blank window. mount() is the v5 entry point.
const app = mount(App, { target: document.getElementById("app") });

export default app;
