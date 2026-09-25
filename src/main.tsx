import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { AtddHarness } from "./components/common/AtddHarness";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
    {import.meta.env.VITE_ATDD === "1" && <AtddHarness />}
  </React.StrictMode>,
);
