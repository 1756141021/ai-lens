import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles/global.css";
import Overlay from "./windows/Overlay.svelte";
import SettingsWindow from "./windows/SettingsWindow.svelte";
import CursorRing from "./windows/CursorRing.svelte";
import PinWindow from "./windows/PinWindow.svelte";

const label = getCurrentWindow().label;
const Root =
  label === "settings" ? SettingsWindow
  : label === "cursor" ? CursorRing
  : label.startsWith("pin-") ? PinWindow
  : Overlay;

export default mount(Root, { target: document.getElementById("app")! });
