import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles/global.css";
import Overlay from "./windows/Overlay.svelte";
import SettingsWindow from "./windows/SettingsWindow.svelte";

const label = getCurrentWindow().label;
const Root = label === "settings" ? SettingsWindow : Overlay;

export default mount(Root, { target: document.getElementById("app")! });
