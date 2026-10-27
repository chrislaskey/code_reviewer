import { mount } from "svelte";
import "./app.css";
import SessionsIndexPage from "./SessionsIndexPage.svelte";

mount(SessionsIndexPage, { target: document.getElementById("app") });
