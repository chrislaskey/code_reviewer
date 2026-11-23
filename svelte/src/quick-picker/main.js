// The quick picker's window in the desktop app (../../../tauri/src/quick_picker/):
// one page, no router.
import { mount } from "svelte";
import QuickPickerPage from "./QuickPickerPage.svelte";
import { translationsStart } from "../lib/translations.js";

translationsStart().then(() => mount(QuickPickerPage, { target: document.getElementById("app") }));
